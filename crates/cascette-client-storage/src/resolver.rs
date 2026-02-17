//! Content resolution pipeline
//!
//! Resolves file paths to actual content through the CASC lookup chain.

use crate::{Result, StorageError};
use cascette_crypto::Jenkins96;
use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::{encoding::EncodingFile, root::RootFile};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, info};

/// Resolves file paths to content through the CASC lookup chain
pub struct ContentResolver {
    /// Root file for path -> content key lookup
    root_file: Arc<RwLock<Option<RootFile>>>,
    /// Encoding file for content key -> encoding key lookup
    encoding_file: Arc<RwLock<Option<EncodingFile>>>,
    /// Path resolution cache
    path_cache: DashMap<String, ContentKey>,
    /// Content key resolution cache
    content_cache: DashMap<ContentKey, EncodingKey>,
    /// `FileDataID` to content key mapping (for modern root files)
    file_data_id_map: DashMap<u32, ContentKey>,
}

impl ContentResolver {
    /// Create a new content resolver
    pub fn new() -> Self {
        Self {
            root_file: Arc::new(RwLock::new(None)),
            encoding_file: Arc::new(RwLock::new(None)),
            path_cache: DashMap::new(),
            content_cache: DashMap::new(),
            file_data_id_map: DashMap::new(),
        }
    }

    /// Load root file for path resolution
    ///
    /// # Errors
    ///
    /// Returns error if root file cannot be parsed
    pub fn load_root_file(&self, data: &[u8]) -> Result<()> {
        info!("Loading root file ({} bytes)", data.len());

        // Parse root file
        let root = RootFile::parse(data)
            .map_err(|e| StorageError::Resolver(format!("Failed to parse root file: {e}")))?;

        // Log root file info
        debug!(
            "Loaded root file version {:?} with {} total files, {} named files",
            root.version,
            root.total_files(),
            root.named_files()
        );

        // Build FileDataID map if supported
        for block in &root.blocks {
            for entry in &block.records {
                self.file_data_id_map
                    .insert(entry.file_data_id.get(), entry.content_key);
            }
        }

        *self.root_file.write() = Some(root);
        Ok(())
    }

    /// Load encoding file for content -> encoding lookup
    ///
    /// # Errors
    ///
    /// Returns error if encoding file cannot be parsed
    pub fn load_encoding_file(&self, data: &[u8]) -> Result<()> {
        info!("Loading encoding file ({} bytes)", data.len());

        let encoding = EncodingFile::parse(data)
            .map_err(|e| StorageError::Resolver(format!("Failed to parse encoding file: {e}")))?;

        // Build content key cache from pages
        let mut cached_entries = 0;
        for page in &encoding.ckey_pages {
            for entry in &page.entries {
                // Cache content key to encoding key mappings (use first encoding key)
                if let Some(encoding_key) = entry.encoding_keys.first() {
                    self.content_cache.insert(entry.content_key, *encoding_key);
                    cached_entries += 1;
                }
            }
        }

        debug!(
            "Loaded encoding file with {} ckey pages and cached {} entries",
            encoding.ckey_pages.len(),
            cached_entries
        );
        *self.encoding_file.write() = Some(encoding);
        Ok(())
    }

    /// Resolve a file path to a content key
    pub fn resolve_path(&self, path: &str) -> Option<ContentKey> {
        // Check cache first
        if let Some(cached) = self.path_cache.get(path) {
            return Some(*cached);
        }

        let content_key = self.root_file.read().as_ref().and_then(|root_file| {
            // Calculate name hash for path-based lookup
            let name_hash = Jenkins96::hash(path.as_bytes());

            // Search through all blocks for matching entry
            root_file
                .blocks
                .iter()
                .flat_map(|block| &block.records)
                .find(|entry| entry.name_hash == Some(name_hash.hash64))
                .map(|entry| entry.content_key)
        });

        if let Some(key) = content_key {
            self.path_cache.insert(path.to_string(), key);
        }

        content_key
    }

    /// Resolve a `FileDataID` to a content key (for modern clients)
    pub fn resolve_file_data_id(&self, fdid: u32) -> Option<ContentKey> {
        self.file_data_id_map.get(&fdid).map(|v| *v)
    }

    /// Resolve a content key to an encoding key
    pub fn resolve_content_key(&self, key: &ContentKey) -> Option<EncodingKey> {
        // Check cache first
        if let Some(cached) = self.content_cache.get(key) {
            return Some(*cached);
        }

        // Search in encoding file pages
        let encoding = {
            let guard = self.encoding_file.read();
            guard.as_ref()?.clone()
        };

        // Search all content key pages for the key
        for page in &encoding.ckey_pages {
            for entry in &page.entries {
                if entry.content_key == *key {
                    // Use the first encoding key (primary encoding)
                    if let Some(encoding_key) = entry.encoding_keys.first() {
                        // Cache the result for future lookups
                        self.content_cache.insert(*key, *encoding_key);
                        return Some(*encoding_key);
                    }
                }
            }
        }

        debug!("Content key not found: {}", hex::encode(key.as_bytes()));
        None
    }

    /// Complete resolution from path to encoding key
    pub fn resolve_path_to_encoding(&self, path: &str) -> Option<EncodingKey> {
        self.resolve_path(path)
            .and_then(|content_key| self.resolve_content_key(&content_key))
    }

    /// Resolve from `FileDataID` to encoding key
    pub fn resolve_fdid_to_encoding(&self, fdid: u32) -> Option<EncodingKey> {
        self.resolve_file_data_id(fdid)
            .and_then(|content_key| self.resolve_content_key(&content_key))
    }

    /// Get information about a file
    pub fn get_file_info(&self, path: &str) -> Option<FileInfo> {
        let content_key = self.resolve_path(path)?;
        let encoding_key = self.resolve_content_key(&content_key)?;

        // Get file size from encoding file
        let size = self.get_content_size(&content_key).unwrap_or(0);

        Some(FileInfo {
            path: path.to_string(),
            content_key,
            encoding_key,
            size,
        })
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.path_cache.clear();
        self.content_cache.clear();
        self.file_data_id_map.clear();
    }

    /// Get the size of content by content key
    pub fn get_content_size(&self, key: &ContentKey) -> Option<u64> {
        let encoding = {
            let guard = self.encoding_file.read();
            guard.as_ref()?.clone()
        };

        // Search content key pages for size information
        for page in &encoding.ckey_pages {
            for entry in &page.entries {
                if entry.content_key == *key {
                    return Some(entry.file_size);
                }
            }
        }

        None
    }

    /// Get resolver statistics
    pub fn stats(&self) -> ResolverStats {
        let root_entries = {
            let guard = self.root_file.read();
            guard.as_ref().map_or(0, |r| r.total_files() as usize)
        };

        let encoding_entries = {
            let guard = self.encoding_file.read();
            guard
                .as_ref()
                .map_or(0, |e| (e.ckey_pages.len() + e.ekey_pages.len()) * 1_000) // Rough estimate
        };

        ResolverStats {
            root_entries,
            encoding_entries,
            path_cache_size: self.path_cache.len(),
            content_cache_size: self.content_cache.len(),
        }
    }
}

impl Default for ContentResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a resolved file
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File path
    pub path: String,
    /// Content key from root file
    pub content_key: ContentKey,
    /// Encoding key from encoding file
    pub encoding_key: EncodingKey,
    /// File size in bytes
    pub size: u64,
}

/// Resolver statistics
#[derive(Debug, Clone)]
pub struct ResolverStats {
    /// Number of entries in root file
    pub root_entries: usize,
    /// Number of entries in encoding file
    pub encoding_entries: usize,
    /// Number of cached path lookups
    pub path_cache_size: usize,
    /// Number of cached content lookups
    pub content_cache_size: usize,
}

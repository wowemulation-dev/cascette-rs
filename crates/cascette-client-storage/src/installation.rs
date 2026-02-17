//! Game installation management
//!
//! This module handles local CASC storage only - the `.idx` index files and
//! `.data` archive files in the `Data/data/` directory.
//!
//! CDN indices (`.index` files in `Data/indices/`) are NOT part of local storage
//! and should be handled separately where needed (e.g., browse commands).

use crate::{
    Result, StorageError, archive::ArchiveManager, index::IndexManager, resolver::ContentResolver,
};
use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, info, warn};

/// Represents a game installation with its local CASC storage
///
/// This handles only the local storage components:
/// - `.idx` index files (map encoding keys to archive locations)
/// - `.data` archive files (contain BLTE-encoded content)
///
/// CDN indices (`.index` files) are NOT managed here.
pub struct Installation {
    path: PathBuf,
    /// Index manager for .idx files (local storage indices)
    index_manager: Arc<AsyncRwLock<IndexManager>>,
    /// Archive manager for .data files
    archive_manager: Arc<AsyncRwLock<ArchiveManager>>,
    /// Content resolver for lookup chain
    resolver: Arc<ContentResolver>,
    /// Simple in-memory cache for performance optimization
    cache: Arc<AsyncRwLock<dashmap::DashMap<String, Vec<u8>>>>,
}

impl Installation {
    /// Open an existing installation or create a new one
    ///
    /// # Errors
    ///
    /// Returns error if directory cannot be created or components cannot be initialized
    pub fn open(path: PathBuf) -> Result<Self> {
        // Ensure installation directory exists
        if !path.exists() {
            info!("Creating installation directory: {}", path.display());
            std::fs::create_dir_all(&path)?;
        }

        // Create required subdirectories
        let indices_path = path.join(crate::INDICES_DIR);
        let data_path = path.join(crate::DATA_DIR);
        let config_path = path.join(crate::CONFIG_DIR);

        for dir in [&indices_path, &data_path, &config_path] {
            if !dir.exists() {
                std::fs::create_dir_all(dir)?;
            }
        }

        // Initialize managers for local CASC storage
        // Note: Both .idx (index) and .data (archive) files are in Data/data/ directory
        // CDN .index files in Data/indices/ are NOT handled here
        let index_manager = Arc::new(AsyncRwLock::new(IndexManager::new(&data_path)));
        let archive_manager = Arc::new(AsyncRwLock::new(ArchiveManager::new(&data_path)));
        let resolver = Arc::new(ContentResolver::new());

        // Initialize simple in-memory cache for performance
        let cache = Arc::new(AsyncRwLock::new(dashmap::DashMap::new()));

        info!("Opened installation at {}", path.display());

        Ok(Self {
            path,
            index_manager,
            archive_manager,
            resolver,
            cache,
        })
    }

    /// Read a file by content key
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be found or read
    pub async fn read_file(&self, key: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 16 {
            return Err(StorageError::InvalidFormat(
                "Content key must be 16 bytes".to_string(),
            ));
        }
        let mut key_bytes = [0u8; 16];
        key_bytes.copy_from_slice(key);
        let content_key = ContentKey::from_bytes(key_bytes);
        self.read_file_by_content_key(&content_key).await
    }

    /// Read a file by content key (attempts local index lookup with caching)
    ///
    /// WARNING: Local .idx files are actually keyed by ENCODING keys (truncated to 9 bytes),
    /// not content keys. This method is provided for backward compatibility but may not
    /// find files. Use `read_file_by_encoding_key()` for reliable local storage access.
    ///
    /// The correct CASC lookup chain is:
    /// 1. Root file: FDID -> ContentKey
    /// 2. Encoding file: ContentKey -> EncodingKey
    /// 3. Local .idx: EncodingKey -> archive location
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be found or read
    pub async fn read_file_by_content_key(&self, content_key: &ContentKey) -> Result<Vec<u8>> {
        let cache_key = hex::encode(content_key.as_bytes());
        debug!("Reading file by content key: {}", cache_key);

        // Step 1: Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached_data) = cache.get(&cache_key) {
                debug!("Cache hit for content key: {}", cache_key);
                return Ok(cached_data.clone());
            }
        }

        debug!("Cache miss for content key: {}", cache_key);

        // Step 2: Look up content key directly in local indices
        // Local .idx files use content keys (not encoding keys)
        let index_entry = {
            let index_manager = self.index_manager.read().await;
            index_manager
                .lookup_by_content_key(content_key)
                .ok_or_else(|| {
                    StorageError::NotFound(format!(
                        "Content key not found in local indices: {cache_key}"
                    ))
                })?
        };

        debug!(
            "Found in archive {} at offset {} with size {}",
            index_entry.archive_id(),
            index_entry.archive_offset(),
            index_entry.size
        );

        // Step 3: Read raw BLTE data from archive
        let raw_data = {
            let archive_manager = self.archive_manager.read().await;
            archive_manager.read_content(
                index_entry.archive_id(),
                index_entry.archive_offset(),
                index_entry.size,
            )?
        };

        // Step 4: Decode BLTE container to get actual file content
        let data = Self::decode_blte(&raw_data)?;

        // Step 5: Cache the decoded result for future reads
        {
            let cache = self.cache.read().await;
            cache.insert(cache_key, data.clone());
        }

        Ok(data)
    }

    /// Read a file by encoding key from local storage
    ///
    /// This is the CORRECT method for reading files from local CASC installations.
    /// Local .idx files are keyed by encoding keys (truncated to 9 bytes).
    ///
    /// The CASC lookup chain is:
    /// 1. Root file: FDID -> ContentKey
    /// 2. Encoding file: ContentKey -> EncodingKey
    /// 3. Local .idx: EncodingKey -> archive location
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be found or read
    pub async fn read_file_by_encoding_key(&self, encoding_key: &EncodingKey) -> Result<Vec<u8>> {
        let cache_key = format!("ekey:{}", hex::encode(encoding_key.as_bytes()));
        debug!("Reading file by encoding key: {}", cache_key);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached_data) = cache.get(&cache_key) {
                debug!("Cache hit for encoding key");
                return Ok(cached_data.clone());
            }
        }

        // Look up encoding key in indices to get archive location
        let index_entry = {
            let index_manager = self.index_manager.read().await;
            index_manager.lookup(encoding_key).ok_or_else(|| {
                StorageError::NotFound(format!(
                    "Archive location not found for encoding key: {}",
                    hex::encode(encoding_key.as_bytes())
                ))
            })?
        };

        debug!(
            "Found in archive {} at offset {} with size {}",
            index_entry.archive_id(),
            index_entry.archive_offset(),
            index_entry.size
        );

        // Read raw BLTE data from archive
        let raw_data = {
            let archive_manager = self.archive_manager.read().await;
            archive_manager.read_content(
                index_entry.archive_id(),
                index_entry.archive_offset(),
                index_entry.size,
            )?
        };

        // Decode BLTE container to get actual file content
        let data = Self::decode_blte(&raw_data)?;

        // Cache the decoded result
        {
            let cache = self.cache.read().await;
            cache.insert(cache_key, data.clone());
        }

        Ok(data)
    }

    /// Decode BLTE-encoded data to get the actual file content
    ///
    /// Local CASC archives have a 30-byte header before each BLTE entry:
    /// - 0x00-0x0F: Encoding key (16 bytes, reversed)
    /// - 0x10-0x13: Size including header (4 bytes)
    /// - 0x14-0x15: Flags (2 bytes)
    /// - 0x16-0x19: ChecksumA (4 bytes)
    /// - 0x1A-0x1D: ChecksumB (4 bytes)
    /// - 0x1E+: BLTE data
    fn decode_blte(raw_data: &[u8]) -> Result<Vec<u8>> {
        /// Local archive entry header size (before BLTE data)
        const LOCAL_HEADER_SIZE: usize = 0x1E; // 30 bytes

        // Check minimum size for local header + BLTE magic
        if raw_data.len() < LOCAL_HEADER_SIZE + 4 {
            // Too small, return as-is
            debug!("Data too small for local CASC format, returning raw");
            return Ok(raw_data.to_vec());
        }

        // Check for BLTE magic at offset 0x1E (after local header)
        let blte_offset = if &raw_data[LOCAL_HEADER_SIZE..LOCAL_HEADER_SIZE + 4] == b"BLTE" {
            // Standard local archive format with 30-byte header
            LOCAL_HEADER_SIZE
        } else if &raw_data[0..4] == b"BLTE" {
            // Direct BLTE (no local header, e.g., from CDN)
            0
        } else {
            // Not BLTE-encoded, return raw data
            debug!("Data is not BLTE-encoded, returning raw");
            return Ok(raw_data.to_vec());
        };

        let blte_data = &raw_data[blte_offset..];

        // Parse and decompress BLTE
        let blte = BlteFile::parse(blte_data).map_err(|e| {
            StorageError::Io(std::io::Error::other(format!("Failed to parse BLTE: {e}")))
        })?;

        let decoded = blte.decompress().map_err(|e| {
            // If decompression fails (e.g., encrypted), return raw data with warning
            warn!("BLTE decompression failed: {e}");
            StorageError::Io(std::io::Error::other(format!(
                "BLTE decompression failed: {e}"
            )))
        })?;

        debug!(
            "BLTE decoded: {} bytes (offset {}) -> {} bytes",
            raw_data.len(),
            blte_offset,
            decoded.len()
        );
        Ok(decoded)
    }

    /// Read a file by path (complete resolution chain with caching)
    ///
    /// # Errors
    ///
    /// Returns error if path cannot be resolved or file cannot be read
    pub async fn read_file_by_path(&self, path: &str) -> Result<Vec<u8>> {
        debug!("Reading file by path: {}", path);

        // Check cache with path as key first
        {
            let cache = self.cache.read().await;
            if let Some(cached_data) = cache.get(path) {
                debug!("Cache hit for path: {}", path);
                return Ok(cached_data.clone());
            }
        }

        // Step 1: Resolve path to content key using root file
        let content_key = self.resolver.resolve_path(path).ok_or_else(|| {
            StorageError::NotFound(format!("Path not found in root file: {path}"))
        })?;

        // Step 2: Use content key pipeline
        let data = self.read_file_by_content_key(&content_key).await?;

        // Cache with path as well for faster future path-based lookups
        {
            let cache = self.cache.read().await;
            cache.insert(path.to_string(), data.clone());
        }

        Ok(data)
    }

    /// Read a file by `FileDataID` (modern clients with caching)
    ///
    /// # Errors
    ///
    /// Returns error if `FileDataID` cannot be resolved or file cannot be read
    pub async fn read_file_by_fdid(&self, fdid: u32) -> Result<Vec<u8>> {
        let fdid_key = format!("fdid:{fdid}");
        debug!("Reading file by FileDataID: {}", fdid);

        // Check cache with FDID as key
        {
            let cache = self.cache.read().await;
            if let Some(cached_data) = cache.get(&fdid_key) {
                debug!("Cache hit for FDID: {}", fdid);
                return Ok(cached_data.clone());
            }
        }

        // Step 1: Resolve FileDataID to content key
        let content_key = self
            .resolver
            .resolve_file_data_id(fdid)
            .ok_or_else(|| StorageError::NotFound(format!("FileDataID not found: {fdid}")))?;

        // Step 2: Use content key pipeline
        let data = self.read_file_by_content_key(&content_key).await?;

        // Cache with FDID key for future lookups
        {
            let cache = self.cache.read().await;
            cache.insert(fdid_key, data.clone());
        }

        Ok(data)
    }

    /// Read multiple files concurrently by content keys
    ///
    /// # Errors
    ///
    /// Returns error if any file cannot be found or read
    pub async fn read_files_by_content_keys(
        self: Arc<Self>,
        keys: &[ContentKey],
    ) -> Result<Vec<Vec<u8>>> {
        use futures::future::join_all;

        let futures = keys.iter().map(|&key| {
            let installation = Arc::clone(&self);
            async move { installation.read_file_by_content_key(&key).await }
        });

        let results: Result<Vec<_>> = join_all(futures).await.into_iter().collect();

        results
    }

    /// Read multiple files concurrently by paths
    ///
    /// # Errors
    ///
    /// Returns error if any path cannot be resolved or file cannot be read
    pub async fn read_files_by_paths(self: Arc<Self>, paths: &[String]) -> Result<Vec<Vec<u8>>> {
        use futures::future::join_all;

        let futures = paths.iter().map(|path| {
            let installation = Arc::clone(&self);
            let path = path.clone();
            async move { installation.read_file_by_path(&path).await }
        });

        let results: Result<Vec<_>> = join_all(futures).await.into_iter().collect();

        results
    }

    /// Read multiple files concurrently by `FileDataIDs`
    ///
    /// # Errors
    ///
    /// Returns error if any `FileDataID` cannot be resolved or file cannot be read
    pub async fn read_files_by_fdids(self: Arc<Self>, fdids: &[u32]) -> Result<Vec<Vec<u8>>> {
        use futures::future::join_all;

        let futures = fdids.iter().map(|&fdid| {
            let installation = Arc::clone(&self);
            async move { installation.read_file_by_fdid(fdid).await }
        });

        let results: Result<Vec<_>> = join_all(futures).await.into_iter().collect();

        results
    }

    /// Write a file to storage
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be written or compressed
    pub async fn write_file(&self, data: Vec<u8>, compress: bool) -> Result<ContentKey> {
        debug!(
            "Writing file ({} bytes, compress: {})",
            data.len(),
            compress
        );

        // Calculate content key from uncompressed data
        let content_key = ContentKey::from_data(&data);

        // Write to archive and get location
        let (archive_id, archive_offset, size) = {
            let mut archive_manager = self.archive_manager.write().await;
            archive_manager.write_content(data, compress)?
        };

        // Create encoding key from compressed data location
        let mut location_data = Vec::new();
        location_data.extend_from_slice(&archive_id.to_be_bytes());
        location_data.extend_from_slice(&archive_offset.to_be_bytes());
        let encoding_key = EncodingKey::from_data(&location_data);

        // Update indices
        {
            let mut index_manager = self.index_manager.write().await;
            index_manager.add_entry(&encoding_key, archive_id, archive_offset, size)?;
        }

        // Note: Resolver cache will be updated on next lookup

        info!(
            "Wrote file to archive {} at offset {} (content key: {})",
            archive_id,
            archive_offset,
            hex::encode(content_key.as_bytes())
        );

        Ok(content_key)
    }

    /// Initialize installation by loading local indices and archives
    ///
    /// # Errors
    ///
    /// Returns error if indices cannot be loaded
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing installation at {}", self.path.display());

        // Load local storage index files (.idx in Data/data/)
        self.index_manager.write().await.load_all().await?;

        // Load all archive files (.data in Data/data/)
        self.archive_manager.write().await.open_all().await?;

        info!("Installation initialization complete");
        Ok(())
    }

    /// Load root file for path resolution
    ///
    /// # Errors
    ///
    /// Returns error if root file cannot be loaded
    pub fn load_root_file(&self, data: &[u8]) -> Result<()> {
        self.resolver.load_root_file(data)
    }

    /// Load encoding file for content resolution
    ///
    /// # Errors
    ///
    /// Returns error if encoding file cannot be loaded
    pub fn load_encoding_file(&self, data: &[u8]) -> Result<()> {
        self.resolver.load_encoding_file(data)
    }

    /// Verify installation integrity
    ///
    /// # Errors
    ///
    /// Returns error if verification process fails
    pub async fn verify(&self) -> Result<VerificationResult> {
        info!("Verifying installation integrity");

        let mut result = VerificationResult {
            total: 0,
            valid: 0,
            invalid: 0,
            missing: 0,
        };

        // Verify index files
        let index_stats = {
            let index_manager = self.index_manager.read().await;
            index_manager.stats()
        };

        result.total += index_stats.total_entries;
        result.valid += index_stats.total_entries; // Assume valid if loaded

        // Verify archive files accessibility
        let archive_stats = {
            let archive_manager = self.archive_manager.read().await;
            archive_manager.stats()
        };

        // Add archive count to totals
        result.total += archive_stats.archive_count;
        result.valid += archive_stats.archive_count;

        info!(
            "Verification complete: {} total, {} valid, {} invalid, {} missing",
            result.total, result.valid, result.invalid, result.missing
        );

        Ok(result)
    }

    /// Get file information by path
    ///
    /// # Errors
    ///
    /// Returns error if path cannot be resolved
    pub fn get_file_info(&self, path: &str) -> Result<Option<crate::resolver::FileInfo>> {
        Ok(self.resolver.get_file_info(path))
    }

    /// Get installation statistics
    pub async fn stats(&self) -> InstallationStats {
        let index_stats = {
            let index_manager = self.index_manager.read().await;
            index_manager.stats()
        };

        let archive_stats = {
            let archive_manager = self.archive_manager.read().await;
            archive_manager.stats()
        };

        let resolver_stats = self.resolver.stats();

        InstallationStats {
            path: self.path.clone(),
            index_files: index_stats.index_count,
            index_entries: index_stats.total_entries,
            archive_files: archive_stats.archive_count,
            archive_size: archive_stats.total_size,
            cached_paths: resolver_stats.path_cache_size,
            cached_content: resolver_stats.content_cache_size,
        }
    }

    /// Get the installation path
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Check if a content key exists in local indices
    ///
    /// Note: Local .idx files actually use encoding keys, not content keys.
    /// This method is kept for backward compatibility but `has_encoding_key`
    /// should be preferred for accurate local index lookups.
    pub async fn has_content_key(&self, content_key: &ContentKey) -> bool {
        let index_manager = self.index_manager.read().await;
        index_manager.lookup_by_content_key(content_key).is_some()
    }

    /// Check if an encoding key exists in local indices (.idx files)
    ///
    /// Local .idx files are keyed by encoding keys (truncated to 9 bytes).
    pub async fn has_encoding_key(&self, encoding_key: &EncodingKey) -> bool {
        let index_manager = self.index_manager.read().await;
        index_manager.lookup(encoding_key).is_some()
    }

    /// Get all index entries from the installation
    ///
    /// Returns a vector of all index entries with their encoding keys and archive locations.
    /// This is useful for browsing the installation contents.
    pub async fn get_all_index_entries(&self) -> Vec<crate::index::IndexEntry> {
        let index_manager = self.index_manager.read().await;
        index_manager
            .iter_entries()
            .map(|(_, entry)| entry.clone())
            .collect()
    }

    /// Read raw content from an archive at the specified location
    ///
    /// This is a lower-level method for direct archive access. The data is
    /// automatically decompressed if BLTE-encoded.
    ///
    /// # Errors
    ///
    /// Returns error if archive not found or read fails
    pub async fn read_from_archive(
        &self,
        archive_id: u16,
        offset: u32,
        size: u32,
    ) -> Result<Vec<u8>> {
        let archive_manager = self.archive_manager.read().await;
        archive_manager.read_content(archive_id, offset, size)
    }
}

/// Result of installation verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Total number of files checked
    pub total: usize,
    /// Number of valid files
    pub valid: usize,
    /// Number of invalid/corrupted files
    pub invalid: usize,
    /// Number of missing files
    pub missing: usize,
}

/// Installation statistics for local CASC storage
#[derive(Debug, Clone)]
pub struct InstallationStats {
    /// Installation path
    pub path: PathBuf,
    /// Number of local index files (.idx)
    pub index_files: usize,
    /// Total local index entries
    pub index_entries: usize,
    /// Number of archive files (.data)
    pub archive_files: usize,
    /// Total archive size in bytes
    pub archive_size: u64,
    /// Number of cached path resolutions
    pub cached_paths: usize,
    /// Number of cached content resolutions
    pub cached_content: usize,
}

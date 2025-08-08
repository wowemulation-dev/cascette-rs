//! Async-first index operations for CASC storage
//!
//! This module provides fully async index operations with features like:
//! - Parallel index loading and parsing
//! - Concurrent lookups with read-through caching
//! - Batch operations for efficient bulk processing
//! - Background index updates without blocking reads
//! - Streaming index parsing for large files

use crate::error::{CascError, Result};
use crate::types::{ArchiveLocation, EKey};
use dashmap::DashMap;
use futures::stream::{self, StreamExt};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, trace, warn};

/// Async index configuration
#[derive(Debug, Clone)]
pub struct AsyncIndexConfig {
    /// Maximum concurrent file operations
    pub max_concurrent_files: usize,
    /// Buffer size for reading index files
    pub buffer_size: usize,
    /// Enable read-through caching
    pub enable_caching: bool,
    /// Maximum entries to cache in memory
    pub max_cache_entries: usize,
    /// Enable background index updates
    pub enable_background_updates: bool,
}

impl Default for AsyncIndexConfig {
    fn default() -> Self {
        Self {
            max_concurrent_files: 16,
            buffer_size: 64 * 1024, // 64KB
            enable_caching: true,
            max_cache_entries: 100_000,
            enable_background_updates: true,
        }
    }
}

/// Async index manager for parallel operations
pub struct AsyncIndexManager {
    /// Configuration
    config: AsyncIndexConfig,
    /// Per-bucket indices
    bucket_indices: Arc<DashMap<u8, Arc<AsyncIndex>>>,
    /// Global lookup cache
    lookup_cache: Arc<DashMap<EKey, ArchiveLocation>>,
    /// Semaphore for controlling concurrent operations
    semaphore: Arc<Semaphore>,
    /// Background update handle
    update_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl AsyncIndexManager {
    /// Create a new async index manager
    pub fn new(config: AsyncIndexConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_files));

        Self {
            config,
            bucket_indices: Arc::new(DashMap::new()),
            lookup_cache: Arc::new(DashMap::new()),
            semaphore,
            update_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Load all indices from a directory in parallel
    pub async fn load_directory(&self, path: &Path) -> Result<usize> {
        info!("Loading indices from {:?} with async operations", path);

        // Collect all index files
        let index_files = self.discover_index_files(path).await?;

        if index_files.is_empty() {
            info!("No index files found in {:?}", path);
            return Ok(0);
        }

        info!(
            "Found {} index files, loading in parallel",
            index_files.len()
        );

        // Load all files in parallel with controlled concurrency
        let results = stream::iter(index_files)
            .map(|path| self.load_single_index(path))
            .buffer_unordered(self.config.max_concurrent_files)
            .collect::<Vec<_>>()
            .await;

        // Count successful loads and log errors
        let mut loaded = 0;
        for result in results {
            match result {
                Ok(bucket) => {
                    debug!("Successfully loaded index for bucket {:02x}", bucket);
                    loaded += 1;
                }
                Err(e) => {
                    warn!("Failed to load index: {}", e);
                }
            }
        }

        info!("Successfully loaded {} indices", loaded);
        Ok(loaded)
    }

    /// Discover all index files in a directory
    async fn discover_index_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut index_files = Vec::new();

        // Check both .idx and .index files
        let mut entries = tokio::fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "idx" || ext == "index" {
                    index_files.push(path);
                }
            }
        }

        // Also check subdirectories like data/ and indices/
        for subdir in &["data", "indices"] {
            let subpath = path.join(subdir);
            if subpath.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(&subpath).await {
                    while let Some(entry) = entries.next_entry().await? {
                        let path = entry.path();
                        if let Some(ext) = path.extension() {
                            if ext == "idx" || ext == "index" {
                                index_files.push(path);
                            }
                        }
                    }
                }
            }
        }

        Ok(index_files)
    }

    /// Load a single index file
    async fn load_single_index(&self, path: PathBuf) -> Result<u8> {
        let _permit = self.semaphore.acquire().await.unwrap();

        debug!("Loading index from {:?}", path);

        let index = if path.extension().and_then(|s| s.to_str()) == Some("idx") {
            AsyncIndex::load_idx(&path).await?
        } else {
            AsyncIndex::load_index(&path).await?
        };

        let bucket = index.bucket();
        self.bucket_indices.insert(bucket, Arc::new(index));

        Ok(bucket)
    }

    /// Perform an async lookup
    pub async fn lookup(&self, ekey: &EKey) -> Option<ArchiveLocation> {
        // Check cache first
        if self.config.enable_caching {
            if let Some(location) = self.lookup_cache.get(ekey) {
                trace!("Cache hit for {}", ekey);
                return Some(*location);
            }
        }

        // Check the appropriate bucket
        let bucket = ekey.bucket_index();

        if let Some(index) = self.bucket_indices.get(&bucket) {
            if let Some(location) = index.lookup(ekey).await {
                // Update cache
                if self.config.enable_caching {
                    self.update_cache(*ekey, location);
                }
                return Some(location);
            }
        }

        // Fallback: search all buckets (rare)
        for entry in self.bucket_indices.iter() {
            if let Some(location) = entry.value().lookup(ekey).await {
                // Update cache with correct bucket
                if self.config.enable_caching {
                    self.update_cache(*ekey, location);
                }
                return Some(location);
            }
        }

        None
    }

    /// Batch lookup for multiple keys
    pub async fn lookup_batch(&self, ekeys: &[EKey]) -> Vec<Option<ArchiveLocation>> {
        // Process in parallel for better performance
        let futures = ekeys.iter().map(|ekey| self.lookup(ekey));

        futures::future::join_all(futures).await
    }

    /// Update the lookup cache
    fn update_cache(&self, ekey: EKey, location: ArchiveLocation) {
        // Simple LRU-like behavior: remove oldest if at capacity
        if self.lookup_cache.len() >= self.config.max_cache_entries {
            // Remove a random entry (simple eviction)
            if let Some(entry) = self.lookup_cache.iter().next() {
                self.lookup_cache.remove(entry.key());
            }
        }

        self.lookup_cache.insert(ekey, location);
    }

    /// Start background index updates
    pub async fn start_background_updates(&self, path: PathBuf, interval: std::time::Duration) {
        if !self.config.enable_background_updates {
            return;
        }

        let manager = Arc::new(self.clone_config());

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;

                debug!("Running background index update");

                if let Err(e) = manager.refresh_indices(&path).await {
                    warn!("Background index update failed: {}", e);
                }
            }
        });

        *self.update_handle.write().await = Some(handle);
    }

    /// Refresh indices without blocking reads
    async fn refresh_indices(&self, path: &Path) -> Result<()> {
        // Load new indices in the background
        let index_files = self.discover_index_files(path).await?;

        for file_path in index_files {
            // Load without blocking reads
            if let Ok(index) = self.load_single_index(file_path).await {
                debug!("Refreshed index for bucket {:02x}", index);
            }
        }

        Ok(())
    }

    /// Stop background updates
    pub async fn stop_background_updates(&self) {
        if let Some(handle) = self.update_handle.write().await.take() {
            handle.abort();
        }
    }

    /// Get statistics about loaded indices
    pub async fn get_stats(&self) -> IndexStats {
        let mut total_entries = 0;
        let mut total_buckets = 0;

        for entry in self.bucket_indices.iter() {
            total_buckets += 1;
            total_entries += entry.value().entry_count().await;
        }

        IndexStats {
            total_entries,
            total_buckets,
            cache_size: self.lookup_cache.len(),
            cache_hit_rate: 0.0, // Would need to track hits/misses
        }
    }

    /// Clear all caches
    pub async fn clear_cache(&self) {
        self.lookup_cache.clear();
    }

    /// Clone configuration for background tasks
    fn clone_config(&self) -> Self {
        Self {
            config: self.config.clone(),
            bucket_indices: self.bucket_indices.clone(),
            lookup_cache: self.lookup_cache.clone(),
            semaphore: self.semaphore.clone(),
            update_handle: Arc::new(RwLock::new(None)),
        }
    }
}

/// Individual async index
pub struct AsyncIndex {
    bucket: u8,
    entries: Arc<RwLock<BTreeMap<EKey, ArchiveLocation>>>,
}

impl AsyncIndex {
    /// Create a new async index
    pub fn new(bucket: u8) -> Self {
        Self {
            bucket,
            entries: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Load an .idx file asynchronously
    pub async fn load_idx(path: &Path) -> Result<Self> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);

        // Parse header
        let mut header_buf = vec![0u8; 8];
        reader.read_exact(&mut header_buf).await?;

        // Parse bucket from filename or header
        let bucket = Self::extract_bucket_from_path(path)?;

        let index = Self::new(bucket);

        // Stream parse entries
        index.parse_idx_entries(&mut reader).await?;

        Ok(index)
    }

    /// Load an .index file asynchronously
    pub async fn load_index(path: &Path) -> Result<Self> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);

        let bucket = Self::extract_bucket_from_path(path)?;
        let index = Self::new(bucket);

        // Stream parse entries
        index.parse_index_entries(&mut reader).await?;

        Ok(index)
    }

    /// Parse .idx entries in streaming fashion
    async fn parse_idx_entries(&self, reader: &mut BufReader<File>) -> Result<()> {
        let mut entries = BTreeMap::new();
        let mut buffer = vec![0u8; 4096]; // Read in chunks

        // Skip to data section
        reader.seek(tokio::io::SeekFrom::Start(0x108)).await?;

        while let Ok(n) = reader.read(&mut buffer).await {
            if n == 0 {
                break;
            }

            // Parse entries from buffer
            let mut offset = 0;
            while offset + 25 <= n {
                // 9 bytes key + 16 bytes location
                let key_bytes = &buffer[offset..offset + 9];
                // Create a 16-byte key from 9-byte truncated version
                let mut full_key = [0u8; 16];
                full_key[..9].copy_from_slice(key_bytes);
                let ekey = EKey::new(full_key);

                let archive_id = u16::from_le_bytes([buffer[offset + 9], buffer[offset + 10]]);
                let archive_offset = u32::from_le_bytes([
                    buffer[offset + 11],
                    buffer[offset + 12],
                    buffer[offset + 13],
                    buffer[offset + 14],
                ]);
                let size = u32::from_le_bytes([
                    buffer[offset + 15],
                    buffer[offset + 16],
                    buffer[offset + 17],
                    buffer[offset + 18],
                ]);

                let location = ArchiveLocation {
                    archive_id,
                    offset: archive_offset as u64,
                    size,
                };

                entries.insert(ekey, location);
                offset += 25;
            }
        }

        *self.entries.write().await = entries;
        Ok(())
    }

    /// Parse .index entries in streaming fashion
    async fn parse_index_entries(&self, _reader: &mut BufReader<File>) -> Result<()> {
        // Similar streaming implementation for .index format
        let entries = BTreeMap::new();

        // Implementation would parse the group index format
        // This is a placeholder for the actual parsing logic

        *self.entries.write().await = entries;
        Ok(())
    }

    /// Extract bucket from file path
    fn extract_bucket_from_path(path: &Path) -> Result<u8> {
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| CascError::InvalidIndexFormat("Invalid filename".into()))?;

        // Try to parse bucket from filename (e.g., "00.idx" -> 0x00)
        if filename.len() >= 2 {
            if let Ok(bucket) = u8::from_str_radix(&filename[..2], 16) {
                return Ok(bucket);
            }
        }

        // Default to bucket 0 if can't determine
        Ok(0)
    }

    /// Async lookup
    pub async fn lookup(&self, ekey: &EKey) -> Option<ArchiveLocation> {
        self.entries.read().await.get(ekey).copied()
    }

    /// Get bucket index
    pub fn bucket(&self) -> u8 {
        self.bucket
    }

    /// Get entry count
    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Add an entry (for updates)
    pub async fn add_entry(&self, ekey: EKey, location: ArchiveLocation) {
        self.entries.write().await.insert(ekey, location);
    }

    /// Remove an entry
    pub async fn remove_entry(&self, ekey: &EKey) -> Option<ArchiveLocation> {
        self.entries.write().await.remove(ekey)
    }

    /// Batch add entries
    pub async fn add_entries_batch(&self, entries: Vec<(EKey, ArchiveLocation)>) {
        let mut map = self.entries.write().await;
        for (ekey, location) in entries {
            map.insert(ekey, location);
        }
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_entries: usize,
    pub total_buckets: usize,
    pub cache_size: usize,
    pub cache_hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_index_creation() {
        let index = AsyncIndex::new(0x00);
        assert_eq!(index.bucket(), 0x00);
        assert_eq!(index.entry_count().await, 0);
    }

    #[tokio::test]
    async fn test_async_index_operations() {
        let index = AsyncIndex::new(0x01);

        let mut key_data = [0u8; 16];
        key_data[..9].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let ekey = EKey::new(key_data);
        let location = ArchiveLocation {
            archive_id: 1,
            offset: 100,
            size: 500,
        };

        // Add entry
        index.add_entry(ekey, location).await;
        assert_eq!(index.entry_count().await, 1);

        // Lookup
        let found = index.lookup(&ekey).await;
        assert_eq!(found, Some(location));

        // Remove
        let removed = index.remove_entry(&ekey).await;
        assert_eq!(removed, Some(location));
        assert_eq!(index.entry_count().await, 0);
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let config = AsyncIndexConfig::default();
        let manager = AsyncIndexManager::new(config);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_buckets, 0);
    }

    #[tokio::test]
    async fn test_batch_lookup() {
        let config = AsyncIndexConfig::default();
        let manager = AsyncIndexManager::new(config);

        // Add some test data
        let index = AsyncIndex::new(0x00);
        let mut key1_data = [0u8; 16];
        key1_data[..9].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let ekey1 = EKey::new(key1_data);

        let mut key2_data = [0u8; 16];
        key2_data[..9].copy_from_slice(&[0, 9, 8, 7, 6, 5, 4, 3, 2]);
        let ekey2 = EKey::new(key2_data);

        let location1 = ArchiveLocation {
            archive_id: 1,
            offset: 100,
            size: 200,
        };

        let location2 = ArchiveLocation {
            archive_id: 2,
            offset: 300,
            size: 400,
        };

        index.add_entry(ekey1, location1).await;
        index.add_entry(ekey2, location2).await;

        manager.bucket_indices.insert(0x00, Arc::new(index));

        // Batch lookup
        let results = manager.lookup_batch(&[ekey1, ekey2]).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(location1));
        assert_eq!(results[1], Some(location2));
    }
}

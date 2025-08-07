//! Generic cache implementation for arbitrary data

use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, trace, warn};

use crate::{CacheStats, Result, ensure_dir, get_cache_dir};

/// Cache entry metadata for LRU tracking
#[derive(Debug, Clone)]
struct CacheEntryMetadata {
    /// Last access timestamp
    last_accessed: u64,
    /// File size in bytes
    size: u64,
    /// Access count for statistics
    access_count: u64,
}

/// Generic cache for storing arbitrary data with LRU eviction
pub struct GenericCache {
    /// Base directory for this cache
    base_dir: PathBuf,
    /// LRU tracking metadata
    lru_metadata: Arc<Mutex<HashMap<String, CacheEntryMetadata>>>,
    /// Access order for LRU eviction (most recent at back)
    access_order: Arc<Mutex<VecDeque<String>>>,
    /// Maximum cache size in bytes (None for unlimited)
    max_size_bytes: Option<u64>,
    /// Maximum number of entries (None for unlimited)
    max_entries: Option<usize>,
    /// Current cache size in bytes
    current_size: Arc<Mutex<u64>>,
    /// Cache statistics
    stats: Arc<CacheStats>,
}

impl GenericCache {
    /// Create a new generic cache with the default directory
    pub async fn new() -> Result<Self> {
        Self::with_config(None, None, None).await
    }

    /// Create a new generic cache with a custom subdirectory
    pub async fn with_subdirectory(subdir: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("generic").join(subdir);
        Self::with_config_and_path(base_dir, None, None, None).await
    }

    /// Create a new cache with size and entry limits
    pub async fn with_limits(
        max_size_bytes: Option<u64>,
        max_entries: Option<usize>,
    ) -> Result<Self> {
        Self::with_config(Some("generic"), max_size_bytes, max_entries).await
    }

    /// Create a cache with full configuration
    pub async fn with_config(
        subdir: Option<&str>,
        max_size_bytes: Option<u64>,
        max_entries: Option<usize>,
    ) -> Result<Self> {
        let base_dir = match subdir {
            Some(sub) => get_cache_dir()?.join(sub),
            None => get_cache_dir()?.join("generic"),
        };

        Self::with_config_and_path(base_dir, max_size_bytes, max_entries, None).await
    }

    /// Create a cache with full configuration and custom path
    pub async fn with_config_and_path(
        base_dir: PathBuf,
        max_size_bytes: Option<u64>,
        max_entries: Option<usize>,
        stats: Option<Arc<CacheStats>>,
    ) -> Result<Self> {
        ensure_dir(&base_dir).await?;

        let stats = stats.unwrap_or_else(|| Arc::new(CacheStats::new()));
        let cache = Self {
            base_dir: base_dir.clone(),
            lru_metadata: Arc::new(Mutex::new(HashMap::new())),
            access_order: Arc::new(Mutex::new(VecDeque::new())),
            max_size_bytes,
            max_entries,
            current_size: Arc::new(Mutex::new(0)),
            stats,
        };

        // Initialize cache state by scanning existing files
        cache.initialize_cache_state().await?;

        debug!(
            "Initialized generic cache at: {:?} (max_size: {:?} bytes, max_entries: {:?})",
            base_dir, max_size_bytes, max_entries
        );

        Ok(cache)
    }

    /// Initialize cache state from existing files
    async fn initialize_cache_state(&self) -> Result<()> {
        // Collect file information without holding locks during async operations
        let mut entries = tokio::fs::read_dir(&self.base_dir).await?;
        let mut file_entries = Vec::new();
        let mut total_size = 0u64;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Ok(metadata_fs) = tokio::fs::metadata(&path).await {
                if metadata_fs.is_file() {
                    if let Some(key) = path.file_name().and_then(|n| n.to_str()) {
                        let modified_time = metadata_fs
                            .modified()
                            .unwrap_or(SystemTime::UNIX_EPOCH)
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        file_entries.push((key.to_string(), metadata_fs.len(), modified_time));
                        total_size += metadata_fs.len();
                    }
                }
            }
        }

        // Sort by modification time (oldest first for proper LRU order)
        file_entries.sort_by_key(|(_, _, time)| *time);

        // Now update the internal state with locks held for minimal time
        {
            let mut metadata = self.lru_metadata.lock();
            let mut access_order = self.access_order.lock();
            let mut current_size = self.current_size.lock();

            *current_size = total_size;

            for (key, size, time) in file_entries {
                let entry_metadata = CacheEntryMetadata {
                    last_accessed: time,
                    size,
                    access_count: 0,
                };

                metadata.insert(key.clone(), entry_metadata);
                access_order.push_back(key);
            }

            debug!(
                "Initialized cache with {} entries, total size: {} bytes",
                metadata.len(),
                *current_size
            );
        }

        Ok(())
    }

    /// Get current timestamp in seconds since Unix epoch
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Update access order for LRU tracking
    fn update_access_order(&self, key: &str) {
        let mut access_order = self.access_order.lock();
        let mut metadata = self.lru_metadata.lock();

        // Remove key from current position
        access_order.retain(|k| k != key);

        // Add to back (most recent)
        access_order.push_back(key.to_string());

        // Update access metadata
        if let Some(entry) = metadata.get_mut(key) {
            entry.last_accessed = Self::current_timestamp();
            entry.access_count += 1;
        }
    }

    /// Evict least recently used entries to make space
    async fn evict_if_needed(&self, new_entry_size: u64) -> Result<()> {
        let max_size = self.max_size_bytes;
        let max_entries = self.max_entries;

        if max_size.is_none() && max_entries.is_none() {
            return Ok(()); // No limits set
        }

        let current_size = *self.current_size.lock();
        let current_entries = self.lru_metadata.lock().len();

        // Check if eviction is needed
        let size_exceeded = max_size
            .map(|max| current_size + new_entry_size > max)
            .unwrap_or(false);
        let entries_exceeded = max_entries
            .map(|max| current_entries >= max)
            .unwrap_or(false);

        if !size_exceeded && !entries_exceeded {
            return Ok(());
        }

        debug!(
            "Cache eviction needed: size_exceeded={}, entries_exceeded={}",
            size_exceeded, entries_exceeded
        );

        // Evict entries until we have space
        let mut evicted_count = 0;
        let mut evicted_bytes = 0;

        loop {
            let key_to_evict = {
                let access_order = self.access_order.lock();
                access_order.front().cloned()
            };

            let Some(key) = key_to_evict else { break };

            let entry_size = {
                let metadata = self.lru_metadata.lock();
                metadata.get(&key).map(|e| e.size).unwrap_or(0)
            };

            // Remove the file and metadata
            if let Err(e) = self.evict_entry(&key).await {
                warn!("Failed to evict cache entry '{}': {}", key, e);
                break;
            }

            evicted_count += 1;
            evicted_bytes += entry_size;
            self.stats.record_eviction(entry_size);

            // Check if we have enough space now
            let new_current_size = *self.current_size.lock();
            let new_current_entries = self.lru_metadata.lock().len();

            let size_ok = max_size
                .map(|max| new_current_size + new_entry_size <= max)
                .unwrap_or(true);
            let entries_ok = max_entries
                .map(|max| new_current_entries < max)
                .unwrap_or(true);

            if size_ok && entries_ok {
                break;
            }

            // Safety check to prevent infinite loops
            if evicted_count > 1000 {
                warn!(
                    "Evicted {} entries but still need more space, stopping",
                    evicted_count
                );
                break;
            }
        }

        if evicted_count > 0 {
            debug!(
                "Evicted {} entries ({} bytes)",
                evicted_count, evicted_bytes
            );
        }

        Ok(())
    }

    /// Evict a specific cache entry
    async fn evict_entry(&self, key: &str) -> Result<()> {
        let path = self.get_path(key);

        // Remove from filesystem
        if tokio::fs::metadata(&path).await.is_ok() {
            tokio::fs::remove_file(&path).await?;
        }

        // Remove from tracking structures
        let mut metadata = self.lru_metadata.lock();
        let mut access_order = self.access_order.lock();
        let mut current_size = self.current_size.lock();

        if let Some(entry_metadata) = metadata.remove(key) {
            *current_size = current_size.saturating_sub(entry_metadata.size);
        }

        access_order.retain(|k| k != key);

        trace!("Evicted cache entry: {}", key);
        Ok(())
    }

    /// Get the full path for a cache key
    pub fn get_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(key)
    }

    /// Get cache statistics
    pub fn stats(&self) -> Arc<CacheStats> {
        Arc::clone(&self.stats)
    }

    /// Get current cache size in bytes
    pub fn current_size(&self) -> u64 {
        *self.current_size.lock()
    }

    /// Get current number of entries
    pub fn current_entries(&self) -> usize {
        self.lru_metadata.lock().len()
    }

    /// Get cache configuration
    pub fn config(&self) -> (Option<u64>, Option<usize>) {
        (self.max_size_bytes, self.max_entries)
    }

    /// Check if a cache entry exists
    pub async fn exists(&self, key: &str) -> bool {
        let exists = tokio::fs::metadata(self.get_path(key)).await.is_ok();
        if exists {
            // Update access order for exists check
            self.update_access_order(key);
        }
        exists
    }

    /// Write data to the cache
    pub async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let data_size = data.len() as u64;

        // Check if we need to evict entries to make space
        self.evict_if_needed(data_size).await?;

        let path = self.get_path(key);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        // Check if this is an existing entry (for size tracking)
        let existing_size = {
            let metadata = self.lru_metadata.lock();
            metadata.get(key).map(|e| e.size).unwrap_or(0)
        };

        trace!("Writing {} bytes to cache key: {}", data.len(), key);
        tokio::fs::write(&path, data).await?;

        // Update tracking metadata
        {
            let mut metadata = self.lru_metadata.lock();
            let mut current_size = self.current_size.lock();

            // Update size tracking
            *current_size = current_size.saturating_sub(existing_size) + data_size;

            // Update or create entry metadata
            let entry_metadata = CacheEntryMetadata {
                last_accessed: Self::current_timestamp(),
                size: data_size,
                access_count: 0, // Will be incremented to 1 by update_access_order below
            };
            metadata.insert(key.to_string(), entry_metadata);
        }

        // Update access order
        self.update_access_order(key);

        // Record statistics
        self.stats.record_write(data_size);

        Ok(())
    }

    /// Read data from the cache
    pub async fn read(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.get_path(key);

        trace!("Reading from cache key: {}", key);
        let data = tokio::fs::read(&path).await?;

        // Update access order for cache hit
        self.update_access_order(key);

        // Record cache hit statistics
        self.stats.record_hit(data.len() as u64);

        Ok(data)
    }

    /// Delete a cache entry
    pub async fn delete(&self, key: &str) -> Result<()> {
        let path = self.get_path(key);

        if tokio::fs::metadata(&path).await.is_ok() {
            trace!("Deleting cache key: {}", key);
            tokio::fs::remove_file(&path).await?;
        }

        // Update tracking metadata
        {
            let mut metadata = self.lru_metadata.lock();
            let mut access_order = self.access_order.lock();
            let mut current_size = self.current_size.lock();

            if let Some(entry_metadata) = metadata.remove(key) {
                *current_size = current_size.saturating_sub(entry_metadata.size);
            }

            access_order.retain(|k| k != key);
        }

        // Record delete statistics
        self.stats.record_delete();

        Ok(())
    }

    /// Clear all entries in this cache
    pub async fn clear(&self) -> Result<()> {
        debug!("Clearing all entries in generic cache");

        let mut entries = tokio::fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                if metadata.is_file() {
                    tokio::fs::remove_file(&path).await?;
                }
            }
        }

        // Clear tracking metadata
        {
            let mut metadata = self.lru_metadata.lock();
            let mut access_order = self.access_order.lock();
            let mut current_size = self.current_size.lock();

            metadata.clear();
            access_order.clear();
            *current_size = 0;
        }

        Ok(())
    }

    /// Warm cache with a list of keys by pre-loading them into LRU order
    pub async fn warm_cache(&self, keys: &[String]) -> Result<()> {
        debug!("Warming cache with {} keys", keys.len());

        for key in keys {
            if self.exists(key).await {
                // exists() already updates access order
                trace!("Warmed cache key: {}", key);
            }
        }

        Ok(())
    }

    /// Get LRU ordered list of cache keys (least recently used first)
    pub fn get_lru_keys(&self) -> Vec<String> {
        let access_order = self.access_order.lock();
        access_order.iter().cloned().collect()
    }

    /// Get most recently used keys (up to limit)
    pub fn get_mru_keys(&self, limit: usize) -> Vec<String> {
        let access_order = self.access_order.lock();
        access_order.iter().rev().take(limit).cloned().collect()
    }

    /// Get cache entry metadata
    pub fn get_entry_info(&self, key: &str) -> Option<(u64, u64, u64)> {
        let metadata = self.lru_metadata.lock();
        metadata
            .get(key)
            .map(|e| (e.size, e.last_accessed, e.access_count))
    }

    /// Get the base directory of this cache
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Write multiple entries to the cache in parallel
    ///
    /// This is more efficient than calling write() multiple times sequentially.
    pub async fn write_batch(&self, entries: &[(String, Vec<u8>)]) -> Result<()> {
        use futures::future::try_join_all;

        let futures = entries.iter().map(|(key, data)| self.write(key, data));

        try_join_all(futures).await?;
        Ok(())
    }

    /// Read multiple entries from the cache in parallel
    ///
    /// Returns a vector of results in the same order as the input keys.
    /// Failed reads will be represented as Err values in the vector.
    pub async fn read_batch(&self, keys: &[String]) -> Vec<Result<Vec<u8>>> {
        use futures::future::join_all;

        let futures = keys.iter().map(|key| self.read(key));
        join_all(futures).await
    }

    /// Delete multiple entries from the cache in parallel
    ///
    /// This is more efficient than calling delete() multiple times sequentially.
    pub async fn delete_batch(&self, keys: &[String]) -> Result<()> {
        use futures::future::try_join_all;

        let futures = keys.iter().map(|key| self.delete(key));
        try_join_all(futures).await?;
        Ok(())
    }

    /// Check existence of multiple entries in parallel
    ///
    /// Returns a vector of booleans in the same order as the input keys.
    pub async fn exists_batch(&self, keys: &[String]) -> Vec<bool> {
        use futures::future::join_all;

        let futures = keys.iter().map(|key| self.exists(key));
        join_all(futures).await
    }

    /// Stream data from cache to a writer
    ///
    /// This is more memory-efficient than `read()` for large files.
    pub async fn read_streaming<W>(&self, key: &str, mut writer: W) -> Result<u64>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;

        let path = self.get_path(key);
        trace!("Streaming from cache key: {}", key);

        let mut file = tokio::fs::File::open(&path).await?;
        let bytes_copied = tokio::io::copy(&mut file, &mut writer).await?;
        writer.flush().await?;

        // Update access order and record cache hit
        self.update_access_order(key);
        self.stats.record_hit(bytes_copied);

        Ok(bytes_copied)
    }

    /// Stream data from a reader to cache
    ///
    /// This is more memory-efficient than `write()` for large data.
    pub async fn write_streaming<R>(&self, key: &str, mut reader: R) -> Result<u64>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        use tokio::io::AsyncWriteExt;

        // Check if this is an existing entry (for size tracking)
        let existing_size = {
            let metadata = self.lru_metadata.lock();
            metadata.get(key).map(|e| e.size).unwrap_or(0)
        };

        let path = self.get_path(key);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Streaming to cache key: {}", key);

        let mut file = tokio::fs::File::create(&path).await?;
        let bytes_copied = tokio::io::copy(&mut reader, &mut file).await?;
        file.flush().await?;

        // Check if we need to evict entries after writing
        self.evict_if_needed(0).await?; // Size check after write

        // Update tracking metadata
        {
            let mut metadata = self.lru_metadata.lock();
            let mut current_size = self.current_size.lock();

            // Update size tracking
            *current_size = current_size.saturating_sub(existing_size) + bytes_copied;

            // Update or create entry metadata
            let entry_metadata = CacheEntryMetadata {
                last_accessed: Self::current_timestamp(),
                size: bytes_copied,
                access_count: 0, // Will be incremented to 1 by update_access_order below
            };
            metadata.insert(key.to_string(), entry_metadata);
        }

        // Update access order
        self.update_access_order(key);

        // Record statistics
        self.stats.record_write(bytes_copied);

        Ok(bytes_copied)
    }

    /// Process cache data in chunks without loading it all into memory
    ///
    /// The callback is called for each chunk read from the cache file.
    pub async fn read_chunked<F>(&self, key: &str, mut callback: F) -> Result<u64>
    where
        F: FnMut(&[u8]) -> Result<()>,
    {
        use tokio::io::AsyncReadExt;

        let path = self.get_path(key);
        trace!("Reading cache key in chunks: {}", key);

        let mut file = tokio::fs::File::open(&path).await?;
        let mut buffer = vec![0u8; 8192]; // 8KB chunks
        let mut total_bytes = 0u64;

        loop {
            let bytes_read = file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break; // EOF
            }

            callback(&buffer[..bytes_read])?;
            total_bytes += bytes_read as u64;
        }

        Ok(total_bytes)
    }

    /// Write data to cache in chunks from an iterator
    ///
    /// This allows writing large data without keeping it all in memory.
    pub async fn write_chunked<I>(&self, key: &str, chunks: I) -> Result<u64>
    where
        I: IntoIterator<Item = Result<Vec<u8>>>,
    {
        use tokio::io::AsyncWriteExt;

        let path = self.get_path(key);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing cache key in chunks: {}", key);

        let mut file = tokio::fs::File::create(&path).await?;
        let mut total_bytes = 0u64;

        for chunk_result in chunks {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;
            total_bytes += chunk.len() as u64;
        }

        file.flush().await?;
        Ok(total_bytes)
    }

    /// Copy data between cache entries efficiently
    ///
    /// This is more efficient than read + write for large files.
    pub async fn copy(&self, from_key: &str, to_key: &str) -> Result<u64> {
        use tokio::io::AsyncWriteExt;

        let from_path = self.get_path(from_key);
        let to_path = self.get_path(to_key);

        // Ensure parent directory exists for destination
        if let Some(parent) = to_path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Copying cache from {} to {}", from_key, to_key);

        let mut from_file = tokio::fs::File::open(&from_path).await?;
        let mut to_file = tokio::fs::File::create(&to_path).await?;

        let bytes_copied = tokio::io::copy(&mut from_file, &mut to_file).await?;
        to_file.flush().await?;

        Ok(bytes_copied)
    }

    /// Get the size of a cache entry without reading it
    pub async fn size(&self, key: &str) -> Result<u64> {
        let path = self.get_path(key);
        let metadata = tokio::fs::metadata(&path).await?;

        // Update access order for size check
        self.update_access_order(key);

        Ok(metadata.len())
    }

    /// Stream data from cache with a custom buffer size
    ///
    /// Useful for optimizing I/O based on expected data size.
    pub async fn read_streaming_buffered<W>(
        &self,
        key: &str,
        writer: W,
        buffer_size: usize,
    ) -> Result<u64>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::{AsyncWriteExt, BufWriter};

        let path = self.get_path(key);
        trace!(
            "Streaming from cache key with {}B buffer: {}",
            buffer_size, key
        );

        let file = tokio::fs::File::open(&path).await?;
        let mut reader = tokio::io::BufReader::with_capacity(buffer_size, file);
        let mut writer = BufWriter::with_capacity(buffer_size, writer);

        let bytes_copied = tokio::io::copy(&mut reader, &mut writer).await?;
        writer.flush().await?;

        Ok(bytes_copied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generic_cache_operations() {
        let cache = GenericCache::with_subdirectory("test").await.unwrap();

        // Test write and read
        let key = "test_key";
        let data = b"test data";

        cache.write(key, data).await.unwrap();
        assert!(cache.exists(key).await);

        let read_data = cache.read(key).await.unwrap();
        assert_eq!(read_data, data);

        // Test delete
        cache.delete(key).await.unwrap();
        assert!(!cache.exists(key).await);

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let cache = GenericCache::with_subdirectory("test_batch").await.unwrap();

        // Test batch write
        let entries = vec![
            ("key1".to_string(), b"data1".to_vec()),
            ("key2".to_string(), b"data2".to_vec()),
            ("key3".to_string(), b"data3".to_vec()),
        ];

        cache.write_batch(&entries).await.unwrap();

        // Test batch exists
        let keys = vec![
            "key1".to_string(),
            "key2".to_string(),
            "key3".to_string(),
            "key4".to_string(),
        ];
        let exists = cache.exists_batch(&keys).await;
        assert_eq!(exists, vec![true, true, true, false]);

        // Test batch read
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let results = cache.read_batch(&keys).await;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap(), b"data1");
        assert_eq!(results[1].as_ref().unwrap(), b"data2");
        assert_eq!(results[2].as_ref().unwrap(), b"data3");

        // Test batch delete
        let keys = vec!["key1".to_string(), "key2".to_string()];
        cache.delete_batch(&keys).await.unwrap();
        assert!(!cache.exists("key1").await);
        assert!(!cache.exists("key2").await);
        assert!(cache.exists("key3").await);

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_streaming_operations() {
        let cache = GenericCache::with_subdirectory("test_streaming")
            .await
            .unwrap();

        // Test streaming write
        let key = "streaming_test";
        let test_data = b"Hello, streaming world! This is a test of streaming I/O operations.";
        let mut reader = std::io::Cursor::new(test_data);

        let bytes_written = cache.write_streaming(key, &mut reader).await.unwrap();
        assert_eq!(bytes_written, test_data.len() as u64);
        assert!(cache.exists(key).await);

        // Test streaming read
        let mut output = Vec::new();
        let bytes_read = cache.read_streaming(key, &mut output).await.unwrap();
        assert_eq!(bytes_read, test_data.len() as u64);
        assert_eq!(output, test_data);

        // Test size
        let size = cache.size(key).await.unwrap();
        assert_eq!(size, test_data.len() as u64);

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_chunked_operations() {
        let cache = GenericCache::with_subdirectory("test_chunked")
            .await
            .unwrap();

        // Test chunked write
        let key = "chunked_test";
        let chunks = vec![
            Ok(b"chunk1".to_vec()),
            Ok(b"chunk2".to_vec()),
            Ok(b"chunk3".to_vec()),
        ];

        let bytes_written = cache.write_chunked(key, chunks).await.unwrap();
        assert_eq!(bytes_written, 18); // 6 + 6 + 6 bytes
        assert!(cache.exists(key).await);

        // Test chunked read
        let mut collected_data = Vec::new();
        let bytes_read = cache
            .read_chunked(key, |chunk| {
                collected_data.extend_from_slice(chunk);
                Ok(())
            })
            .await
            .unwrap();

        assert_eq!(bytes_read, 18);
        assert_eq!(collected_data, b"chunk1chunk2chunk3");

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_copy_operation() {
        let cache = GenericCache::with_subdirectory("test_copy").await.unwrap();

        // Create source data
        let source_key = "source";
        let dest_key = "destination";
        let test_data = b"This data will be copied between cache entries";

        cache.write(source_key, test_data).await.unwrap();

        // Test copy
        let bytes_copied = cache.copy(source_key, dest_key).await.unwrap();
        assert_eq!(bytes_copied, test_data.len() as u64);

        // Verify both entries exist and have same content
        assert!(cache.exists(source_key).await);
        assert!(cache.exists(dest_key).await);

        let source_data = cache.read(source_key).await.unwrap();
        let dest_data = cache.read(dest_key).await.unwrap();
        assert_eq!(source_data, dest_data);
        assert_eq!(source_data, test_data);

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_buffered_streaming() {
        let cache = GenericCache::with_subdirectory("test_buffered")
            .await
            .unwrap();

        // Create test data
        let key = "buffered_test";
        let test_data = vec![42u8; 16384]; // 16KB of data

        cache.write(key, &test_data).await.unwrap();

        // Test buffered streaming with custom buffer size
        let mut output = Vec::new();
        let bytes_read = cache
            .read_streaming_buffered(key, &mut output, 4096)
            .await
            .unwrap();

        assert_eq!(bytes_read, test_data.len() as u64);
        assert_eq!(output, test_data);

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_large_file_streaming() {
        let cache = GenericCache::with_subdirectory("test_large").await.unwrap();

        // Create a larger test file (1MB)
        let key = "large_test";
        let chunk_size = 8192;
        let num_chunks = 128; // 128 * 8192 = 1MB

        // Write in chunks
        let chunks: Vec<Result<Vec<u8>>> = (0..num_chunks)
            .map(|i| Ok(vec![(i % 256) as u8; chunk_size]))
            .collect();

        let bytes_written = cache.write_chunked(key, chunks).await.unwrap();
        assert_eq!(bytes_written, (chunk_size * num_chunks) as u64);

        // Read back in chunks and verify
        let mut total_read = 0u64;
        let mut chunk_count = 0;

        cache
            .read_chunked(key, |chunk| {
                total_read += chunk.len() as u64;
                chunk_count += 1;
                Ok(())
            })
            .await
            .unwrap();

        assert_eq!(total_read, bytes_written);
        assert!(chunk_count > 0); // Should be multiple chunks due to 8KB buffer

        // Cleanup
        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_lru_eviction_by_size() {
        // Create cache with 1KB limit
        let cache = GenericCache::with_config_and_path(
            get_cache_dir().unwrap().join("test_lru_eviction_by_size"),
            Some(1024),
            None,
            None,
        ).await.unwrap();

        // Write 3 entries of 400 bytes each (1200 bytes total)
        let data_400b = vec![42u8; 400];
        cache.write("key1", &data_400b).await.unwrap();
        cache.write("key2", &data_400b).await.unwrap();
        cache.write("key3", &data_400b).await.unwrap(); // Should evict key1

        // key1 should be evicted, key2 and key3 should exist
        assert!(!cache.exists("key1").await);
        assert!(cache.exists("key2").await);
        assert!(cache.exists("key3").await);

        // Check that cache size is within limits
        assert!(cache.current_size() <= 1024);
        assert_eq!(cache.current_entries(), 2);

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_lru_eviction_by_entries() {
        // Create cache with 2 entry limit
        let cache = GenericCache::with_config_and_path(
            get_cache_dir().unwrap().join("test_lru_eviction_by_entries"),
            None,
            Some(2),
            None,
        ).await.unwrap();

        // Write 3 entries
        cache.write("key1", b"data1").await.unwrap();
        cache.write("key2", b"data2").await.unwrap();
        cache.write("key3", b"data3").await.unwrap(); // Should evict key1

        // key1 should be evicted, key2 and key3 should exist
        assert!(!cache.exists("key1").await);
        assert!(cache.exists("key2").await);
        assert!(cache.exists("key3").await);
        assert_eq!(cache.current_entries(), 2);

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_lru_access_order_update() {
        let cache = GenericCache::with_config_and_path(
            get_cache_dir().unwrap().join("test_lru_access_order_update"),
            None,
            Some(2),
            None,
        ).await.unwrap();

        // Write 2 entries
        cache.write("key1", b"data1").await.unwrap();
        cache.write("key2", b"data2").await.unwrap();

        // Access key1 to make it more recently used
        let _ = cache.read("key1").await.unwrap();

        // Write key3, which should evict key2 (least recently used)
        cache.write("key3", b"data3").await.unwrap();

        // key1 and key3 should exist, key2 should be evicted
        // Use file system check instead of exists() to avoid modifying access order
        let key1_path = cache.get_path("key1");
        let key2_path = cache.get_path("key2");
        let key3_path = cache.get_path("key3");

        assert!(tokio::fs::metadata(key1_path).await.is_ok());
        assert!(tokio::fs::metadata(key2_path).await.is_err());
        assert!(tokio::fs::metadata(key3_path).await.is_ok());

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_cache_statistics_integration() {
        let cache = GenericCache::with_config_and_path(
            get_cache_dir().unwrap().join("test_cache_statistics_integration"),
            None,
            Some(2),
            None,
        ).await.unwrap();
        let stats = cache.stats();

        // Write some data
        cache.write("key1", b"data1").await.unwrap();
        assert_eq!(stats.bytes_written(), 5);

        // Read data (cache hit)
        let _ = cache.read("key1").await.unwrap();
        assert_eq!(stats.hits(), 1);
        assert_eq!(stats.bytes_saved(), 5);

        // Try to read non-existent key (this will be a filesystem miss, not cache miss)
        // Our cache doesn't track misses from read attempts, only from business logic

        // Delete entry
        cache.delete("key1").await.unwrap();

        // Verify statistics through snapshot
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.write_operations, 1);
        assert_eq!(snapshot.read_operations, 1);
        assert_eq!(snapshot.delete_operations, 1);

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_cache_warming() {
        let cache = GenericCache::with_subdirectory("test_warm").await.unwrap();

        // Create some entries
        cache.write("key1", b"data1").await.unwrap();
        cache.write("key2", b"data2").await.unwrap();
        cache.write("key3", b"data3").await.unwrap();

        // Clear access order (simulate cache restart)
        {
            let mut access_order = cache.access_order.lock();
            access_order.clear();
        }

        // Warm cache with specific keys
        let warm_keys = vec!["key2".to_string(), "key1".to_string()];
        cache.warm_cache(&warm_keys).await.unwrap();

        // Check LRU order - key2 should be least recently used, key1 most recent
        let lru_keys = cache.get_lru_keys();
        assert!(lru_keys.contains(&"key1".to_string()));
        assert!(lru_keys.contains(&"key2".to_string()));

        let mru_keys = cache.get_mru_keys(1);
        assert_eq!(mru_keys[0], "key1"); // Most recently accessed

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_entry_metadata() {
        let cache = GenericCache::with_subdirectory("test_metadata")
            .await
            .unwrap();

        // Write an entry
        cache.write("test_key", b"test_data").await.unwrap();

        // Get entry info
        let (size, last_accessed, access_count) = cache.get_entry_info("test_key").unwrap();
        assert_eq!(size, 9); // "test_data" is 9 bytes
        assert!(last_accessed > 0); // Should have a timestamp
        assert_eq!(access_count, 1); // Written once (access count starts at 1)

        // Access the entry
        let _ = cache.read("test_key").await.unwrap();

        // Check updated metadata
        let (_, _, access_count) = cache.get_entry_info("test_key").unwrap();
        assert!(access_count >= 2); // Written once, read once (may be higher due to exists() calls)

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_cache_size_tracking() {
        let cache = GenericCache::with_subdirectory("test_size").await.unwrap();

        assert_eq!(cache.current_size(), 0);
        assert_eq!(cache.current_entries(), 0);

        // Write entries
        cache.write("key1", b"hello").await.unwrap(); // 5 bytes
        assert_eq!(cache.current_size(), 5);
        assert_eq!(cache.current_entries(), 1);

        cache.write("key2", b"world!").await.unwrap(); // 6 bytes
        assert_eq!(cache.current_size(), 11);
        assert_eq!(cache.current_entries(), 2);

        // Overwrite existing entry
        cache.write("key1", b"hello world").await.unwrap(); // 11 bytes
        assert_eq!(cache.current_size(), 17); // 11 + 6 bytes
        assert_eq!(cache.current_entries(), 2);

        // Delete entry
        cache.delete("key2").await.unwrap();
        assert_eq!(cache.current_size(), 11); // Only key1 remains
        assert_eq!(cache.current_entries(), 1);

        let _ = cache.clear().await;
    }

    #[tokio::test]
    async fn test_no_limits_cache() {
        // Test cache with no size or entry limits
        let cache = GenericCache::with_subdirectory("test_no_limits")
            .await
            .unwrap();
        let (max_size, max_entries) = cache.config();

        assert_eq!(max_size, None);
        assert_eq!(max_entries, None);

        // Clear any existing entries first
        let _ = cache.clear().await;
        assert_eq!(cache.current_entries(), 0);

        // Should be able to write many entries without eviction
        for i in 0..100 {
            let key = format!("key_{i}");
            let data = format!("data_{i}");
            cache.write(&key, data.as_bytes()).await.unwrap();
        }

        assert_eq!(cache.current_entries(), 100);

        let _ = cache.clear().await;
    }
}

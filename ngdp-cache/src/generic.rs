//! Generic cache implementation for arbitrary data

use std::path::{Path, PathBuf};
use tracing::{debug, trace};

use crate::{Result, ensure_dir, get_cache_dir};

/// Generic cache for storing arbitrary data
pub struct GenericCache {
    /// Base directory for this cache
    base_dir: PathBuf,
}

impl GenericCache {
    /// Create a new generic cache with the default directory
    pub async fn new() -> Result<Self> {
        let base_dir = get_cache_dir()?.join("generic");
        ensure_dir(&base_dir).await?;

        debug!("Initialized generic cache at: {:?}", base_dir);

        Ok(Self { base_dir })
    }

    /// Create a new generic cache with a custom subdirectory
    pub async fn with_subdirectory(subdir: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("generic").join(subdir);
        ensure_dir(&base_dir).await?;

        debug!("Initialized generic cache at: {:?}", base_dir);

        Ok(Self { base_dir })
    }

    /// Get the full path for a cache key
    pub fn get_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(key)
    }

    /// Check if a cache entry exists
    pub async fn exists(&self, key: &str) -> bool {
        tokio::fs::metadata(self.get_path(key)).await.is_ok()
    }

    /// Write data to the cache
    pub async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.get_path(key);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to cache key: {}", data.len(), key);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Read data from the cache
    pub async fn read(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.get_path(key);

        trace!("Reading from cache key: {}", key);
        let data = tokio::fs::read(&path).await?;

        Ok(data)
    }

    /// Delete a cache entry
    pub async fn delete(&self, key: &str) -> Result<()> {
        let path = self.get_path(key);

        if tokio::fs::metadata(&path).await.is_ok() {
            trace!("Deleting cache key: {}", key);
            tokio::fs::remove_file(&path).await?;
        }

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

        Ok(())
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
        
        let futures = entries.iter().map(|(key, data)| {
            self.write(key, data)
        });
        
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
        
        let path = self.get_path(key);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Streaming to cache key: {}", key);
        
        let mut file = tokio::fs::File::create(&path).await?;
        let bytes_copied = tokio::io::copy(&mut reader, &mut file).await?;
        file.flush().await?;
        
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
        Ok(metadata.len())
    }

    /// Stream data from cache with a custom buffer size
    ///
    /// Useful for optimizing I/O based on expected data size.
    pub async fn read_streaming_buffered<W>(
        &self, 
        key: &str, 
        writer: W, 
        buffer_size: usize
    ) -> Result<u64>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::{AsyncWriteExt, BufWriter};
        
        let path = self.get_path(key);
        trace!("Streaming from cache key with {}B buffer: {}", buffer_size, key);
        
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
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string(), "key4".to_string()];
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
        let cache = GenericCache::with_subdirectory("test_streaming").await.unwrap();

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
        let cache = GenericCache::with_subdirectory("test_chunked").await.unwrap();

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
        let bytes_read = cache.read_chunked(key, |chunk| {
            collected_data.extend_from_slice(chunk);
            Ok(())
        }).await.unwrap();
        
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
        let cache = GenericCache::with_subdirectory("test_buffered").await.unwrap();

        // Create test data
        let key = "buffered_test";
        let test_data = vec![42u8; 16384]; // 16KB of data
        
        cache.write(key, &test_data).await.unwrap();
        
        // Test buffered streaming with custom buffer size
        let mut output = Vec::new();
        let bytes_read = cache.read_streaming_buffered(key, &mut output, 4096).await.unwrap();
        
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
        
        cache.read_chunked(key, |chunk| {
            total_read += chunk.len() as u64;
            chunk_count += 1;
            Ok(())
        }).await.unwrap();
        
        assert_eq!(total_read, bytes_written);
        assert!(chunk_count > 0); // Should be multiple chunks due to 8KB buffer

        // Cleanup
        let _ = cache.clear().await;
    }
}

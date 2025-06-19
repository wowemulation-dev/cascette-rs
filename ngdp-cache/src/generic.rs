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
        self.get_path(key).exists()
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

        if path.exists() {
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
            if path.is_file() {
                tokio::fs::remove_file(&path).await?;
            }
        }

        Ok(())
    }

    /// Get the base directory of this cache
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
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
}

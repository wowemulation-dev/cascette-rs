//! TACT protocol cache implementation

use std::path::PathBuf;
use tracing::{debug, trace};

use crate::{Result, ensure_dir, get_cache_dir};

/// Cache for TACT protocol data (configs, indices, etc.)
pub struct TactCache {
    /// Base directory for TACT cache
    base_dir: PathBuf,
}

impl TactCache {
    /// Create a new TACT cache
    pub async fn new() -> Result<Self> {
        let base_dir = get_cache_dir()?.join("tact");
        ensure_dir(&base_dir).await?;

        debug!("Initialized TACT cache at: {:?}", base_dir);

        Ok(Self { base_dir })
    }

    /// Get the config cache directory
    pub fn config_dir(&self) -> PathBuf {
        self.base_dir.join("config")
    }

    /// Get the data cache directory
    pub fn data_dir(&self) -> PathBuf {
        self.base_dir.join("data")
    }

    /// Get the patch cache directory
    pub fn patch_dir(&self) -> PathBuf {
        self.base_dir.join("patch")
    }

    /// Construct a config cache path from a hash
    ///
    /// Follows CDN structure: config/{first2}/{next2}/{hash}
    pub fn config_path(&self, hash: &str) -> PathBuf {
        if hash.len() >= 4 {
            self.config_dir()
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash)
        } else {
            self.config_dir().join(hash)
        }
    }

    /// Construct a data cache path from a hash
    ///
    /// Follows CDN structure: data/{first2}/{next2}/{hash}
    pub fn data_path(&self, hash: &str) -> PathBuf {
        if hash.len() >= 4 {
            self.data_dir()
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash)
        } else {
            self.data_dir().join(hash)
        }
    }

    /// Construct an index cache path from a hash
    ///
    /// Follows CDN structure: data/{first2}/{next2}/{hash}.index
    pub fn index_path(&self, hash: &str) -> PathBuf {
        let mut path = self.data_path(hash);
        path.set_extension("index");
        path
    }

    /// Check if a config exists in cache
    pub async fn has_config(&self, hash: &str) -> bool {
        self.config_path(hash).exists()
    }

    /// Check if data exists in cache
    pub async fn has_data(&self, hash: &str) -> bool {
        self.data_path(hash).exists()
    }

    /// Check if an index exists in cache
    pub async fn has_index(&self, hash: &str) -> bool {
        self.index_path(hash).exists()
    }

    /// Write config data to cache
    pub async fn write_config(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.config_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to config cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Write data to cache
    pub async fn write_data(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.data_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to data cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Write index to cache
    pub async fn write_index(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.index_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to index cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Read config from cache
    pub async fn read_config(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.config_path(hash);
        trace!("Reading config from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read data from cache
    pub async fn read_data(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.data_path(hash);
        trace!("Reading data from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read index from cache
    pub async fn read_index(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.index_path(hash);
        trace!("Reading index from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Get the base directory of this cache
    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tact_cache_paths() {
        let cache = TactCache::new().await.unwrap();

        // Test path construction
        let hash = "abcdef1234567890abcdef1234567890";

        let config_path = cache.config_path(hash);
        assert!(config_path.to_str().unwrap().contains("config/ab/cd"));

        let data_path = cache.data_path(hash);
        assert!(data_path.to_str().unwrap().contains("data/ab/cd"));

        let index_path = cache.index_path(hash);
        assert!(index_path.to_str().unwrap().ends_with(".index"));
    }

    #[tokio::test]
    async fn test_tact_cache_operations() {
        let cache = TactCache::new().await.unwrap();
        let hash = "test1234567890abcdef1234567890ab";
        let data = b"test config data";

        // Write and read config
        cache.write_config(hash, data).await.unwrap();
        assert!(cache.has_config(hash).await);

        let read_data = cache.read_config(hash).await.unwrap();
        assert_eq!(read_data, data);

        // Cleanup
        let _ = tokio::fs::remove_file(cache.config_path(hash)).await;
    }
}

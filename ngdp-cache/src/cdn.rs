//! CDN content cache implementation
//!
//! This module caches all CDN content following the CDN path structure:
//! - `{cdn_path}/config/{first2}/{next2}/{hash}` - Configuration files
//! - `{cdn_path}/data/{first2}/{next2}/{hash}` - Data files and archives
//! - `{cdn_path}/patch/{first2}/{next2}/{hash}` - Patch files
//!
//! Where `{cdn_path}` is the path provided by the CDN (e.g., "tpr/wow").
//! Archives and indices are stored in the data directory with `.index` extension for indices.

use std::path::PathBuf;
use tracing::{debug, trace};

use crate::{Result, ensure_dir, get_cache_dir};

/// Cache for CDN content following the standard CDN directory structure
pub struct CdnCache {
    /// Base directory for CDN cache
    base_dir: PathBuf,
    /// CDN path prefix (e.g., "tpr/wow")
    cdn_path: Option<String>,
}

impl CdnCache {
    /// Create a new CDN cache
    pub async fn new() -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn");
        ensure_dir(&base_dir).await?;

        debug!("Initialized CDN cache at: {:?}", base_dir);

        Ok(Self {
            base_dir,
            cdn_path: None,
        })
    }

    /// Create a CDN cache for a specific product
    pub async fn for_product(product: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn").join(product);
        ensure_dir(&base_dir).await?;

        debug!(
            "Initialized CDN cache for product '{}' at: {:?}",
            product, base_dir
        );

        Ok(Self {
            base_dir,
            cdn_path: None,
        })
    }

    /// Create a CDN cache with a custom base directory
    pub async fn with_base_dir(base_dir: PathBuf) -> Result<Self> {
        ensure_dir(&base_dir).await?;

        debug!("Initialized CDN cache at: {:?}", base_dir);

        Ok(Self {
            base_dir,
            cdn_path: None,
        })
    }

    /// Create a CDN cache with a specific CDN path
    pub async fn with_cdn_path(cdn_path: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn");
        ensure_dir(&base_dir).await?;

        debug!(
            "Initialized CDN cache with path '{}' at: {:?}",
            cdn_path, base_dir
        );

        Ok(Self {
            base_dir,
            cdn_path: Some(cdn_path.to_string()),
        })
    }

    /// Set the CDN path for this cache
    pub fn set_cdn_path(&mut self, cdn_path: Option<String>) {
        self.cdn_path = cdn_path;
    }

    /// Get the effective base directory including CDN path
    fn effective_base_dir(&self) -> PathBuf {
        if let Some(ref cdn_path) = self.cdn_path {
            self.base_dir.join(cdn_path)
        } else {
            self.base_dir.clone()
        }
    }

    /// Get the config cache directory
    pub fn config_dir(&self) -> PathBuf {
        let base = self.effective_base_dir();
        let path_str = base.to_string_lossy();

        // Check if the path already ends with "config" or contains "configs"
        if path_str.ends_with("/config") || path_str.ends_with("\\config") {
            // Path already has /config suffix, don't add another
            base
        } else if path_str.contains("configs/") || path_str.contains("configs\\") {
            // For paths like "tpr/configs/data", don't add "config"
            base
        } else {
            // For paths like "tpr/wow", add "config"
            base.join("config")
        }
    }

    /// Get the data cache directory
    pub fn data_dir(&self) -> PathBuf {
        self.effective_base_dir().join("data")
    }

    /// Get the patch cache directory
    pub fn patch_dir(&self) -> PathBuf {
        self.effective_base_dir().join("patch")
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

    /// Construct a patch cache path from a hash
    ///
    /// Follows CDN structure: patch/{first2}/{next2}/{hash}
    pub fn patch_path(&self, hash: &str) -> PathBuf {
        if hash.len() >= 4 {
            self.patch_dir()
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash)
        } else {
            self.patch_dir().join(hash)
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

    /// Check if a patch exists in cache
    pub async fn has_patch(&self, hash: &str) -> bool {
        self.patch_path(hash).exists()
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

    /// Write patch data to cache
    pub async fn write_patch(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.patch_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to patch cache: {}", data.len(), hash);
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

    /// Read patch from cache
    pub async fn read_patch(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.patch_path(hash);
        trace!("Reading patch from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read index from cache
    pub async fn read_index(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.index_path(hash);
        trace!("Reading index from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Stream read data from cache
    ///
    /// Returns a file handle for efficient streaming
    pub async fn open_data(&self, hash: &str) -> Result<tokio::fs::File> {
        let path = self.data_path(hash);
        trace!("Opening data for streaming: {}", hash);
        Ok(tokio::fs::File::open(&path).await?)
    }

    /// Get data size without reading it
    pub async fn data_size(&self, hash: &str) -> Result<u64> {
        let path = self.data_path(hash);
        let metadata = tokio::fs::metadata(&path).await?;
        Ok(metadata.len())
    }

    /// Get the base directory of this cache
    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    /// Get the CDN path if set
    pub fn cdn_path(&self) -> Option<&str> {
        self.cdn_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cdn_cache_paths() {
        let cache = CdnCache::new().await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let config_path = cache.config_path(hash);
        assert!(config_path.to_str().unwrap().contains("config/de/ad"));

        let data_path = cache.data_path(hash);
        assert!(data_path.to_str().unwrap().contains("data/de/ad"));

        let patch_path = cache.patch_path(hash);
        assert!(patch_path.to_str().unwrap().contains("patch/de/ad"));

        let index_path = cache.index_path(hash);
        assert!(index_path.to_str().unwrap().contains("data/de/ad"));
        assert!(index_path.to_str().unwrap().ends_with(".index"));
    }

    #[tokio::test]
    async fn test_cdn_cache_with_cdn_path() {
        let cache = CdnCache::with_cdn_path("tpr/wow").await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let config_path = cache.config_path(hash);
        assert!(
            config_path
                .to_str()
                .unwrap()
                .contains("tpr/wow/config/de/ad")
        );

        let data_path = cache.data_path(hash);
        assert!(data_path.to_str().unwrap().contains("tpr/wow/data/de/ad"));

        let patch_path = cache.patch_path(hash);
        assert!(patch_path.to_str().unwrap().contains("tpr/wow/patch/de/ad"));
    }

    #[tokio::test]
    async fn test_cdn_product_cache() {
        let cache = CdnCache::for_product("wow").await.unwrap();
        assert!(cache.base_dir().to_str().unwrap().contains("cdn/wow"));
    }

    #[tokio::test]
    async fn test_cdn_cache_operations() {
        let cache = CdnCache::for_product("test").await.unwrap();
        let hash = "test5678901234567890abcdef123456";
        let data = b"test data content";

        // Write and read data
        cache.write_data(hash, data).await.unwrap();
        assert!(cache.has_data(hash).await);

        let read_data = cache.read_data(hash).await.unwrap();
        assert_eq!(read_data, data);

        // Test size
        let size = cache.data_size(hash).await.unwrap();
        assert_eq!(size, data.len() as u64);

        // Test config
        let config_data = b"test config data";
        cache.write_config(hash, config_data).await.unwrap();
        assert!(cache.has_config(hash).await);

        let read_config = cache.read_config(hash).await.unwrap();
        assert_eq!(read_config, config_data);

        // Cleanup
        let _ = tokio::fs::remove_file(cache.data_path(hash)).await;
        let _ = tokio::fs::remove_file(cache.config_path(hash)).await;
    }
}

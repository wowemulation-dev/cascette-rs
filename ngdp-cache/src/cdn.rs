//! CDN content cache implementation

use std::path::PathBuf;
use tracing::{debug, trace};

use crate::{Result, ensure_dir, get_cache_dir};

/// Cache for CDN content (archives, loose files, etc.)
pub struct CdnCache {
    /// Base directory for CDN cache
    base_dir: PathBuf,
}

impl CdnCache {
    /// Create a new CDN cache
    pub async fn new() -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn");
        ensure_dir(&base_dir).await?;

        debug!("Initialized CDN cache at: {:?}", base_dir);

        Ok(Self { base_dir })
    }

    /// Create a CDN cache for a specific product
    pub async fn for_product(product: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn").join(product);
        ensure_dir(&base_dir).await?;

        debug!(
            "Initialized CDN cache for product '{}' at: {:?}",
            product, base_dir
        );

        Ok(Self { base_dir })
    }

    /// Get the archives directory
    pub fn archives_dir(&self) -> PathBuf {
        self.base_dir.join("archives")
    }

    /// Get the loose files directory
    pub fn loose_dir(&self) -> PathBuf {
        self.base_dir.join("loose")
    }

    /// Construct an archive path from a hash
    ///
    /// Archives are stored as: archives/{first2}/{next2}/{hash}
    pub fn archive_path(&self, hash: &str) -> PathBuf {
        if hash.len() >= 4 {
            self.archives_dir()
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash)
        } else {
            self.archives_dir().join(hash)
        }
    }

    /// Construct a loose file path from a hash
    ///
    /// Loose files are stored as: loose/{first2}/{next2}/{hash}
    pub fn loose_path(&self, hash: &str) -> PathBuf {
        if hash.len() >= 4 {
            self.loose_dir()
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash)
        } else {
            self.loose_dir().join(hash)
        }
    }

    /// Check if an archive exists in cache
    pub async fn has_archive(&self, hash: &str) -> bool {
        self.archive_path(hash).exists()
    }

    /// Check if a loose file exists in cache
    pub async fn has_loose(&self, hash: &str) -> bool {
        self.loose_path(hash).exists()
    }

    /// Write archive data to cache
    pub async fn write_archive(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.archive_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to archive cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Write loose file to cache
    pub async fn write_loose(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.loose_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to loose file cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Read archive from cache
    pub async fn read_archive(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.archive_path(hash);
        trace!("Reading archive from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read loose file from cache
    pub async fn read_loose(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.loose_path(hash);
        trace!("Reading loose file from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Stream read an archive from cache
    ///
    /// Returns a file handle for efficient streaming
    pub async fn open_archive(&self, hash: &str) -> Result<tokio::fs::File> {
        let path = self.archive_path(hash);
        trace!("Opening archive for streaming: {}", hash);
        Ok(tokio::fs::File::open(&path).await?)
    }

    /// Get archive size without reading it
    pub async fn archive_size(&self, hash: &str) -> Result<u64> {
        let path = self.archive_path(hash);
        let metadata = tokio::fs::metadata(&path).await?;
        Ok(metadata.len())
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
    async fn test_cdn_cache_paths() {
        let cache = CdnCache::new().await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let archive_path = cache.archive_path(hash);
        assert!(archive_path.to_str().unwrap().contains("archives/de/ad"));

        let loose_path = cache.loose_path(hash);
        assert!(loose_path.to_str().unwrap().contains("loose/de/ad"));
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
        let data = b"test archive data";

        // Write and read archive
        cache.write_archive(hash, data).await.unwrap();
        assert!(cache.has_archive(hash).await);

        let read_data = cache.read_archive(hash).await.unwrap();
        assert_eq!(read_data, data);

        // Test size
        let size = cache.archive_size(hash).await.unwrap();
        assert_eq!(size, data.len() as u64);

        // Cleanup
        let _ = tokio::fs::remove_file(cache.archive_path(hash)).await;
    }
}

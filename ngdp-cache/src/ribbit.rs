//! Ribbit response cache implementation

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, trace};

use crate::{Error, Result, ensure_dir, get_cache_dir};

/// Cache for Ribbit protocol responses
pub struct RibbitCache {
    /// Base directory for Ribbit cache
    base_dir: PathBuf,
    /// Default TTL for cached responses (in seconds)
    default_ttl: Duration,
}

impl RibbitCache {
    /// Create a new Ribbit cache with default TTL of 5 minutes
    pub async fn new() -> Result<Self> {
        Self::with_ttl(Duration::from_secs(300)).await
    }

    /// Create a new Ribbit cache with custom TTL
    pub async fn with_ttl(ttl: Duration) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("ribbit");
        ensure_dir(&base_dir).await?;

        debug!(
            "Initialized Ribbit cache at: {:?} with TTL: {:?}",
            base_dir, ttl
        );

        Ok(Self {
            base_dir,
            default_ttl: ttl,
        })
    }

    /// Get cache path for a specific endpoint
    pub fn cache_path(&self, region: &str, product: &str, endpoint: &str) -> PathBuf {
        self.base_dir.join(region).join(product).join(endpoint)
    }

    /// Get metadata path for cache entry
    pub fn metadata_path(&self, region: &str, product: &str, endpoint: &str) -> PathBuf {
        let mut path = self.cache_path(region, product, endpoint);
        path.set_extension("meta");
        path
    }

    /// Check if a cache entry exists and is still valid
    pub async fn is_valid(&self, region: &str, product: &str, endpoint: &str) -> bool {
        let meta_path = self.metadata_path(region, product, endpoint);

        if let Ok(metadata) = tokio::fs::read_to_string(&meta_path).await {
            if let Ok(timestamp) = metadata.trim().parse::<u64>() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                return (now - timestamp) < self.default_ttl.as_secs();
            }
        }

        false
    }

    /// Write response to cache
    pub async fn write(
        &self,
        region: &str,
        product: &str,
        endpoint: &str,
        data: &[u8],
    ) -> Result<()> {
        let path = self.cache_path(region, product, endpoint);
        let meta_path = self.metadata_path(region, product, endpoint);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        // Use temporary files for atomic writes in the same directory
        let temp_path = path.with_file_name(format!(
            "{}.tmp",
            path.file_name().unwrap().to_string_lossy()
        ));
        let temp_meta_path = meta_path.with_file_name(format!(
            "{}.tmp",
            meta_path.file_name().unwrap().to_string_lossy()
        ));

        // Get timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Write data to temporary file first
        trace!(
            "Writing {} bytes to Ribbit cache: {}/{}/{}",
            data.len(),
            region,
            product,
            endpoint
        );

        // Handle errors and cleanup temporary files
        let write_result = async {
            tokio::fs::write(&temp_path, data).await?;
            tokio::fs::write(&temp_meta_path, timestamp.to_string()).await?;

            // Atomically rename both files into place
            // This ensures that both files appear simultaneously
            tokio::fs::rename(&temp_path, &path).await?;
            tokio::fs::rename(&temp_meta_path, &meta_path).await?;

            Ok::<(), std::io::Error>(())
        }
        .await;

        // Clean up temporary files on error
        if write_result.is_err() {
            let _ = tokio::fs::remove_file(&temp_path).await;
            let _ = tokio::fs::remove_file(&temp_meta_path).await;
        }

        write_result?;

        Ok(())
    }

    /// Read response from cache
    pub async fn read(&self, region: &str, product: &str, endpoint: &str) -> Result<Vec<u8>> {
        if !self.is_valid(region, product, endpoint).await {
            return Err(Error::CacheEntryNotFound(format!(
                "{region}/{product}/{endpoint}"
            )));
        }

        let path = self.cache_path(region, product, endpoint);
        trace!(
            "Reading from Ribbit cache: {}/{}/{}",
            region, product, endpoint
        );
        Ok(tokio::fs::read(&path).await?)
    }

    /// Clear expired entries from cache
    pub async fn clear_expired(&self) -> Result<()> {
        debug!("Clearing expired entries from Ribbit cache");
        self.clear_expired_in_dir(&self.base_dir).await
    }

    /// Recursively clear expired entries in a directory
    fn clear_expired_in_dir<'a>(
        &'a self,
        dir: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    self.clear_expired_in_dir(&path).await?;
                } else if path.extension().and_then(|s| s.to_str()) == Some("meta") {
                    // Check if this metadata file indicates an expired entry
                    if let Ok(metadata) = tokio::fs::read_to_string(&path).await {
                        if let Ok(timestamp) = metadata.trim().parse::<u64>() {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();

                            if (now - timestamp) >= self.default_ttl.as_secs() {
                                // Remove both the data and metadata files
                                let data_path = path.with_extension("");
                                let _ = tokio::fs::remove_file(&data_path).await;
                                let _ = tokio::fs::remove_file(&path).await;
                                trace!("Removed expired cache entry: {:?}", data_path);
                            }
                        }
                    }
                }
            }

            Ok(())
        })
    }

    /// Get the base directory of this cache
    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    /// Get the current TTL setting
    pub fn ttl(&self) -> Duration {
        self.default_ttl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ribbit_cache_operations() {
        let cache = RibbitCache::with_ttl(Duration::from_secs(60))
            .await
            .unwrap();

        let region = "us";
        let product = "wow";
        let endpoint = "versions";
        let data = b"test ribbit response";

        // Write and verify
        cache.write(region, product, endpoint, data).await.unwrap();
        assert!(cache.is_valid(region, product, endpoint).await);

        // Read back
        let read_data = cache.read(region, product, endpoint).await.unwrap();
        assert_eq!(read_data, data);

        // Cleanup
        let _ = tokio::fs::remove_file(cache.cache_path(region, product, endpoint)).await;
        let _ = tokio::fs::remove_file(cache.metadata_path(region, product, endpoint)).await;
    }

    #[tokio::test]
    async fn test_ribbit_cache_expiry() {
        // Create cache with 0 second TTL
        let cache = RibbitCache::with_ttl(Duration::from_secs(0)).await.unwrap();

        let region = "eu";
        let product = "wow";
        let endpoint = "cdns";
        let data = b"test data";

        cache.write(region, product, endpoint, data).await.unwrap();

        // Should be immediately expired
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!cache.is_valid(region, product, endpoint).await);

        // Should fail to read
        assert!(cache.read(region, product, endpoint).await.is_err());

        // Cleanup
        let _ = tokio::fs::remove_file(cache.cache_path(region, product, endpoint)).await;
        let _ = tokio::fs::remove_file(cache.metadata_path(region, product, endpoint)).await;
    }
}

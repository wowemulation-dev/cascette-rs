//! CDN content cache implementation
//!
//! This module caches all CDN content following the CDN path structure:
//! - `{cdn_path}/config/{first2}/{next2}/{hash}` - Configuration files
//! - `{cdn_path}/data/{first2}/{next2}/{hash}` - Data files and archives
//! - `{cdn_path}/patch/{first2}/{next2}/{hash}` - Patch files
//!
//! Where `{cdn_path}` is the path provided by the CDN (e.g., "tpr/wow").
//! Archives and indices are stored in the data directory with `.index` extension for indices.

use crate::{Cache, Result};
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

/// Cache for CDN content following the standard CDN directory structure
pub struct CdnCache {
    c: Cache,
}

impl CdnCache {
    /// Create a new CDN cache in
    /// [the user's cache directory][crate::get_cache_dir].
    pub async fn new() -> Result<Self> {
        let c = Cache::with_subdirectory("cdn").await?;
        Ok(Self { c })
    }

    /// Create a new CDN cache with a custom subdirectory
    pub async fn with_subdirectory(subdir: impl AsRef<Path>) -> Result<Self> {
        let subdir = PathBuf::from("cdn").join(subdir);
        let c = Cache::with_subdirectory(subdir).await?;
        Ok(Self { c })
    }

    /// Create a CDN cache with a custom base directory
    pub async fn with_base_dir(base_dir: impl AsRef<Path>) -> Result<Self> {
        let path = base_dir.as_ref().join("cdn");
        let c = Cache::with_base_dir(path).await?;
        Ok(Self { c })
    }
}

impl Deref for CdnCache {
    type Target = Cache;

    fn deref(&self) -> &Self::Target {
        &self.c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt as _;

    #[tokio::test]
    async fn test_cdn_cache_paths() {
        let cache = CdnCache::new().await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let config_path = cache.cache_path("config", hash);
        assert!(config_path.ends_with("config/de/ad/deadbeef1234567890abcdef12345678"));

        let index_path = cache.cache_path_with_suffix("data", hash, ".index");
        assert!(index_path.ends_with("data/de/ad/deadbeef1234567890abcdef12345678.index"));
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_cdn_cache_operations() {
        let cache = CdnCache::with_subdirectory("test").await.unwrap();
        let hash = "test5678901234567890abcdef123456";
        let data = b"test data content";

        // Delete any stale cache entry
        cache.delete_object("data", hash).await.unwrap();

        {
            // Write and read data
            let mut fh = cache
                .write_buffer("data", hash, &data[..])
                .await
                .unwrap();

            // Test size
            let size = fh.metadata().await.unwrap().len();
            assert_eq!(size, data.len() as u64);

            // Check that we can read back with the supplied handle
            let mut read_data = Vec::new();
            fh.read_to_end(&mut read_data).await.unwrap();
            assert_eq!(read_data, data);
        }

        {
            // Re-open the file
            let mut fh = cache
                .read_object("data", hash)
                .await
                .unwrap()
                .expect("cached file should exist");

            // Test size
            let size = fh.metadata().await.unwrap().len();
            assert_eq!(size, data.len() as u64);

            // Check that we can read back with the supplied handle
            let mut read_data = Vec::new();
            fh.read_to_end(&mut read_data).await.unwrap();
            assert_eq!(read_data, data);
        }

        // Cleanup
        cache.delete_object("data", hash).await.unwrap();
    }
}

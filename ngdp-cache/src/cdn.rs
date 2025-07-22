//! CDN content cache implementation
//!
//! This module caches all CDN content following the CDN path structure:
//! - `{cdn_path}/config/{first2}/{next2}/{hash}` - Configuration files
//! - `{cdn_path}/data/{first2}/{next2}/{hash}` - Data files and archives
//! - `{cdn_path}/patch/{first2}/{next2}/{hash}` - Patch files
//!
//! Where `{cdn_path}` is the path provided by the CDN (e.g., "tpr/wow").
//! Archives and indices are stored in the data directory with `.index` extension for indices.

use crate::{Result, ensure_dir, get_cache_dir};
use futures::StreamExt as _;
use reqwest::Response;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};
use tracing::*;

/// Cache for CDN content following the standard CDN directory structure
pub struct CdnCache {
    /// Base directory for CDN cache
    base_dir: PathBuf,
}

// TODO: merge with GenericCache
impl CdnCache {
    /// Create a new CDN cache in [the user's cache directory][get_cache_dir].
    pub async fn new() -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn");
        ensure_dir(&base_dir).await?;

        debug!("Initialized CDN cache at: {:?}", base_dir);

        Ok(Self { base_dir })
    }

    /// Create a new CDN cache for a specific product in
    /// [the user's cache directory][get_cache_dir].
    ///
    /// # Deprecated
    ///
    /// Use [`Self::with_base_dir`] instead.
    #[deprecated(note = "use with_base_dir instead")]
    pub async fn for_product(product: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn").join(product);
        ensure_dir(&base_dir).await?;

        debug!(
            "Initialized CDN cache for product '{}' at: {:?}",
            product, base_dir
        );

        Ok(Self { base_dir })
    }

    /// Create a CDN cache with a custom base directory
    pub async fn with_base_dir(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        ensure_dir(base_dir).await?;

        debug!("Initialized CDN cache at: {base_dir:?}");

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
        })
    }

    /// Create a CDN cache with a specific CDN path
    #[deprecated(note = "use with_base_dir instead")]
    pub async fn with_cdn_path(cdn_path: &str) -> Result<Self> {
        let base_dir = get_cache_dir()?.join("cdn").join(cdn_path);
        ensure_dir(&base_dir).await?;

        debug!(
            "Initialized CDN cache with path '{}' at: {:?}",
            cdn_path, base_dir
        );

        Ok(Self { base_dir })
    }

    /// Path where `path/hash` should be cached to.
    ///
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    pub fn cache_path(&self, path: impl AsRef<Path>, hash: &str, suffix: &str) -> PathBuf {
        let mut path = self.base_dir().join(path);
        if hash.len() >= 4 {
            // abcdef -> ab/cd/abcdef
            path.push(&hash[..2]);
            path.push(&hash[2..4]);
        }
        path.push(format!("{hash}{suffix}"));
        path
    }

    /// Open a cache file for reading.
    ///    
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    ///
    /// Returns `Ok(None)` if the file does not exist. All other errors are
    /// propegated normally.
    pub async fn read_cache<'a>(
        &self,
        path: impl AsRef<Path>,
        hash: &'a str,
        suffix: &'a str,
    ) -> Result<Option<File>> {
        let path = path.as_ref();
        debug!("Cache for {path:?} {hash:?} {suffix:?}");
        let path = self.cache_path(path, hash, suffix);

        match OpenOptions::new().read(true).open(&path).await {
            Ok(f) => Ok(Some(f)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => {
                error!("Read cache error for {path:?}: {e:?}");
                Err(e.into())
            }
        }
    }

    /// Writes a [`Response`] to a file, and then return a handle to that file,
    /// seeked to the start.
    ///
    /// The file will be open in read-write mode, but trait bounds will
    /// attempt to prevent write operations.
    ///
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    pub async fn write_response(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
        response: Response,
    ) -> Result<File> {
        let path = self.cache_path(path, hash, suffix);
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        let mut output = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)
            .await?;
        let len = response.content_length().unwrap_or(0);
        let mut stream = response.bytes_stream();

        let mut first = true;
        while let Some(buf) = stream.next().await {
            if first {
                first = false;
                // Only resize the file once the first chunk arrives.
                output.set_len(len).await?;
            }
            let buf = buf?;
            output.write_all(&buf).await?;
        }

        output.flush().await?;
        output.rewind().await?;
        Ok(output)
    }

    /// Write a buffer to the cache.
    pub async fn write_buffer(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
        mut buffer: impl AsyncReadExt + Unpin,
    ) -> Result<File> {
        let path = self.cache_path(path, hash, suffix);
        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        let mut output = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)
            .await?;

        let mut b = [0; 8 << 10];
        while let Ok(len) = buffer.read(&mut b).await {
            output.write_all(&b[..len]).await?;
        }

        output.flush().await?;
        output.rewind().await?;
        Ok(output)
    }

    /// Delete an item from the cache.
    ///
    /// Returns:
    ///
    /// * `Ok(true)` if a cache file existed and was deleted
    /// * `Ok(false)` if a cache file did not exist
    /// * `Err` on other errors
    pub async fn delete_object(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> Result<bool> {
        let path = self.cache_path(path, hash, suffix);

        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Get cached object size without reading it
    ///
    /// # Safety
    ///
    /// This function is not atomic.
    pub async fn object_size(
        &self,
        path: impl AsRef<Path>,
        hash: &str,
        suffix: &str,
    ) -> Result<Option<u64>> {
        let path = self.cache_path(path, hash, suffix);
        match tokio::fs::metadata(&path).await {
            Ok(m) => Ok(Some(m.len())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
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
    async fn test_cdn_cache_paths() {
        let cache = CdnCache::new().await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let config_path = cache.cache_path("config", hash, "");
        assert!(config_path.ends_with("config/de/ad/deadbeef1234567890abcdef12345678"));

        let index_path = cache.cache_path("data", hash, ".index");
        assert!(index_path.ends_with("data/de/ad/deadbeef1234567890abcdef12345678.index"));
    }

    #[tokio::test]
    async fn test_cdn_cache_with_cdn_path() {
        let cache = CdnCache::with_cdn_path("tpr/wow").await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let config_path = cache.cache_path("config", hash, "");
        assert!(config_path.ends_with("tpr/wow/config/de/ad/deadbeef1234567890abcdef12345678"));
    }

    #[tokio::test]
    async fn test_cdn_product_cache() {
        let cache = CdnCache::for_product("wow").await.unwrap();
        assert!(cache.base_dir().ends_with("cdn/wow"));
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_cdn_cache_operations() {
        let cache = CdnCache::for_product("test").await.unwrap();
        let hash = "test5678901234567890abcdef123456";
        let data = b"test data content";

        // Delete any stale cache entry
        cache.delete_object("data", hash, "").await.unwrap();

        {
            // Write and read data
            let mut fh = cache
                .write_buffer("data", hash, "", &data[..])
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
                .read_cache("data", hash, "")
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
        cache.delete_object("data", hash, "").await.unwrap();
    }
}

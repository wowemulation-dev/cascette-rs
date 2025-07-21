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
use tracing::{debug, trace, error};

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
    #[deprecated(note = "simplifying API")]
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
    pub async fn with_base_dir(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        ensure_dir(base_dir).await?;

        debug!("Initialized CDN cache at: {base_dir:?}");

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
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

    /// Get the data range cache directory
    pub fn data_range_dir(&self) -> PathBuf {
        self.effective_base_dir().join("range")
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

    /// Construct a data range path from a hash.
    ///
    /// These paths don't actually exist on the CDN, but we use a similar
    /// structure: `range/{first2}/{next2}/{hash}`
    pub fn data_range_path(&self, hash: &str) -> PathBuf {
        if hash.len() >= 4 {
            self.data_range_dir()
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash)
        } else {
            self.data_range_dir().join(hash)
        }
    }

    /// Check if a config exists in cache
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"config\", ...).await?` instead; this function is not atomic"
    )]
    pub async fn has_config(&self, hash: &str) -> bool {
        tokio::fs::metadata(self.config_path(hash)).await.is_ok()
    }

    /// Check if data exists in cache
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"data\", ...).await?` instead; this function is not atomic"
    )]
    pub async fn has_data(&self, hash: &str) -> bool {
        tokio::fs::metadata(self.data_path(hash)).await.is_ok()
    }

    /// Check if a data range file exists in cache
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"range\", ...).await?` instead; this function is not atomic"
    )]
    pub async fn has_data_range(&self, hash: &str) -> bool {
        tokio::fs::metadata(self.data_range_path(hash))
            .await
            .is_ok()
    }

    /// Check if a patch exists in cache
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"patch\", ...).await?` instead; this function is not atomic"
    )]
    pub async fn has_patch(&self, hash: &str) -> bool {
        tokio::fs::metadata(self.patch_path(hash)).await.is_ok()
    }

    /// Check if an index exists in cache
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"data\", \"{hash}.index\").await?` instead; this function is not atomic"
    )]
    pub async fn has_index(&self, hash: &str) -> bool {
        tokio::fs::metadata(self.index_path(hash)).await.is_ok()
    }

    /// Write config data to cache
    #[deprecated(note = "use write_response")]
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
    #[deprecated(note = "use write_response")]
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
    #[deprecated(note = "use write_response")]
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
    #[deprecated(note = "use write_response")]
    pub async fn write_index(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.index_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to index cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Write data range to cache
    #[deprecated(note = "use write_response")]
    pub async fn write_data_range(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.data_range_path(hash);

        if let Some(parent) = path.parent() {
            ensure_dir(parent).await?;
        }

        trace!("Writing {} bytes to data range cache: {}", data.len(), hash);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    /// Path where `path/hash` should be cached to.
    ///
    /// `hash` has no formatting restrictions, and may have a suffix appended to
    /// it.
    pub fn cache_path(&self, path: impl AsRef<Path>, hash: &str, suffix: &str) -> PathBuf {
        let mut path = self.effective_base_dir().join(path);
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
            },
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

    /// Read config from cache
    #[deprecated(note = "use `read_cache(\"config\", ...)` instead")]
    pub async fn read_config(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.config_path(hash);
        trace!("Reading config from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read data from cache
    #[deprecated(note = "use `read_cache(\"data\", ...)` instead")]
    pub async fn read_data(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.data_path(hash);
        trace!("Reading data from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read patch from cache
    #[deprecated(note = "use `read_cache(\"patch\", ...)` instead")]
    pub async fn read_patch(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.patch_path(hash);
        trace!("Reading patch from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read index from cache
    #[deprecated(note = "use `read_cache(\"index\", ...)` instead")]
    pub async fn read_index(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.index_path(hash);
        trace!("Reading index from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Read index from cache
    #[deprecated(note = "use `read_cache(\"range\", ...)` instead")]
    pub async fn read_data_range(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.data_range_path(hash);
        trace!("Reading data range from cache: {}", hash);
        Ok(tokio::fs::read(&path).await?)
    }

    /// Stream read data from cache
    ///
    /// Returns a file handle for efficient streaming
    #[deprecated(note = "use `read_cache(\"data\", ...)` instead")]
    pub async fn open_data(&self, hash: &str) -> Result<tokio::fs::File> {
        let path = self.data_path(hash);
        trace!("Opening data for streaming: {}", hash);
        Ok(tokio::fs::File::open(&path).await?)
    }

    /// Get data size without reading it
    ///
    /// # Safety
    ///
    /// This function is not atomic.
    pub async fn data_size(&self, hash: &str) -> Result<u64> {
        let path = self.data_path(hash);
        let metadata = tokio::fs::metadata(&path).await?;
        Ok(metadata.len())
    }

    /// Get the base directory of this cache
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the CDN path if set
    pub fn cdn_path(&self) -> Option<&str> {
        self.cdn_path.as_deref()
    }

    /// Write multiple config files in parallel
    #[deprecated(note = "use write_response")]
    #[allow(deprecated)]
    pub async fn write_configs_batch(&self, entries: &[(String, Vec<u8>)]) -> Result<()> {
        use futures::future::try_join_all;

        let futures = entries
            .iter()
            .map(|(hash, data)| self.write_config(hash, data));

        try_join_all(futures).await?;
        Ok(())
    }

    /// Write multiple data files in parallel
    #[deprecated(note = "use write_response")]
    #[allow(deprecated)]
    pub async fn write_data_batch(&self, entries: &[(String, Vec<u8>)]) -> Result<()> {
        use futures::future::try_join_all;

        let futures = entries
            .iter()
            .map(|(hash, data)| self.write_data(hash, data));

        try_join_all(futures).await?;
        Ok(())
    }

    /// Read multiple config files in parallel
    #[deprecated(note = "use `read_cache(\"config\", ...)` instead")]
    #[allow(deprecated)]
    pub async fn read_configs_batch(&self, hashes: &[String]) -> Vec<Result<Vec<u8>>> {
        use futures::future::join_all;

        let futures = hashes.iter().map(|hash| self.read_config(hash));
        join_all(futures).await
    }

    /// Read multiple data files in parallel
    #[deprecated(note = "use `read_cache(\"data\", ...)` instead")]
    #[allow(deprecated)]
    pub async fn read_data_batch(&self, hashes: &[String]) -> Vec<Result<Vec<u8>>> {
        use futures::future::join_all;

        let futures = hashes.iter().map(|hash| self.read_data(hash));
        join_all(futures).await
    }

    /// Check existence of multiple configs in parallel
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"config\", ...).await?` instead; this function is not atomic"
    )]
    #[allow(deprecated)]
    pub async fn has_configs_batch(&self, hashes: &[String]) -> Vec<bool> {
        use futures::future::join_all;

        let futures = hashes.iter().map(|hash| self.has_config(hash));
        join_all(futures).await
    }

    /// Check existence of multiple data files in parallel
    #[deprecated(
        note = "use `if let Some(f) = read_cache(\"data\", ...).await?` instead; this function is not atomic"
    )]
    #[allow(deprecated)]
    pub async fn has_data_batch(&self, hashes: &[String]) -> Vec<bool> {
        use futures::future::join_all;

        let futures = hashes.iter().map(|hash| self.has_data(hash));
        join_all(futures).await
    }

    /// Get sizes of multiple data files in parallel
    ///
    /// # Safety
    ///
    /// This function is not atomic.
    pub async fn data_sizes_batch(&self, hashes: &[String]) -> Vec<Result<u64>> {
        use futures::future::join_all;

        let futures = hashes.iter().map(|hash| self.data_size(hash));
        join_all(futures).await
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
        assert!(config_path.ends_with("config/de/ad/deadbeef1234567890abcdef12345678"));

        let data_path = cache.data_path(hash);
        assert!(data_path.ends_with("data/de/ad/deadbeef1234567890abcdef12345678"));

        let patch_path = cache.patch_path(hash);
        assert!(patch_path.ends_with("patch/de/ad/deadbeef1234567890abcdef12345678"));

        let index_path = cache.index_path(hash);
        assert!(index_path.ends_with("data/de/ad/deadbeef1234567890abcdef12345678.index"));

        let data_range_path = cache.data_range_path(hash);
        assert!(data_range_path.ends_with("range/de/ad/deadbeef1234567890abcdef12345678"));
    }

    #[tokio::test]
    async fn test_cdn_cache_with_cdn_path() {
        let cache = CdnCache::with_cdn_path("tpr/wow").await.unwrap();

        let hash = "deadbeef1234567890abcdef12345678";

        let config_path = cache.config_path(hash);
        assert!(config_path.ends_with("tpr/wow/config/de/ad/deadbeef1234567890abcdef12345678"));

        let data_path = cache.data_path(hash);
        assert!(data_path.ends_with("tpr/wow/data/de/ad/deadbeef1234567890abcdef12345678"));

        let patch_path = cache.patch_path(hash);
        assert!(patch_path.ends_with("tpr/wow/patch/de/ad/deadbeef1234567890abcdef12345678"));
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

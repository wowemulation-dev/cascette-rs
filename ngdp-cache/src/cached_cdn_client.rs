//! Cached wrapper for CDN client
//!
//! This module provides a caching layer for CdnClient that stores CDN content
//! files locally in ~/.cache/ngdp/cdn/ preserving the CDN path structure.
//!
//! The cache structure mirrors the CDN paths:
//! - `{cdn_path}/config/{first2}/{next2}/{hash}` - Configuration files
//! - `{cdn_path}/data/{first2}/{next2}/{hash}` - Data files
//! - `{cdn_path}/patch/{first2}/{next2}/{hash}` - Patch files
//!
//! Where `{cdn_path}` is extracted from the path parameter (e.g., "tpr/wow").
//!
//! Content files are immutable (addressed by hash), so once cached they never expire.
//! This significantly reduces bandwidth usage and improves performance when
//! accessing the same content multiple times.
//!
//! # Example
//!
//! ```no_run
//! use ngdp_cache::cached_cdn_client::CachedCdnClient;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a cached client
//! let client = CachedCdnClient::new().await?;
//!
//! // Download content - first time fetches from CDN
//! // The CDN path "tpr/wow" will be preserved in the cache structure
//! let response = client.download(
//!     "blzddist1-a.akamaihd.net",
//!     "tpr/wow/config",  // Config files use /config suffix
//!     "2e9c1e3b5f5a0c9d9e8f1234567890ab",
//!     "",
//! ).await?;
//! let data = response.bytes().await?;
//!
//! // File is cached at: ~/.cache/ngdp/cdn/tpr/wow/config/2e/9c/{hash}
//!
//! // Subsequent calls use cache - no network request!
//! let response2 = client.download(
//!     "blzddist1-a.akamaihd.net",
//!     "tpr/wow/config",
//!     "2e9c1e3b5f5a0c9d9e8f1234567890ab",
//!     "",
//! ).await?;
//! # Ok(())
//! # }
//! ```

use crate::{CdnCache, Error, Result};
use bytes::Bytes;
use ngdp_cdn::{CdnClient, CdnClientBuilder, CdnClientBuilderTrait, CdnClientTrait};
use reqwest::Response;
use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt},
};
use tracing::*;

/// A caching wrapper around CdnClient
///
/// TODO: use `CdnClientWithFallback` instead of `CdnClient`.
pub struct CachedCdnClient {
    /// The underlying CDN client
    client: CdnClient,

    cache: CdnCache,
}

impl CachedCdnClient {
    /// Create a new cached client for a specific product
    #[deprecated(note = "simplifying API")]
    #[allow(deprecated)]
    pub async fn for_product(product: &str) -> Result<Self> {
        let client = CdnClient::new()?;
        let cache = CdnCache::with_subdirectory(product).await?;

        debug!("Initialized cached CDN client for product '{}'", product);

        Ok(Self { client, cache })
    }

    /// Create a new cached client with custom cache directory    
    pub async fn with_cache_dir(cache_dir: impl AsRef<Path>) -> Result<Self> {
        let client = CdnClient::new()?;
        let cache = CdnCache::with_base_dir(cache_dir).await?;

        Ok(Self { client, cache })
    }

    /// Create from an existing CDN client
    pub async fn with_client(client: CdnClient) -> Result<Self> {
        let cache = CdnCache::new().await?;
        Ok(Self { client, cache })
    }

    /// Enable or disable caching
    #[deprecated(note = "AsyncRead/Seek APIs always need caching, this does nothing!")]
    #[allow(deprecated)]
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        if !enabled {
            error!("Cannot disable caching with CachedCdnClient");
        }
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache.base_dir()
    }

    /// Make a basic request to a CDN URL
    ///
    /// This method does not use caching as it's for arbitrary URLs.
    /// Use `download` for hash-based content that should be cached.
    pub async fn request(&self, url: &str) -> Result<Response> {
        Ok(self.client.request(url).await?)
    }

    /// Stream download content from CDN with caching
    ///
    /// For large files, this method allows streaming the content while still
    /// benefiting from caching. If the content is cached, it opens the file
    /// for streaming. Otherwise, it downloads and caches the content first.
    ///
    /// # Deprecated
    ///
    /// This is now an alias for [`Self::download`].
    ///
    /// The original version of the function only read data file caches.
    ///
    /// For other files and on cache misses, it will download the file into
    /// memory, and return a buffer, and **not** cache the result.
    ///
    /// Use [`Self::download`][] instead.
    #[deprecated(note = "Use `download()` instead")]
    #[allow(deprecated)]
    pub async fn download_stream(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        suffix: &str,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        Ok(Box::new(
            self.download(cdn_host, path, hash, suffix)
                .await?
                .into_inner(),
        ))
    }

    /// Get the size of cached content without reading it
    ///
    /// # Safety
    ///
    /// This function is not atomic.
    pub async fn cached_size(&self, path: &str, hash: &str, suffix: &str) -> Result<Option<u64>> {
        self.cache.object_size_with_suffix(path, hash, suffix).await
    }

    /// Clear all cached content
    ///
    /// This removes all cached CDN content from disk.
    /// Use with caution as it will require re-downloading all content.
    pub async fn clear_cache(&self) -> Result<()> {
        let cache_dir = self.cache_dir();
        if tokio::fs::metadata(cache_dir).await.is_ok() {
            tokio::fs::remove_dir_all(cache_dir).await?;
        }
        Ok(())
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<CacheStats> {
        let mut stats = CacheStats::default();

        // Count files and calculate sizes for each content type
        for entry in walkdir::WalkDir::new(self.cache_dir())
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    stats.total_files += 1;
                    stats.total_size += metadata.len();

                    let path = entry.path().to_string_lossy();
                    if path.contains("config") {
                        stats.config_files += 1;
                        stats.config_size += metadata.len();
                    } else if path.contains("patch") {
                        stats.patch_files += 1;
                        stats.patch_size += metadata.len();
                    } else if path.contains("data") {
                        stats.data_files += 1;
                        stats.data_size += metadata.len();
                    } else if path.contains("range") {
                        stats.range_files += 1;
                        stats.range_size = metadata.len();
                    }
                }
            }
        }

        Ok(stats)
    }
}

#[async_trait::async_trait]
impl CdnClientTrait for CachedCdnClient {
    type Response = CachedResponse;
    type Error = Error;
    type Builder = CachedCdnClientBuilder;

    /// Create a new cached CDN client
    async fn new() -> std::result::Result<Self, Self::Error> {
        let client = CdnClient::new()?;
        let cache = CdnCache::new().await?;

        debug!("Initialized cached CDN client");

        Ok(Self { client, cache })
    }

    fn builder() -> Self::Builder {
        CachedCdnClientBuilder::new()
    }

    /// Download content from CDN by hash with caching
    ///
    /// If caching is enabled and the content exists in cache, it will be returned
    /// without making a network request. Otherwise, the content is downloaded
    /// from the CDN and stored in cache for future use.
    async fn download(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        suffix: &str,
    ) -> std::result::Result<Self::Response, Self::Error> {
        if let Some(file) = self.cache.read_object_with_suffix(path, hash, suffix).await? {
            debug!("Cache hit for CDN {path}/{hash}{suffix}");
            return Ok(CachedResponse::from_cache(file));
        }

        // Cache miss - download from CDN
        debug!("Cache miss for CDN {path}/{hash}{suffix}, fetching from server");
        let response = self.client.download(cdn_host, path, hash, suffix).await?;

        // Copy the downloaded data to cache
        let file = self
            .cache
            .write_response_with_suffix(path, hash, suffix, response)
            .await?;

        Ok(CachedResponse::from_network(file))
    }

    /// Download partial content from CDN, with caching.
    ///
    /// This requires an additional `cache_hash`, which is used to key the cache
    /// entries in the range cache directory. A BLTE stream's `EKey` is one way
    /// to handle this.
    ///
    /// A single, global namespace is used for all entries in `cache_hash`,
    /// regardless of whether they are `data` or `patches`.
    ///
    /// If caching is enabled and the content exists in cache, it will be returned
    /// without making a network request. Otherwise, the content is downloaded
    /// from the CDN and stored in cache for future use.
    async fn download_range(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        cache_hash: &str,
        range: impl Into<RangeInclusive<u64>> + Send,
    ) -> std::result::Result<Self::Response, Self::Error> {
        let range = range.into();
        let cache_path = "range";

        if let Some(file) = self.cache.read_object(cache_path, cache_hash).await? {
            debug!("Cache hit for CDN {path}/{cache_hash}");
            return Ok(CachedResponse::from_cache(file));
        }

        // Cache miss - download from CDN
        debug!(
            "Cache miss for ranged file {}/{}, fetching {}/{} ({}-{}) from server",
            cache_path,
            cache_hash,
            path,
            hash,
            range.start(),
            range.end(),
        );
        let response = self
            .client
            .download_range(cdn_host, path, hash, cache_hash, range)
            .await?;

        // Copy the downloaded data to cache
        let file = self
            .cache
            .write_response(cache_path, &cache_hash, response)
            .await?;

        Ok(CachedResponse::from_network(file))
    }
}

#[derive(Clone, Debug)]
pub struct CachedCdnClientBuilder {
    /// The underlying CDN client builder
    builder: CdnClientBuilder,

    /// Base cache directory
    cache_base_dir: Option<PathBuf>,
}

#[async_trait::async_trait]
impl CdnClientBuilderTrait for CachedCdnClientBuilder {
    type Client = CachedCdnClient;
    type Error = Error;

    fn new() -> Self {
        Self {
            builder: CdnClientBuilder::new(),
            cache_base_dir: None,
        }
    }

    async fn build(self) -> std::result::Result<Self::Client, Self::Error> {
        let cache = match self.cache_base_dir {
            Some(c) => CdnCache::with_base_dir(c).await?,
            None => CdnCache::new().await?,
        };

        Ok(CachedCdnClient {
            client: self.builder.build().await?,
            cache,
        })
    }
}

impl CachedCdnClientBuilder {
    /// Set the cache base directory
    pub fn with_cache_base_dir(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        self.cache_base_dir = Some(path.to_path_buf());
        self
    }

    /// Configure the base CDN client
    pub fn configure_base_client<F>(mut self, f: F) -> Self
    where
        F: FnOnce(CdnClientBuilder) -> CdnClientBuilder,
    {
        self.builder = f(self.builder);
        self
    }
}

/// Response wrapper that indicates whether content came from cache
pub struct CachedResponse {
    /// The response data
    data: File,
    /// Whether this response came from cache
    from_cache: bool,
}

impl CachedResponse {
    /// Create a response from cache
    fn from_cache(data: File) -> Self {
        Self {
            data,
            from_cache: true,
        }
    }

    /// Create a response from network
    fn from_network(data: File) -> Self {
        Self {
            data,
            from_cache: false,
        }
    }

    /// Check if this response came from cache
    pub fn is_from_cache(&self) -> bool {
        self.from_cache
    }

    /// Get the response data
    pub fn into_inner(self) -> File {
        self.data
    }

    /// Get the response data as bytes
    ///
    /// This calls [`File::read_to_end`][], which loads the entire file into
    /// RAM. Consider using the [`AsyncRead`] or
    /// [`AsyncBufRead`][tokio::io::AsyncBufRead] traits instead.
    pub async fn bytes(mut self) -> Result<Bytes> {
        let mut buf = Vec::new();
        self.data.read_to_end(&mut buf).await?;
        Ok(buf.into())
    }

    // /// Get the response data as text
    // pub async fn text(self) -> Result<String> {
    //     Ok(String::from_utf8(self.data.to_vec())?)
    // }

    // /// Get the content length
    // pub fn content_length(&self) -> usize {
    //     self.data.len()
    // }
}

/// Cache statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    /// Total number of cached files
    pub total_files: u64,
    /// Total size of cached files in bytes
    pub total_size: u64,
    /// Number of cached config files
    pub config_files: u64,
    /// Size of cached config files in bytes
    pub config_size: u64,
    /// Number of cached data files
    pub data_files: u64,
    /// Size of cached data files in bytes
    pub data_size: u64,
    /// Number of cached patch files
    pub patch_files: u64,
    /// Size of cached patch files in bytes
    pub patch_size: u64,
    /// Number of cached range files
    pub range_files: u64,
    /// Size of cached range files in bytes
    pub range_size: u64,
}

impl CacheStats {
    /// Get total size in human-readable format
    pub fn total_size_human(&self) -> String {
        format_bytes(self.total_size)
    }

    /// Get config size in human-readable format
    pub fn config_size_human(&self) -> String {
        format_bytes(self.config_size)
    }

    /// Get data size in human-readable format
    pub fn data_size_human(&self) -> String {
        format_bytes(self.data_size)
    }

    /// Get patch size in human-readable format  
    pub fn patch_size_human(&self) -> String {
        format_bytes(self.patch_size)
    }

    /// Get range size in human-readable format  
    pub fn range_size_human(&self) -> String {
        format_bytes(self.range_size)
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[tokio::test]
    async fn test_cache_with_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(client.cache_dir(), &temp_dir.path().join("cdn"));
    }
}

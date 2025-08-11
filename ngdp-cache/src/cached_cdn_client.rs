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
//! ).await?;
//! # Ok(())
//! # }
//! ```

use bytes::Bytes;
use reqwest::Response;
use std::path::PathBuf;
use tokio::io::AsyncRead;
use tracing::debug;

use ngdp_cdn::CdnClient;

use crate::{CdnCache, Result};

/// Type of CDN content based on path
#[derive(Debug, Clone, Copy, PartialEq)]
enum ContentType {
    Config,
    Data,
    Patch,
}

impl ContentType {
    /// Determine content type from CDN path
    fn from_path(path: &str) -> Self {
        let path_lower = path.to_lowercase();
        if path_lower.contains("/config") || path_lower.ends_with("config") {
            Self::Config
        } else if path_lower.contains("/patch") || path_lower.ends_with("patch") {
            Self::Patch
        } else {
            Self::Data
        }
    }
}

/// A caching wrapper around CdnClient
pub struct CachedCdnClient {
    /// The underlying CDN client
    client: CdnClient,
    /// Base cache directory
    cache_base_dir: PathBuf,
    /// Whether caching is enabled
    enabled: bool,
}

impl CachedCdnClient {
    /// Create a new cached CDN client
    pub async fn new() -> Result<Self> {
        let client = CdnClient::new()?;
        let cache_base_dir = crate::get_cache_dir()?.join("cdn");
        crate::ensure_dir(&cache_base_dir).await?;

        debug!("Initialized cached CDN client");

        Ok(Self {
            client,
            cache_base_dir,
            enabled: true,
        })
    }

    /// Create a new cached client for a specific product
    pub async fn for_product(product: &str) -> Result<Self> {
        let client = CdnClient::new()?;
        let cache_base_dir = crate::get_cache_dir()?.join("cdn").join(product);
        crate::ensure_dir(&cache_base_dir).await?;

        debug!("Initialized cached CDN client for product '{}'", product);

        Ok(Self {
            client,
            cache_base_dir,
            enabled: true,
        })
    }

    /// Create a new cached client with custom cache directory
    pub async fn with_cache_dir(cache_dir: PathBuf) -> Result<Self> {
        let client = CdnClient::new()?;
        crate::ensure_dir(&cache_dir).await?;

        Ok(Self {
            client,
            cache_base_dir: cache_dir,
            enabled: true,
        })
    }

    /// Create from an existing CDN client
    pub async fn with_client(client: CdnClient) -> Result<Self> {
        let cache_base_dir = crate::get_cache_dir()?.join("cdn");
        crate::ensure_dir(&cache_base_dir).await?;

        Ok(Self {
            client,
            cache_base_dir,
            enabled: true,
        })
    }

    /// Add a primary CDN host
    pub fn add_primary_host(&self, host: impl Into<String>) {
        self.client.add_primary_host(host);
    }

    /// Add multiple primary CDN hosts
    pub fn add_primary_hosts(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        self.client.add_primary_hosts(hosts);
    }

    /// Add a fallback CDN host
    pub fn add_fallback_host(&self, host: impl Into<String>) {
        self.client.add_fallback_host(host);
    }

    /// Add multiple fallback CDN hosts
    pub fn add_fallback_hosts(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        self.client.add_fallback_hosts(hosts);
    }

    /// Set primary CDN hosts, replacing any existing ones
    pub fn set_primary_hosts(&self, hosts: impl IntoIterator<Item = impl Into<String>>) {
        self.client.set_primary_hosts(hosts);
    }

    /// Get all configured hosts (primary first, then fallback)
    pub fn get_all_hosts(&self) -> Vec<String> {
        self.client.get_all_hosts()
    }

    /// Enable or disable caching
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_base_dir
    }

    /// Get or create a cache for a specific CDN path
    async fn get_cache_for_path(&self, cdn_path: &str) -> Result<CdnCache> {
        // Use the CDN path as-is - don't try to extract a base path
        let mut cache = CdnCache::with_base_dir(self.cache_base_dir.clone()).await?;
        cache.set_cdn_path(Some(cdn_path.to_string()));
        Ok(cache)
    }

    /// Check if content is cached
    async fn is_cached(&self, path: &str, hash: &str) -> Result<bool> {
        let cache = self.get_cache_for_path(path).await?;
        let content_type = ContentType::from_path(path);
        Ok(match content_type {
            ContentType::Config => cache.has_config(hash).await,
            ContentType::Data => cache.has_data(hash).await,
            ContentType::Patch => cache.has_patch(hash).await,
        })
    }

    /// Read content from cache
    async fn read_from_cache(&self, path: &str, hash: &str) -> Result<Bytes> {
        let cache = self.get_cache_for_path(path).await?;
        let content_type = ContentType::from_path(path);
        let data = match content_type {
            ContentType::Config => cache.read_config(hash).await?,
            ContentType::Data => cache.read_data(hash).await?,
            ContentType::Patch => cache.read_patch(hash).await?,
        };
        Ok(Bytes::from(data))
    }

    /// Write content to cache
    async fn write_to_cache(&self, path: &str, hash: &str, data: &[u8]) -> Result<()> {
        let cache = self.get_cache_for_path(path).await?;
        let content_type = ContentType::from_path(path);
        match content_type {
            ContentType::Config => cache.write_config(hash, data).await?,
            ContentType::Data => cache.write_data(hash, data).await?,
            ContentType::Patch => cache.write_patch(hash, data).await?,
        };
        Ok(())
    }

    /// Make a basic request to a CDN URL
    ///
    /// This method does not use caching as it's for arbitrary URLs.
    /// Use `download` for hash-based content that should be cached.
    pub async fn request(&self, url: &str) -> Result<Response> {
        Ok(self.client.request(url).await?)
    }

    /// Download content from CDN by hash with caching
    ///
    /// If caching is enabled and the content exists in cache, it will be returned
    /// without making a network request. Otherwise, the content is downloaded
    /// from the CDN and stored in cache for future use.
    pub async fn download(&self, cdn_host: &str, path: &str, hash: &str) -> Result<CachedResponse> {
        // Check cache first if enabled
        if self.enabled && self.is_cached(path, hash).await? {
            debug!("Cache hit for CDN {}/{}", path, hash);
            let data = self.read_from_cache(path, hash).await?;
            return Ok(CachedResponse::from_cache(data));
        }

        // Cache miss - download from CDN
        debug!("Cache miss for CDN {}/{}, fetching from server", path, hash);
        let response = self.client.download(cdn_host, path, hash).await?;

        // Get the response body
        let data = response.bytes().await?;

        // Cache the content if enabled
        if self.enabled {
            if let Err(e) = self.write_to_cache(path, hash, &data).await {
                debug!("Failed to write to CDN cache: {}", e);
            }
        }

        Ok(CachedResponse::from_network(data))
    }

    /// Stream download content from CDN with caching
    ///
    /// For large files, this method allows streaming the content while still
    /// benefiting from caching. If the content is cached, it opens the file
    /// for streaming. Otherwise, it downloads and caches the content first.
    pub async fn download_stream(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        // For data files, we can use the streaming API
        if ContentType::from_path(path) == ContentType::Data {
            // Check if cached
            let cache = self.get_cache_for_path(path).await?;
            if self.enabled && cache.has_data(hash).await {
                debug!("Cache hit for CDN {}/{} (streaming)", path, hash);
                let file = cache.open_data(hash).await?;
                return Ok(Box::new(file));
            }
        }

        // For non-data files or cache misses, download the full content first
        let response = self.download(cdn_host, path, hash).await?;
        let data = response.bytes().await?;

        // Return a cursor over the bytes
        Ok(Box::new(std::io::Cursor::new(data.to_vec())))
    }

    /// Get the size of cached content without reading it
    pub async fn cached_size(&self, path: &str, hash: &str) -> Result<Option<u64>> {
        if !self.enabled || !self.is_cached(path, hash).await? {
            return Ok(None);
        }

        // Only data files support efficient size checking
        if ContentType::from_path(path) == ContentType::Data {
            let cache = self.get_cache_for_path(path).await?;
            Ok(Some(cache.data_size(hash).await?))
        } else {
            // For other types, we need to read the full content
            let data = self.read_from_cache(path, hash).await?;
            Ok(Some(data.len() as u64))
        }
    }

    /// Clear all cached content
    ///
    /// This removes all cached CDN content from disk.
    /// Use with caution as it will require re-downloading all content.
    pub async fn clear_cache(&self) -> Result<()> {
        if tokio::fs::metadata(&self.cache_base_dir).await.is_ok() {
            tokio::fs::remove_dir_all(&self.cache_base_dir).await?;
        }
        Ok(())
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<CacheStats> {
        let mut stats = CacheStats::default();

        // Count files and calculate sizes for each content type
        for entry in walkdir::WalkDir::new(&self.cache_base_dir)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    stats.total_files += 1;
                    stats.total_size += metadata.len();

                    let path = entry.path();
                    if path.to_string_lossy().contains("config") {
                        stats.config_files += 1;
                        stats.config_size += metadata.len();
                    } else if path.to_string_lossy().contains("patch") {
                        stats.patch_files += 1;
                        stats.patch_size += metadata.len();
                    } else if path.to_string_lossy().contains("data") {
                        stats.data_files += 1;
                        stats.data_size += metadata.len();
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Download BuildConfig from CDN with caching
    ///
    /// BuildConfig files are stored at `{path}/config/{hash}`
    pub async fn download_build_config(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<CachedResponse> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash).await
    }

    /// Download CDNConfig from CDN with caching
    ///
    /// CDNConfig files are stored at `{path}/config/{hash}`
    pub async fn download_cdn_config(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<CachedResponse> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash).await
    }

    /// Download ProductConfig from CDN with caching
    ///
    /// ProductConfig files are stored at `{config_path}/{hash}`
    /// Note: This uses the config_path from CDN response, not the regular path
    pub async fn download_product_config(
        &self,
        cdn_host: &str,
        config_path: &str,
        hash: &str,
    ) -> Result<CachedResponse> {
        self.download(cdn_host, config_path, hash).await
    }

    /// Download KeyRing from CDN with caching
    ///
    /// KeyRing files are stored at `{path}/config/{hash}`
    pub async fn download_key_ring(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<CachedResponse> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash).await
    }

    /// Download data file from CDN with caching
    ///
    /// Data files are stored at `{path}/data/{hash}`
    pub async fn download_data(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<CachedResponse> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(cdn_host, &data_path, hash).await
    }

    /// Download patch file from CDN with caching
    ///
    /// Patch files are stored at `{path}/patch/{hash}`
    pub async fn download_patch(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<CachedResponse> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download(cdn_host, &patch_path, hash).await
    }
}

/// Response wrapper that indicates whether content came from cache
pub struct CachedResponse {
    /// The response data
    data: Bytes,
    /// Whether this response came from cache
    from_cache: bool,
}

impl CachedResponse {
    /// Create a response from cache
    fn from_cache(data: Bytes) -> Self {
        Self {
            data,
            from_cache: true,
        }
    }

    /// Create a response from network
    fn from_network(data: Bytes) -> Self {
        Self {
            data,
            from_cache: false,
        }
    }

    /// Check if this response came from cache
    pub fn is_from_cache(&self) -> bool {
        self.from_cache
    }

    /// Get the response data as bytes
    pub async fn bytes(self) -> Result<Bytes> {
        Ok(self.data)
    }

    /// Get the response data as text
    pub async fn text(self) -> Result<String> {
        Ok(String::from_utf8(self.data.to_vec())?)
    }

    /// Get the content length
    pub fn content_length(&self) -> usize {
        self.data.len()
    }
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
    async fn test_cached_cdn_client_creation() {
        let client = CachedCdnClient::new().await.unwrap();
        assert!(client.enabled);
    }

    #[tokio::test]
    async fn test_content_type_detection() {
        assert_eq!(
            ContentType::from_path("tpr/configs/data/config"),
            ContentType::Config
        );
        assert_eq!(
            ContentType::from_path("tpr/wow/config"),
            ContentType::Config
        );
        assert_eq!(ContentType::from_path("config"), ContentType::Config);
        assert_eq!(ContentType::from_path("tpr/wow/data"), ContentType::Data);
        assert_eq!(ContentType::from_path("tpr/wow/patch"), ContentType::Patch);
        assert_eq!(ContentType::from_path("tpr/wow"), ContentType::Data);
    }

    #[tokio::test]
    async fn test_cache_enabling() {
        let mut client = CachedCdnClient::new().await.unwrap();

        client.set_caching_enabled(false);
        assert!(!client.enabled);

        client.set_caching_enabled(true);
        assert!(client.enabled);
    }

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

        assert_eq!(client.cache_dir(), &temp_dir.path().to_path_buf());
    }
}

//! Cached wrapper for TACT HTTP client
//!
//! This module provides a caching layer for TactClient that stores responses
//! in BPSV format with sequence number tracking similar to Ribbit.
//!
//! **Important**: This caches TACT protocol metadata responses (versions, CDN configs, BGDL),
//! NOT actual game content files. The TACT protocol provides:
//! - Version information about available game builds
//! - CDN server configuration (which servers host the content)
//! - Background download settings
//!
//! For caching actual CDN content files, a separate caching layer should be
//! implemented for the ngdp-cdn crate.
//!
//! # Example
//!
//! ```no_run
//! use ngdp_cache::cached_tact_client::CachedTactClient;
//! use tact_client::{Region, ProtocolVersion};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a cached client
//! let client = CachedTactClient::new(Region::US, ProtocolVersion::V1).await?;
//!
//! // Use it exactly like TactClient - caching is transparent
//! let versions = client.get_versions_parsed("wow").await?;
//! println!("Found {} versions", versions.len());
//!
//! // Subsequent calls use cache based on sequence numbers
//! let versions2 = client.get_versions_parsed("wow").await?;  // This will be from cache!
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, trace};

use tact_client::{CdnEntry, HttpClient, ProtocolVersion, Region, VersionEntry};

use crate::{Result, ensure_dir, get_cache_dir};

/// Default TTL for version responses (5 minutes)
const VERSIONS_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// Default TTL for CDN and BGDL responses (30 minutes)
const CDN_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

/// Metadata for cached responses
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CacheMetadata {
    /// Unix timestamp when cached
    timestamp: u64,
    /// TTL in seconds
    ttl_seconds: u64,
    /// Region
    region: String,
    /// Protocol version (v1 or v2)
    protocol: String,
    /// Product name
    product: String,
    /// Endpoint type (versions, cdns, bgdl)
    endpoint: String,
    /// Sequence number from response
    sequence: Option<u64>,
    /// Response size in bytes
    response_size: usize,
}

/// Endpoint types for TACT protocol
#[derive(Debug, Clone, Copy, PartialEq)]
enum TactEndpoint {
    Versions,
    Cdns,
    Bgdl,
}

impl TactEndpoint {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Versions => "versions",
            Self::Cdns => "cdns",
            Self::Bgdl => "bgdl",
        }
    }

    fn ttl(&self) -> Duration {
        match self {
            Self::Versions => VERSIONS_CACHE_TTL,
            Self::Cdns | Self::Bgdl => CDN_CACHE_TTL,
        }
    }
}

/// A caching wrapper around TactClient
pub struct CachedTactClient {
    /// The underlying TACT HTTP client
    client: HttpClient,
    /// Base directory for cache
    cache_dir: PathBuf,
    /// Whether caching is enabled
    enabled: bool,
}

impl CachedTactClient {
    /// Create a new cached TACT client
    pub async fn new(region: Region, protocol: ProtocolVersion) -> Result<Self> {
        let client = HttpClient::new(region, protocol)?;
        let cache_dir = get_cache_dir()?.join("tact");
        ensure_dir(&cache_dir).await?;

        debug!(
            "Initialized cached TACT client for region {:?}, protocol {:?}",
            region, protocol
        );

        Ok(Self {
            client,
            cache_dir,
            enabled: true,
        })
    }

    /// Create a new cached client with custom cache directory
    pub async fn with_cache_dir(
        region: Region,
        protocol: ProtocolVersion,
        cache_dir: PathBuf,
    ) -> Result<Self> {
        let client = HttpClient::new(region, protocol)?;
        ensure_dir(&cache_dir).await?;

        Ok(Self {
            client,
            cache_dir,
            enabled: true,
        })
    }

    /// Create from an existing HTTP client
    pub async fn with_client(client: HttpClient) -> Result<Self> {
        let cache_dir = get_cache_dir()?.join("tact");
        ensure_dir(&cache_dir).await?;

        Ok(Self {
            client,
            cache_dir,
            enabled: true,
        })
    }

    /// Enable or disable caching
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the cache path for an endpoint
    fn get_cache_path(
        &self,
        product: &str,
        endpoint: TactEndpoint,
        sequence: Option<u64>,
    ) -> PathBuf {
        let region = self.client.region().to_string();
        let protocol = match self.client.version() {
            ProtocolVersion::V1 => "v1",
            ProtocolVersion::V2 => "v2",
        };

        let seq = sequence.unwrap_or(0);
        let filename = format!("{}-{}.bpsv", endpoint.as_str(), seq);

        self.cache_dir
            .join(region)
            .join(protocol)
            .join(product)
            .join(filename)
    }

    /// Get the metadata path for an endpoint
    fn get_metadata_path(
        &self,
        product: &str,
        endpoint: TactEndpoint,
        sequence: Option<u64>,
    ) -> PathBuf {
        let mut path = self.get_cache_path(product, endpoint, sequence);
        path.set_extension("meta");
        path
    }

    /// Extract sequence number from TACT response data
    fn extract_sequence_number(&self, data: &str) -> Option<u64> {
        // Look for "## seqn = 12345" pattern
        for line in data.lines() {
            if line.starts_with("## seqn = ") {
                if let Some(seqn_str) = line.strip_prefix("## seqn = ") {
                    if let Ok(seqn) = seqn_str.trim().parse::<u64>() {
                        return Some(seqn);
                    }
                }
            }
        }
        None
    }

    /// Find the most recent valid cached file for an endpoint
    async fn find_cached_file(
        &self,
        product: &str,
        endpoint: TactEndpoint,
    ) -> Option<(PathBuf, u64)> {
        if !self.enabled {
            return None;
        }

        let region = self.client.region().to_string();
        let protocol = match self.client.version() {
            ProtocolVersion::V1 => "v1",
            ProtocolVersion::V2 => "v2",
        };

        let cache_subdir = self.cache_dir.join(&region).join(protocol).join(product);
        if tokio::fs::metadata(&cache_subdir).await.is_err() {
            return None;
        }

        let prefix = format!("{}-", endpoint.as_str());
        let ttl = endpoint.ttl();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut best_file: Option<(PathBuf, u64)> = None;
        let mut best_seqn: u64 = 0;

        // Read directory and find matching files
        if let Ok(mut entries) = tokio::fs::read_dir(&cache_subdir).await {
            while let Some(entry) = entries.next_entry().await.ok()? {
                let path = entry.path();
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if this file matches our endpoint pattern
                    if filename.starts_with(&prefix) && filename.ends_with(".bpsv") {
                        // Extract sequence number from filename
                        if let Some(seqn_part) = filename
                            .strip_prefix(&prefix)
                            .and_then(|s| s.strip_suffix(".bpsv"))
                        {
                            if let Ok(seqn) = seqn_part.parse::<u64>() {
                                // Check if this file is still valid
                                let meta_path = path.with_extension("meta");
                                if let Ok(metadata_str) =
                                    tokio::fs::read_to_string(&meta_path).await
                                {
                                    if let Ok(metadata) =
                                        serde_json::from_str::<CacheMetadata>(&metadata_str)
                                    {
                                        if now.saturating_sub(metadata.timestamp) < ttl.as_secs()
                                            && seqn > best_seqn
                                        {
                                            best_file = Some((path.clone(), seqn));
                                            best_seqn = seqn;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        best_file
    }

    /// Write response to cache
    async fn write_to_cache(
        &self,
        product: &str,
        endpoint: TactEndpoint,
        data: &str,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Extract sequence number from the response data
        let sequence = self.extract_sequence_number(data);

        let cache_path = self.get_cache_path(product, endpoint, sequence);
        let meta_path = self.get_metadata_path(product, endpoint, sequence);

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            ensure_dir(parent).await?;
        }

        // Write the response data
        trace!(
            "Writing {} bytes to TACT cache: {:?}",
            data.len(),
            cache_path
        );
        tokio::fs::write(&cache_path, data).await?;

        // Create and write metadata
        let metadata = CacheMetadata {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl_seconds: endpoint.ttl().as_secs(),
            region: self.client.region().to_string(),
            protocol: match self.client.version() {
                ProtocolVersion::V1 => "v1".to_string(),
                ProtocolVersion::V2 => "v2".to_string(),
            },
            product: product.to_string(),
            endpoint: endpoint.as_str().to_string(),
            sequence,
            response_size: data.len(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        tokio::fs::write(&meta_path, metadata_json).await?;

        Ok(())
    }

    /// Read response from cache
    async fn read_from_cache(&self, product: &str, endpoint: TactEndpoint) -> Result<String> {
        if let Some((cache_path, _seqn)) = self.find_cached_file(product, endpoint).await {
            trace!("Reading from TACT cache: {:?}", cache_path);
            Ok(tokio::fs::read_to_string(&cache_path).await?)
        } else {
            Err(crate::Error::CacheEntryNotFound(format!(
                "No valid cache for {}/{}/{}",
                self.client.region(),
                product,
                endpoint.as_str()
            )))
        }
    }

    /// Get versions with caching
    pub async fn get_versions(&self, product: &str) -> Result<reqwest::Response> {
        // For raw Response objects, we need to use the underlying client directly
        // as we can't reconstruct Response from cached data
        Ok(self.client.get_versions(product).await?)
    }

    /// Get versions with parsed response and caching
    pub async fn get_versions_parsed(&self, product: &str) -> Result<Vec<VersionEntry>> {
        let endpoint = TactEndpoint::Versions;

        // Check cache first
        if self.enabled {
            if let Ok(cached_data) = self.read_from_cache(product, endpoint).await {
                debug!("Cache hit for TACT {}/{}", product, endpoint.as_str());
                // Parse the cached data
                return Ok(tact_client::parse_versions(&cached_data)?);
            }
        }

        // Cache miss - fetch from server
        debug!(
            "Cache miss for TACT {}/{}, fetching from server",
            product,
            endpoint.as_str()
        );
        let response = self.client.get_versions(product).await?;
        let text = response.text().await?;

        // Cache the response
        if let Err(e) = self.write_to_cache(product, endpoint, &text).await {
            debug!("Failed to write to TACT cache: {}", e);
        }

        // Parse and return
        Ok(tact_client::parse_versions(&text)?)
    }

    /// Get CDN configuration with caching
    ///
    /// **Important**: This returns CDN server configuration from the TACT `/cdns` endpoint,
    /// NOT actual CDN content. The TACT protocol has three metadata endpoints:
    /// - `/versions` - game version information
    /// - `/cdns` - CDN server configuration (which CDN servers to use)
    /// - `/bgdl` - background download configuration
    ///
    /// For actual CDN content caching, use the ngdp-cdn crate with its own caching layer.
    pub async fn get_cdns(&self, product: &str) -> Result<reqwest::Response> {
        // For raw Response objects, we need to use the underlying client directly
        Ok(self.client.get_cdns(product).await?)
    }

    /// Get CDN configuration with parsed response and caching
    ///
    /// **Important**: This returns CDN server configuration, NOT actual CDN content.
    /// See `get_cdns()` documentation for details.
    pub async fn get_cdns_parsed(&self, product: &str) -> Result<Vec<CdnEntry>> {
        let endpoint = TactEndpoint::Cdns;

        // Check cache first
        if self.enabled {
            if let Ok(cached_data) = self.read_from_cache(product, endpoint).await {
                debug!("Cache hit for TACT {}/{}", product, endpoint.as_str());
                // Parse the cached data
                return Ok(tact_client::parse_cdns(&cached_data)?);
            }
        }

        // Cache miss - fetch from server
        debug!(
            "Cache miss for TACT {}/{}, fetching from server",
            product,
            endpoint.as_str()
        );
        let response = self.client.get_cdns(product).await?;
        let text = response.text().await?;

        // Cache the response
        if let Err(e) = self.write_to_cache(product, endpoint, &text).await {
            debug!("Failed to write to TACT cache: {}", e);
        }

        // Parse and return
        Ok(tact_client::parse_cdns(&text)?)
    }

    /// Get BGDL with caching
    pub async fn get_bgdl(&self, product: &str) -> Result<reqwest::Response> {
        // For raw Response objects, we need to use the underlying client directly
        Ok(self.client.get_bgdl(product).await?)
    }

    /// Get BGDL with parsed response and caching
    pub async fn get_bgdl_parsed(
        &self,
        product: &str,
    ) -> Result<Vec<tact_client::response_types::BgdlEntry>> {
        let endpoint = TactEndpoint::Bgdl;

        // Check cache first
        if self.enabled {
            if let Ok(cached_data) = self.read_from_cache(product, endpoint).await {
                debug!("Cache hit for TACT {}/{}", product, endpoint.as_str());
                // Parse the cached data
                return Ok(tact_client::response_types::parse_bgdl(&cached_data)?);
            }
        }

        // Cache miss - fetch from server
        debug!(
            "Cache miss for TACT {}/{}, fetching from server",
            product,
            endpoint.as_str()
        );
        let response = self.client.get_bgdl(product).await?;
        let text = response.text().await?;

        // Cache the response
        if let Err(e) = self.write_to_cache(product, endpoint, &text).await {
            debug!("Failed to write to TACT cache: {}", e);
        }

        // Parse and return
        Ok(tact_client::response_types::parse_bgdl(&text)?)
    }

    /// Get raw response from any path with caching
    pub async fn get(&self, path: &str) -> Result<reqwest::Response> {
        // For custom paths, we don't cache as we can't determine the endpoint type
        Ok(self.client.get(path).await?)
    }

    /// Download a file from CDN (no caching for binary files)
    ///
    /// **Note**: This method downloads actual game content from CDN servers and does NOT
    /// cache the response. CDN content caching should be implemented in a separate layer
    /// (e.g., in ngdp-cdn crate) to handle binary data efficiently with proper storage
    /// in ~/.cache/ngdp/cdn/ instead of the TACT metadata cache.
    pub async fn download_file(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<reqwest::Response> {
        Ok(self.client.download_file(cdn_host, path, hash).await?)
    }

    /// Get the underlying HTTP client
    pub fn inner(&self) -> &HttpClient {
        &self.client
    }

    /// Get mutable access to the underlying HTTP client
    pub fn inner_mut(&mut self) -> &mut HttpClient {
        &mut self.client
    }

    /// Clear all cached responses
    pub async fn clear_cache(&self) -> Result<()> {
        debug!("Clearing all cached TACT responses");

        let region = self.client.region().to_string();
        let protocol = match self.client.version() {
            ProtocolVersion::V1 => "v1",
            ProtocolVersion::V2 => "v2",
        };

        let cache_subdir = self.cache_dir.join(region).join(protocol);
        if tokio::fs::metadata(&cache_subdir).await.is_ok() {
            clear_directory_recursively(&cache_subdir).await?;
        }

        Ok(())
    }

    /// Clear expired cache entries
    pub async fn clear_expired(&self) -> Result<()> {
        debug!("Clearing expired TACT cache entries");

        let region = self.client.region().to_string();
        let protocol = match self.client.version() {
            ProtocolVersion::V1 => "v1",
            ProtocolVersion::V2 => "v2",
        };

        let cache_subdir = self.cache_dir.join(region).join(protocol);
        if tokio::fs::metadata(&cache_subdir).await.is_ok() {
            clear_expired_in_directory(&cache_subdir).await?;
        }

        Ok(())
    }
}

/// Recursively clear all files in a directory
fn clear_directory_recursively(
    dir: &PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                if metadata.is_dir() {
                    clear_directory_recursively(&path).await?;
                } else {
                    tokio::fs::remove_file(&path).await?;
                }
            }
        }
        Ok(())
    })
}

/// Clear expired entries in a directory
fn clear_expired_in_directory(
    dir: &PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                clear_expired_in_directory(&path).await?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("meta") {
                // Check if this metadata file indicates an expired entry
                if let Ok(metadata_str) = tokio::fs::read_to_string(&path).await {
                    if let Ok(metadata) = serde_json::from_str::<CacheMetadata>(&metadata_str) {
                        if now.saturating_sub(metadata.timestamp) >= metadata.ttl_seconds {
                            // Remove both the data and metadata files
                            let data_path = path.with_extension("bpsv");
                            let _ = tokio::fs::remove_file(&data_path).await;
                            let _ = tokio::fs::remove_file(&path).await;
                            trace!("Removed expired TACT cache entry: {:?}", data_path);
                        }
                    }
                }
            }
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_properties() {
        assert_eq!(TactEndpoint::Versions.as_str(), "versions");
        assert_eq!(TactEndpoint::Cdns.as_str(), "cdns");
        assert_eq!(TactEndpoint::Bgdl.as_str(), "bgdl");

        assert_eq!(TactEndpoint::Versions.ttl(), VERSIONS_CACHE_TTL);
        assert_eq!(TactEndpoint::Cdns.ttl(), CDN_CACHE_TTL);
        assert_eq!(TactEndpoint::Bgdl.ttl(), CDN_CACHE_TTL);
    }

    #[test]
    fn test_sequence_number_extraction() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedTactClient::new(Region::US, ProtocolVersion::V1)
                .await
                .unwrap();

            // Test with sequence number
            let data_with_seqn = "Product!STRING:0|Seqn!DEC:4\n## seqn = 3020098\nwow|12345";
            assert_eq!(
                client.extract_sequence_number(data_with_seqn),
                Some(3020098)
            );

            // Test without sequence number
            let data_no_seqn = "Product!STRING:0|Seqn!DEC:4\nwow|12345";
            assert_eq!(client.extract_sequence_number(data_no_seqn), None);

            // Test with malformed sequence
            let data_bad_seqn = "## seqn = not_a_number\nwow|12345";
            assert_eq!(client.extract_sequence_number(data_bad_seqn), None);
        });
    }

    #[test]
    fn test_cache_path_generation() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedTactClient::new(Region::US, ProtocolVersion::V1)
                .await
                .unwrap();

            let path = client.get_cache_path("wow", TactEndpoint::Versions, Some(12345));
            assert!(
                path.to_string_lossy()
                    .contains("us/v1/wow/versions-12345.bpsv")
            );

            let path_no_seq = client.get_cache_path("d3", TactEndpoint::Cdns, None);
            assert!(
                path_no_seq
                    .to_string_lossy()
                    .contains("us/v1/d3/cdns-0.bpsv")
            );
        });
    }

    #[test]
    fn test_api_methods_compile() {
        // This test just verifies that all API methods compile correctly
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let client = CachedTactClient::new(Region::EU, ProtocolVersion::V2)
                .await
                .unwrap();

            // These would all compile and work in real usage:
            // let _ = client.get_versions_parsed("wow").await;
            // let _ = client.get_cdns_parsed("wow").await;
            // let _ = client.get_bgdl_parsed("wow").await;
            // let _ = client.clear_cache().await;
            // let _ = client.clear_expired().await;

            // Just verify the client was created
            assert_eq!(client.inner().region(), Region::EU);
            assert_eq!(client.inner().version(), ProtocolVersion::V2);
        });
    }
}

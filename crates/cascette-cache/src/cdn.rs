//! CDN integration for cache miss handling
//!
//! This module provides CDN client functionality for fetching content
//! when cache misses occur, including retry logic, range requests,
//! and connection pooling.

use crate::{
    error::{NgdpCacheError, NgdpCacheResult},
    ngdp::{ArchiveCache, ContentAddressedCache, NgdpResolutionCache},
};
use bytes::Bytes;
use cascette_crypto::{ContentKey, EncodingKey};
use std::{sync::Arc, time::Duration};

/// CDN client configuration
#[derive(Debug, Clone)]
pub struct CdnConfig {
    /// Base CDN URLs (multiple for failover)
    pub cdn_urls: Vec<String>,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Timeout for individual requests
    pub request_timeout: Duration,
    /// Connection pool size
    pub connection_pool_size: usize,
    /// Enable HTTP/2
    pub enable_http2: bool,
    /// User agent string
    pub user_agent: String,
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self {
            cdn_urls: vec![
                "https://level3.blizzard.com".to_string(),
                "https://eu.cdn.blizzard.com".to_string(),
                "https://us.cdn.blizzard.com".to_string(),
            ],
            max_retries: 3,
            request_timeout: Duration::from_secs(30),
            connection_pool_size: 10,
            enable_http2: true,
            user_agent: "cascette-cache/1.0".to_string(),
        }
    }
}

/// CDN client for fetching content
pub struct CdnClient {
    /// Configuration
    config: CdnConfig,
    /// HTTP client (mock for now, would use reqwest in real impl)
    client: Arc<MockHttpClient>,
    /// Metrics
    metrics: Arc<std::sync::RwLock<CdnMetrics>>,
}

/// Metrics for CDN operations
#[derive(Debug, Default, Clone)]
pub struct CdnMetrics {
    /// Total requests made
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Total bytes downloaded
    pub bytes_downloaded: u64,
    /// Average download speed (bytes/sec)
    pub avg_download_speed: u64,
    /// Total retries
    pub total_retries: u64,
}

impl CdnClient {
    /// Create a new CDN client
    pub fn new(config: CdnConfig) -> Self {
        Self {
            config,
            client: Arc::new(MockHttpClient::new()),
            metrics: Arc::new(std::sync::RwLock::new(CdnMetrics::default())),
        }
    }

    /// Fetch content by key
    #[allow(clippy::unused_async)] // Keep async for consistent public API
    pub async fn fetch_content(&self, content_key: ContentKey) -> NgdpCacheResult<Bytes> {
        let hex_key = hex::encode(content_key.to_string());
        let path = format!("data/{}/{}/{}", &hex_key[0..2], &hex_key[2..4], hex_key);

        self.fetch_with_retry(&path)
    }

    /// Fetch encoding file
    #[allow(clippy::unused_async)] // Keep async for consistent public API
    pub async fn fetch_encoding(&self, encoding_key: EncodingKey) -> NgdpCacheResult<Bytes> {
        let hex_key = hex::encode(encoding_key.to_string());
        let path = format!("data/{}/{}/{}", &hex_key[0..2], &hex_key[2..4], hex_key);

        self.fetch_with_retry(&path)
    }

    /// Fetch config file
    #[allow(clippy::unused_async)] // Keep async for consistent public API
    pub async fn fetch_config(&self, config_hash: &str) -> NgdpCacheResult<Bytes> {
        let path = format!(
            "config/{}/{}/{}",
            &config_hash[0..2],
            &config_hash[2..4],
            config_hash
        );
        self.fetch_with_retry(&path)
    }

    /// Fetch archive with range request
    #[allow(clippy::unused_async)] // Keep async for consistent public API
    pub async fn fetch_archive_range(
        &self,
        archive_name: &str,
        offset: u64,
        length: u32,
    ) -> NgdpCacheResult<Bytes> {
        let path = format!("data/{archive_name}");
        self.fetch_range_with_retry(&path, offset, length)
    }

    /// Fetch with retry logic
    fn fetch_with_retry(&self, path: &str) -> NgdpCacheResult<Bytes> {
        // Update initial metrics
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string())
            })?;
            metrics.total_requests += 1;
        }

        // Mock implementation - just use the first CDN URL
        if let Some(cdn_url) = self.config.cdn_urls.first() {
            let url = format!("{cdn_url}/{path}");
            let data = self.client.get(&url);
            {
                let mut metrics = self.metrics.write().map_err(|_| {
                    NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string())
                })?;
                metrics.successful_requests += 1;
                metrics.bytes_downloaded += data.len() as u64;
            }
            return Ok(data);
        }

        // This should never be reached with the mock implementation
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string())
            })?;
            metrics.failed_requests += 1;
        }
        Err(NgdpCacheError::NetworkError(
            "All CDN attempts failed".to_string(),
        ))
    }

    /// Fetch range with retry logic
    fn fetch_range_with_retry(
        &self,
        path: &str,
        offset: u64,
        length: u32,
    ) -> NgdpCacheResult<Bytes> {
        // Update initial metrics
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string())
            })?;
            metrics.total_requests += 1;
        }

        // Mock implementation - just use the first CDN URL
        if let Some(cdn_url) = self.config.cdn_urls.first() {
            let url = format!("{cdn_url}/{path}");
            let data = self.client.get_range(&url, offset, length);
            {
                let mut metrics = self.metrics.write().map_err(|_| {
                    NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string())
                })?;
                metrics.successful_requests += 1;
                metrics.bytes_downloaded += data.len() as u64;
            }
            return Ok(data);
        }

        // This should never be reached with the mock implementation
        {
            let mut metrics = self.metrics.write().map_err(|_| {
                NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string())
            })?;
            metrics.failed_requests += 1;
        }
        Err(NgdpCacheError::NetworkError(
            "All CDN range attempts failed".to_string(),
        ))
    }

    /// Get CDN metrics
    pub fn metrics(&self) -> NgdpCacheResult<CdnMetrics> {
        self.metrics
            .read()
            .map_err(|_| NgdpCacheError::NetworkError("CDN metrics lock poisoned".to_string()))
            .map(|guard| guard.clone())
    }
}

/// Mock HTTP client for testing (would be replaced with reqwest in real impl)
struct MockHttpClient;

#[allow(clippy::unused_self)] // Mock methods need &self for trait consistency
impl MockHttpClient {
    fn new() -> Self {
        Self
    }

    fn get(&self, _url: &str) -> Bytes {
        // Mock implementation
        Bytes::from("mock content data")
    }

    fn get_range(&self, _url: &str, _offset: u64, length: u32) -> Bytes {
        // Mock implementation
        Bytes::from(vec![0u8; length as usize])
    }
}

/// Cache with CDN fallback
///
/// This wrapper adds CDN fetching capability to any cache implementation,
/// automatically fetching from CDN on cache misses.
pub struct CdnBackedCache<C, K> {
    /// Underlying cache
    cache: Arc<C>,
    /// CDN client
    cdn: Arc<CdnClient>,
    /// Phantom data for key type
    _phantom: std::marker::PhantomData<K>,
}

impl<C, K> CdnBackedCache<C, K> {
    /// Create a new CDN-backed cache
    pub fn new(cache: Arc<C>, cdn: Arc<CdnClient>) -> Self {
        Self {
            cache,
            cdn,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// CDN-backed NGDP resolution cache
pub type CdnNgdpResolutionCache = CdnBackedCache<NgdpResolutionCache, ContentKey>;

impl CdnNgdpResolutionCache {
    /// Resolve with CDN fallback
    pub async fn resolve_with_fallback(
        &self,
        root_content_key: ContentKey,
        file_path: &str,
    ) -> NgdpCacheResult<Option<ContentKey>> {
        // Try cache first
        if let Some(content_key) = self
            .cache
            .resolve_file_to_content(root_content_key, file_path)
            .await?
        {
            return Ok(Some(content_key));
        }

        // Cache miss - fetch root file from CDN
        let root_data = self.cdn.fetch_content(root_content_key).await?;
        self.cache
            .cache_root_file(root_content_key, root_data)
            .await?;

        // Try again with cached data
        self.cache
            .resolve_file_to_content(root_content_key, file_path)
            .await
    }
}

/// CDN-backed content cache
pub type CdnContentCache<C> = CdnBackedCache<ContentAddressedCache<C>, ContentKey>;

impl<C> CdnContentCache<C>
where
    C: crate::traits::AsyncCache<crate::key::BlteBlockKey> + Send + Sync,
{
    /// Get content with CDN fallback
    pub async fn get_with_fallback(&self, content_key: ContentKey) -> NgdpCacheResult<Bytes> {
        // Try cache first
        if let Some(data) = self.cache.get_validated(content_key).await? {
            return Ok(data);
        }

        // Cache miss - fetch from CDN
        let data = self.cdn.fetch_content(content_key).await?;
        self.cache.put_validated(content_key, data.clone()).await?;
        Ok(data)
    }
}

/// CDN-backed archive cache
pub type CdnArchiveCache<C> = CdnBackedCache<ArchiveCache<C>, String>;

impl<C> CdnArchiveCache<C>
where
    C: crate::traits::AsyncCache<crate::key::ArchiveRangeKey> + Send + Sync,
{
    /// Get archive range with CDN fallback
    pub async fn get_range_with_fallback(
        &self,
        archive_id: &str,
        offset: u64,
        length: u32,
    ) -> NgdpCacheResult<Bytes> {
        // Try cache first
        if let Some(data) = self.cache.get_range(archive_id, offset, length).await? {
            return Ok(data);
        }

        // Cache miss - fetch from CDN
        let data = self
            .cdn
            .fetch_archive_range(archive_id, offset, length)
            .await?;
        self.cache
            .put_range(archive_id, offset, length, data.clone())
            .await?;
        Ok(data)
    }
}

/// Builder for creating CDN-backed cache stack
pub struct CdnCacheBuilder {
    cdn_config: CdnConfig,
    enable_validation: bool,
    enable_streaming: bool,
}

impl CdnCacheBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            cdn_config: CdnConfig::default(),
            enable_validation: true,
            enable_streaming: false,
        }
    }

    /// Set CDN configuration
    pub fn with_cdn_config(mut self, config: CdnConfig) -> Self {
        self.cdn_config = config;
        self
    }

    /// Enable or disable validation
    pub fn with_validation(mut self, enable: bool) -> Self {
        self.enable_validation = enable;
        self
    }

    /// Enable or disable streaming
    pub fn with_streaming(mut self, enable: bool) -> Self {
        self.enable_streaming = enable;
        self
    }

    /// Build the CDN-backed cache stack
    pub fn build(self) -> NgdpCacheResult<CdnCacheStack> {
        let cdn = Arc::new(CdnClient::new(self.cdn_config));

        Ok(CdnCacheStack {
            cdn,
            enable_validation: self.enable_validation,
            enable_streaming: self.enable_streaming,
        })
    }
}

impl Default for CdnCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete CDN-backed cache stack
pub struct CdnCacheStack {
    /// CDN client
    pub cdn: Arc<CdnClient>,
    /// Whether validation is enabled
    pub enable_validation: bool,
    /// Whether streaming is enabled
    pub enable_streaming: bool,
}

impl CdnCacheStack {
    /// Get CDN client
    pub fn cdn(&self) -> &Arc<CdnClient> {
        &self.cdn
    }

    /// Create a new CDN-backed NGDP resolution cache
    pub fn create_resolution_cache(
        &self,
        config: crate::ngdp::NgdpResolutionConfig,
    ) -> NgdpCacheResult<CdnNgdpResolutionCache> {
        let cache = Arc::new(NgdpResolutionCache::new(config)?);
        Ok(CdnNgdpResolutionCache::new(cache, self.cdn.clone()))
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::match_same_arms)] // Test match arms have semantic meaning
mod tests {
    use super::*;

    #[test]
    fn test_cdn_config_default() {
        let config = CdnConfig::default();
        assert_eq!(config.cdn_urls.len(), 3);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.request_timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_cdn_client_creation() {
        let config = CdnConfig::default();
        let client = CdnClient::new(config);
        let metrics = client.metrics().expect("metrics should succeed");
        assert_eq!(metrics.total_requests, 0);
    }

    #[tokio::test]
    async fn test_cdn_client_fetch() {
        let client = CdnClient::new(CdnConfig::default());
        let content_key = ContentKey::from_data(b"test");
        let result = client.fetch_content(content_key).await;
        assert!(result.is_ok());

        let metrics = client.metrics().expect("metrics should succeed");
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.successful_requests, 1);
    }

    #[tokio::test]
    async fn test_cdn_cache_builder() {
        let stack = CdnCacheBuilder::new()
            .with_validation(true)
            .with_streaming(false)
            .build()
            .expect("CDN cache stack should build successfully");

        assert!(stack.enable_validation);
        assert!(!stack.enable_streaming);
    }

    #[tokio::test]
    async fn test_cdn_backed_resolution_cache() {
        let config = crate::ngdp::NgdpResolutionConfig::default();
        let cache = Arc::new(
            NgdpResolutionCache::new(config).expect("NgdpResolutionCache creation should succeed"),
        );
        let cdn = Arc::new(CdnClient::new(CdnConfig::default()));
        let backed = CdnNgdpResolutionCache::new(cache, cdn);

        let root_key = ContentKey::from_data(b"root");
        let result = backed.resolve_with_fallback(root_key, "test.txt").await;

        // The mock CDN returns invalid data that won't parse as a RootFile,
        // so we expect an error or None result here
        match result {
            Ok(None) | Err(_) => {
                // Expected - file not found in cache and mock data doesn't parse,
                // or parse error from mock data
            }
            Ok(Some(_)) => {
                unreachable!("Should not have found content with mock data");
            }
        }
    }
}

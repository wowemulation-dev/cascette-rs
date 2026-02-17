//! HTTP client abstraction for CDN streaming operations
//!
//! Provides a trait-based abstraction layer for HTTP operations required by CDN streaming,
//! with concrete implementations for production use and testing.

use async_trait::async_trait;
use bytes::Bytes;

use super::{
    bootstrap::CdnBootstrap,
    config::StreamingConfig,
    error::StreamingError,
    path::{CdnUrlBuilder, ContentType},
    range::HttpRange,
};

/// CDN server information for failover and load balancing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnServer {
    /// CDN hostname (e.g., "level3.blizzard.com")
    pub host: String,
    /// Whether this server supports HTTPS
    pub supports_https: bool,
    /// Priority for server selection (lower = higher priority)
    pub priority: u32,
    /// Whether this is a fallback server (from `?fallback=1` query param)
    pub is_fallback: bool,
    /// Strict mode — do not fall back to other servers (from `?strict=1`)
    pub strict: bool,
    /// Maximum number of hosts to use (from `?maxhosts=N`)
    pub max_hosts: Option<u32>,
}

impl CdnServer {
    /// Create a new CDN server configuration
    pub fn new(host: String, supports_https: bool, priority: u32) -> Self {
        Self {
            host,
            supports_https,
            priority,
            is_fallback: false,
            strict: false,
            max_hosts: None,
        }
    }

    /// Create HTTPS-capable server with default priority
    pub fn https(host: String) -> Self {
        Self::new(host, true, 100)
    }

    /// Create HTTP-only server with default priority
    pub fn http(host: String) -> Self {
        Self::new(host, false, 200)
    }
}

/// HTTP client abstraction for CDN operations
///
/// This trait provides the minimum interface needed for streaming CDN operations,
/// enabling both production implementations (using reqwest) and mock implementations
/// for testing.
#[async_trait]
pub trait HttpClient: Send + Sync + 'static {
    /// Perform an HTTP GET request with optional range header
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `range` - Optional HTTP range specifier
    ///
    /// # Returns
    /// The response body as bytes, or an error if the request failed
    ///
    /// # Errors
    /// Returns `StreamingError` for network failures, HTTP errors, or timeout
    async fn get_range(&self, url: &str, range: Option<HttpRange>)
    -> Result<Bytes, StreamingError>;

    /// Get the size of a resource using HEAD request
    ///
    /// # Arguments
    /// * `url` - The URL to query
    ///
    /// # Returns
    /// The content length in bytes, if available
    ///
    /// # Errors
    /// Returns `StreamingError` for network failures or if content-length is not available
    async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError>;

    /// Check if the server supports range requests
    ///
    /// # Arguments
    /// * `url` - The URL to test
    ///
    /// # Returns
    /// True if the server accepts range requests
    ///
    /// # Errors
    /// Returns `StreamingError` for network failures
    async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError>;
}

/// Production HTTP client implementation using reqwest
#[derive(Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
    config: StreamingConfig,
    url_builder: CdnUrlBuilder,
    cdn_servers: Vec<CdnServer>,
}

impl ReqwestHttpClient {
    /// Create a new HTTP client with the specified configuration
    ///
    /// # Arguments
    /// * `config` - Streaming configuration including timeouts and connection limits
    ///
    /// # Returns
    /// A configured HTTP client ready for use
    ///
    /// # Errors
    /// Returns `StreamingError` if the underlying reqwest client cannot be created
    pub fn new(config: StreamingConfig) -> Result<Self, StreamingError> {
        crate::transport::ensure_crypto_provider();
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .connect_timeout(config.connect_timeout)
            .pool_idle_timeout(Some(config.connection_idle_timeout))
            .pool_max_idle_per_host(config.max_connections_per_host)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects))
            .user_agent("cascette-rs/0.1.0")
            .build()
            .map_err(|source| StreamingError::HttpClientSetup { source })?;

        Ok(Self {
            client,
            config,
            url_builder: CdnUrlBuilder::new(),
            cdn_servers: Vec::new(),
        })
    }

    /// Create HTTP client with CDN servers for failover
    ///
    /// # Arguments
    /// * `config` - Streaming configuration
    /// * `cdn_servers` - List of CDN servers for failover
    ///
    /// # Returns
    /// A configured HTTP client with CDN server rotation
    pub fn with_cdn_servers(
        config: StreamingConfig,
        mut cdn_servers: Vec<CdnServer>,
    ) -> Result<Self, StreamingError> {
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .connect_timeout(config.connect_timeout)
            .pool_idle_timeout(Some(config.connection_idle_timeout))
            .pool_max_idle_per_host(config.max_connections_per_host)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects))
            .user_agent("cascette-rs/0.1.0")
            .build()
            .map_err(|source| StreamingError::HttpClientSetup { source })?;

        // Sort servers by priority (lower = higher priority)
        cdn_servers.sort_by_key(|server| server.priority);

        Ok(Self {
            client,
            config,
            url_builder: CdnUrlBuilder::new(),
            cdn_servers,
        })
    }

    /// Get the current configuration
    pub fn config(&self) -> &StreamingConfig {
        &self.config
    }

    /// Cache CDN path for a product
    ///
    /// CRITICAL: Path must be extracted from CDN response, never hardcoded
    ///
    /// # Arguments
    /// * `product` - Product name (e.g., "wow", "wow_classic")
    /// * `path` - Path extracted from CDN response (e.g., "tpr/wow")
    pub fn cache_cdn_path(&mut self, product: String, path: String) {
        self.url_builder.cache_path(product, path);
    }

    /// Get cached CDN path for a product
    pub fn get_cached_path(&self, product: &str) -> Option<&str> {
        self.url_builder.get_cached_path(product)
    }

    /// Add CDN server for failover
    pub fn add_cdn_server(&mut self, server: CdnServer) {
        self.cdn_servers.push(server);
        self.cdn_servers.sort_by_key(|s| s.priority);
    }

    /// Get CDN content with automatic failover
    ///
    /// # Arguments
    /// * `product` - Product name for path lookup
    /// * `content_type` - Type of content to fetch
    /// * `hash` - Content hash
    /// * `range` - Optional byte range
    /// * `prefer_https` - Prefer HTTPS when available
    ///
    /// # Returns
    /// Content bytes or error with failover context
    pub async fn get_cdn_content(
        &self,
        product: &str,
        content_type: ContentType,
        hash: &str,
        range: Option<HttpRange>,
        prefer_https: bool,
    ) -> Result<Bytes, StreamingError> {
        if self.cdn_servers.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "No CDN servers configured for failover".to_string(),
            });
        }

        let mut last_error = None;

        for server in &self.cdn_servers {
            let use_https = prefer_https && server.supports_https;

            match self.url_builder.build_url_for_product(
                &server.host,
                product,
                content_type,
                hash,
                use_https,
            ) {
                Ok(url) => match self.get_range(&url, range).await {
                    Ok(data) => return Ok(data),
                    Err(e) => {
                        last_error = Some(StreamingError::CdnFailover {
                            server: server.host.clone(),
                            source: Box::new(e),
                        });
                    }
                },
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| StreamingError::Configuration {
            reason: "All CDN servers failed".to_string(),
        }))
    }

    /// Get product configuration content
    ///
    /// Product configs use special path: tpr/configs/data
    pub async fn get_product_config(
        &self,
        hash: &str,
        range: Option<HttpRange>,
        prefer_https: bool,
    ) -> Result<Bytes, StreamingError> {
        if self.cdn_servers.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "No CDN servers configured for failover".to_string(),
            });
        }

        let mut last_error = None;

        for server in &self.cdn_servers {
            let use_https = prefer_https && server.supports_https;

            match self
                .url_builder
                .build_product_config_url(&server.host, hash, use_https)
            {
                Ok(url) => match self.get_range(&url, range).await {
                    Ok(data) => return Ok(data),
                    Err(e) => {
                        last_error = Some(StreamingError::CdnFailover {
                            server: server.host.clone(),
                            source: Box::new(e),
                        });
                    }
                },
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| StreamingError::Configuration {
            reason: "All CDN servers failed for product config".to_string(),
        }))
    }

    /// Get list of configured CDN servers
    pub fn cdn_servers(&self) -> &[CdnServer] {
        &self.cdn_servers
    }

    /// Create client from bootstrap configuration
    ///
    /// # Arguments
    /// * `config` - Streaming configuration
    /// * `bootstrap` - Bootstrap configuration from Ribbit
    ///
    /// # Returns
    /// Configured HTTP client
    ///
    /// # Errors
    /// Returns `StreamingError` if bootstrap is invalid or has no servers
    pub fn from_bootstrap(
        config: StreamingConfig,
        bootstrap: &CdnBootstrap,
    ) -> Result<Self, StreamingError> {
        // Validate bootstrap first
        bootstrap.validate()?;

        let mut client = Self::with_cdn_servers(config, bootstrap.servers.clone())?;

        // Cache paths from bootstrap
        for (product, path) in &bootstrap.paths {
            client.cache_cdn_path(product.clone(), path.clone());
        }

        Ok(client)
    }

    /// Create client with fallback configuration
    ///
    /// Uses community mirrors as fallback when official servers are unavailable.
    pub fn with_fallback(config: StreamingConfig) -> Result<Self, StreamingError> {
        let fallback = CdnBootstrap::fallback_configuration();
        Self::from_bootstrap(config, &fallback)
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get_range(
        &self,
        url: &str,
        range: Option<HttpRange>,
    ) -> Result<Bytes, StreamingError> {
        let mut request = self.client.get(url);

        if let Some(range) = range {
            request = request.header("Range", range.to_header_value());
        }

        let response = request
            .send()
            .await
            .map_err(|source| StreamingError::NetworkRequest { source })?;

        // Check for successful status codes
        let status = response.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(StreamingError::HttpStatus {
                status_code: status.as_u16(),
                url: url.to_string(),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|source| StreamingError::NetworkRequest { source })?;

        Ok(bytes)
    }

    async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError> {
        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|source| StreamingError::NetworkRequest { source })?;

        if !response.status().is_success() {
            return Err(StreamingError::HttpStatus {
                status_code: response.status().as_u16(),
                url: url.to_string(),
            });
        }

        response
            .content_length()
            .ok_or_else(|| StreamingError::MissingContentLength {
                url: url.to_string(),
            })
    }

    async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError> {
        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|source| StreamingError::NetworkRequest { source })?;

        if !response.status().is_success() {
            return Err(StreamingError::HttpStatus {
                status_code: response.status().as_u16(),
                url: url.to_string(),
            });
        }

        // Check for Accept-Ranges header
        Ok(response
            .headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "bytes"))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::uninlined_format_args)]
mod tests {
    use super::*;
    use crate::cdn::streaming::config::StreamingConfig;
    use mockall::mock;

    mock! {
        TestHttpClient {}

        #[async_trait]
        impl HttpClient for TestHttpClient {
            async fn get_range(&self, url: &str, range: Option<HttpRange>) -> Result<Bytes, StreamingError>;
            async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError>;
            async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError>;
        }
    }

    #[test]
    fn test_cdn_server_creation() {
        let server = CdnServer::new("level3.blizzard.com".to_string(), true, 100);
        assert_eq!(server.host, "level3.blizzard.com");
        assert!(server.supports_https);
        assert_eq!(server.priority, 100);

        let https_server = CdnServer::https("test-cdn.example.com".to_string());
        assert!(https_server.supports_https);
        assert_eq!(https_server.priority, 100);

        let http_server = CdnServer::http("test-cdn.example.com".to_string());
        assert!(!http_server.supports_https);
        assert_eq!(http_server.priority, 200);
    }

    #[test]
    fn test_reqwest_client_creation() {
        let config = StreamingConfig::default();
        let client = ReqwestHttpClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_reqwest_client_with_cdn_servers() {
        let config = StreamingConfig::default();
        let servers = vec![
            CdnServer::https("level3.blizzard.com".to_string()),
            CdnServer::http("cdn.arctium.tools".to_string()),
        ];

        let client = ReqwestHttpClient::with_cdn_servers(config, servers);
        assert!(client.is_ok());

        let client = client.expect("Operation should succeed");
        assert_eq!(client.cdn_servers().len(), 2);
        // Should be sorted by priority (HTTPS server has priority 100, HTTP has 200)
        assert_eq!(client.cdn_servers()[0].host, "level3.blizzard.com");
        assert_eq!(client.cdn_servers()[1].host, "cdn.arctium.tools");
    }

    #[test]
    fn test_client_path_caching() {
        let config = StreamingConfig::default();
        let mut client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        assert!(client.get_cached_path("wow").is_none());

        client.cache_cdn_path("wow".to_string(), "tpr/wow".to_string());
        assert_eq!(client.get_cached_path("wow"), Some("tpr/wow"));
    }

    #[test]
    fn test_client_server_management() {
        let config = StreamingConfig::default();
        let mut client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        assert_eq!(client.cdn_servers().len(), 0);

        client.add_cdn_server(CdnServer::https("example.com".to_string()));
        assert_eq!(client.cdn_servers().len(), 1);

        // Add lower priority server - should be sorted to front
        client.add_cdn_server(CdnServer::new("priority.com".to_string(), true, 50));
        assert_eq!(client.cdn_servers().len(), 2);
        assert_eq!(client.cdn_servers()[0].host, "priority.com");
        assert_eq!(client.cdn_servers()[1].host, "example.com");
    }

    #[test]
    fn test_http_range_header_formatting() {
        let range = HttpRange::new(0, 1023);
        assert_eq!(range.to_header_value(), "bytes=0-1023");

        let range = HttpRange::from_offset_length(1024, 2048);
        assert_eq!(range.to_header_value(), "bytes=1024-3071");
    }

    #[tokio::test]
    async fn test_mock_http_client() {
        let mut mock_client = MockTestHttpClient::new();

        mock_client
            .expect_get_content_length()
            .with(mockall::predicate::eq("http://example.com/test"))
            .times(1)
            .returning(|_| Ok(1024));

        let result = mock_client
            .get_content_length("http://example.com/test")
            .await;
        assert_eq!(result.expect("Operation should succeed"), 1024);
    }

    #[tokio::test]
    #[allow(clippy::panic)]
    async fn test_cdn_content_no_servers() {
        let config = StreamingConfig::default();
        let client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        let result = client
            .get_cdn_content(
                "wow",
                ContentType::Data,
                "1234567890abcdef1234567890abcdef",
                None,
                true,
            )
            .await;

        assert!(result.is_err());
        if let Err(StreamingError::Configuration { reason }) = result {
            assert!(reason.contains("No CDN servers configured"));
        } else {
            unreachable!("Expected Configuration error");
        }
    }

    #[tokio::test]
    #[allow(clippy::panic)]
    async fn test_product_config_no_servers() {
        let config = StreamingConfig::default();
        let client = ReqwestHttpClient::new(config).expect("Operation should succeed");

        let result = client
            .get_product_config("1234567890abcdef1234567890abcdef", None, true)
            .await;

        assert!(result.is_err());
        if let Err(StreamingError::Configuration { reason }) = result {
            assert!(reason.contains("No CDN servers configured"));
        } else {
            unreachable!("Expected Configuration error");
        }
    }
}

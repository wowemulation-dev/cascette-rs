//! CDN client for content delivery with dependency injection

pub mod range;

use futures::StreamExt;
use std::fmt;
use std::sync::Arc;

use crate::config::CdnConfig;
use crate::error::{ProtocolError, Result};
use crate::retry::RetryPolicy;
use crate::transport::HttpClient;

pub use range::{RangeDownloader, RangeError};

/// CDN endpoint configuration injected from external source
#[derive(Debug, Clone)]
pub struct CdnEndpoint {
    /// CDN server hostname (e.g., "level3.blizzard.com")
    pub host: String,
    /// Product-specific path (e.g., "tpr/wow")
    pub path: String,
    /// Optional product path for newer products
    pub product_path: Option<String>,
    /// URL scheme (defaults to "https")
    pub scheme: Option<String>,
}

/// Content type for different CDN paths
#[derive(Debug, Clone, Copy)]
pub enum ContentType {
    Config,
    Data,
    Patch,
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config => write!(f, "config"),
            Self::Data => write!(f, "data"),
            Self::Patch => write!(f, "patch"),
        }
    }
}

/// CDN client for downloading content with injected endpoint configuration
pub struct CdnClient {
    http_client: HttpClient,
    cache: Arc<crate::cache::ProtocolCache>,
    config: CdnConfig,
}

impl CdnClient {
    /// Create a new CDN client - configuration is injected, not discovered
    pub fn new(cache: Arc<crate::cache::ProtocolCache>, config: CdnConfig) -> Result<Self> {
        Ok(Self {
            http_client: HttpClient::new()?,
            cache,
            config,
        })
    }

    /// Build CDN URL from injected endpoint configuration
    fn build_url(endpoint: &CdnEndpoint, content_type: ContentType, key: &[u8]) -> String {
        let hex_key = hex::encode(key);

        // IMPORTANT: Always use path field for ALL game content (config, data, patch)
        // ProductPath is ONLY for Battle.net launcher product configuration files
        // See docs/cdn.md for details on Path vs ProductPath distinction
        let base_path = &endpoint.path;

        // Use endpoint scheme if specified, otherwise default to https
        let scheme = endpoint.scheme.as_deref().unwrap_or("https");

        format!(
            "{}://{}/{}/{}/{}/{}/{}",
            scheme,
            endpoint.host,
            base_path,
            content_type,
            &hex_key[..2],
            &hex_key[2..4],
            hex_key
        )
    }

    /// Download content using injected CDN endpoint
    pub async fn download(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
    ) -> Result<Vec<u8>> {
        let hex_key = hex::encode(key);

        // Use full CDN path structure for cache key to match actual CDN organization
        // This allows direct correlation between cache files and CDN URLs
        // Always use path field for ALL game content (config, data, patch)
        let cache_key = format!(
            "cdn/{}/{}/{}/{}/{}",
            endpoint.path,
            content_type,
            &hex_key[..2],
            &hex_key[2..4],
            hex_key
        );

        // Check cache first
        if let Some(cached) = self.cache.get_bytes(&cache_key)? {
            tracing::debug!("CDN cache hit for {}", hex_key);
            return Ok(cached);
        }

        // Build URL from injected configuration (no Ribbit dependency)
        let url = Self::build_url(endpoint, content_type, key);

        // Download with retry logic
        let data = self.download_with_retry(&url).await?;

        // Store in cache
        self.cache.store_bytes(&cache_key, &data)?;

        Ok(data)
    }

    /// Download with resume support using HTTP Range headers
    ///
    /// If `resume_from` is Some(offset), sends a Range header to resume from that byte offset.
    /// Returns only the new bytes starting from the offset (not the full file).
    ///
    /// # Arguments
    ///
    /// * `endpoint` - CDN endpoint configuration
    /// * `content_type` - Type of content (config, data, patch)
    /// * `key` - Content key bytes
    /// * `resume_from` - Optional byte offset to resume from
    ///
    /// # Returns
    ///
    /// * If `resume_from` is None, returns complete file
    /// * If `resume_from` is Some(offset), returns bytes from offset onwards
    /// * Falls back to full download if server doesn't support Range header
    ///
    /// # Errors
    ///
    /// Returns error if download fails or server returns non-success status
    pub async fn download_with_resume(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
        resume_from: Option<u64>,
    ) -> Result<Vec<u8>> {
        let url = Self::build_url(endpoint, content_type, key);

        // If no resume point, use regular download
        let Some(offset) = resume_from else {
            return self.download_with_retry(&url).await;
        };

        // Try to download with Range header
        let response = self
            .http_client
            .inner()
            .get(&url)
            .header("Range", format!("bytes={offset}-"))
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::PARTIAL_CONTENT => {
                // Server supports Range, return partial content
                Ok(response.bytes().await?.to_vec())
            }
            reqwest::StatusCode::OK => {
                // Server returned full file (doesn't support Range or offset is 0)
                // This is acceptable - caller will handle this case
                tracing::warn!(
                    "Server returned full file instead of partial content (Range header not supported)"
                );
                Ok(response.bytes().await?.to_vec())
            }
            reqwest::StatusCode::RANGE_NOT_SATISFIABLE => {
                // Offset is beyond file size - file is already complete
                tracing::debug!("Range not satisfiable - file may already be complete");
                Ok(Vec::new())
            }
            status if status.is_success() => {
                // Other 2xx status - return what we got
                Ok(response.bytes().await?.to_vec())
            }
            status => {
                // Non-success status
                Err(ProtocolError::HttpStatus(status))
            }
        }
    }

    /// Download with HTTP range requests using injected endpoint
    pub async fn download_range(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>> {
        let url = Self::build_url(endpoint, content_type, key);

        let response = self
            .http_client
            .inner()
            .get(&url)
            .header("Range", format!("bytes={}-{}", offset, offset + length - 1))
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::PARTIAL_CONTENT | reqwest::StatusCode::OK => {
                Ok(response.bytes().await?.to_vec())
            }
            _ => Err(ProtocolError::RangeNotSupported),
        }
    }

    /// Download with progress callback
    pub async fn download_with_progress<F>(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
        mut progress: F,
    ) -> Result<Vec<u8>>
    where
        F: FnMut(u64, u64) + Send,
    {
        let url = Self::build_url(endpoint, content_type, key);

        let response = self.http_client.inner().get(&url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        let mut downloaded = 0u64;
        let mut data = if self.config.enable_progress {
            Vec::with_capacity(total_size as usize)
        } else {
            Vec::new()
        };
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            data.extend_from_slice(&chunk);
            downloaded += chunk.len() as u64;
            if self.config.enable_progress {
                progress(downloaded, total_size);
            }
        }

        Ok(data)
    }

    /// Download archive index file (.index)
    /// Archive indices have a special URL pattern with .index suffix
    pub async fn download_archive_index(
        &self,
        endpoint: &CdnEndpoint,
        archive_key: &str,
    ) -> Result<Vec<u8>> {
        // Build cache key for index file
        // Always use path field for ALL game content
        let cache_key = format!(
            "cdn/{}/data/{}/{}/{}.index",
            endpoint.path,
            &archive_key[..2],
            &archive_key[2..4],
            archive_key
        );

        // Check cache first
        if let Some(cached) = self.cache.get_bytes(&cache_key)? {
            tracing::debug!("CDN cache hit for archive index {}", archive_key);
            return Ok(cached);
        }

        // Build URL with .index suffix
        // Always use path field for ALL game content
        let scheme = endpoint.scheme.as_deref().unwrap_or("https");
        let url = format!(
            "{}://{}/{}/data/{}/{}/{}.index",
            scheme,
            endpoint.host,
            endpoint.path,
            &archive_key[..2],
            &archive_key[2..4],
            archive_key
        );

        // Download with retry logic
        let data = self.download_with_retry(&url).await?;

        // Store in cache
        self.cache.store_bytes(&cache_key, &data)?;

        Ok(data)
    }

    /// Get the CDN configuration
    pub fn config(&self) -> &CdnConfig {
        &self.config
    }

    /// Get file size without downloading using HEAD request
    pub async fn get_file_size(
        &self,
        endpoint: &CdnEndpoint,
        content_type: ContentType,
        key: &[u8],
    ) -> Result<Option<u64>> {
        let url = Self::build_url(endpoint, content_type, key);

        let response = self.http_client.inner().head(&url).send().await?;

        if response.status().is_success() {
            // Note: response.content_length() returns Some(0) for HEAD requests
            // because no body is actually transferred. We need to parse the header manually.
            let content_length = response
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());

            Ok(content_length)
        } else if response.status().is_client_error() {
            // 404 means file doesn't exist, return None
            Ok(None)
        } else {
            Err(ProtocolError::HttpStatus(response.status()))
        }
    }

    /// Get archive index size using HEAD request
    pub async fn get_index_size(
        &self,
        endpoint: &CdnEndpoint,
        archive_key: &str,
    ) -> Result<Option<u64>> {
        let scheme = endpoint.scheme.as_deref().unwrap_or("https");
        let url = format!(
            "{}://{}/{}/data/{}/{}/{}.index",
            scheme,
            endpoint.host,
            endpoint.path,
            &archive_key[..2],
            &archive_key[2..4],
            archive_key
        );

        let response = self.http_client.inner().head(&url).send().await?;

        if response.status().is_success() {
            // Note: response.content_length() returns Some(0) for HEAD requests
            // because no body is actually transferred. We need to parse the header manually.
            let content_length = response
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());

            Ok(content_length)
        } else if response.status().is_client_error() {
            Ok(None)
        } else {
            Err(ProtocolError::HttpStatus(response.status()))
        }
    }

    async fn download_with_retry(&self, url: &str) -> Result<Vec<u8>> {
        let retry_policy = RetryPolicy::default();

        retry_policy
            .execute(|| async {
                let response = self.http_client.inner().get(url).send().await?;

                if response.status().is_success() {
                    Ok(response.bytes().await?.to_vec())
                } else if response.status().is_server_error() {
                    Err(ProtocolError::ServerError(response.status()))
                } else {
                    Err(ProtocolError::HttpStatus(response.status()))
                }
            })
            .await
    }

    /// Create CDN endpoint from BPSV query results
    /// This is a convenience method to help users build `CdnEndpoint` from Ribbit responses
    pub fn endpoint_from_bpsv_row(
        row: &cascette_formats::bpsv::BpsvRow,
        schema: &cascette_formats::bpsv::BpsvSchema,
    ) -> Result<CdnEndpoint> {
        let host = row
            .get_by_name("Hosts", schema)
            .and_then(|v| v.as_string())
            .ok_or_else(|| ProtocolError::Parse("Missing Hosts field".to_string()))?;

        let path = row
            .get_by_name("Path", schema)
            .and_then(|v| v.as_string())
            .ok_or_else(|| ProtocolError::Parse("Missing Path field".to_string()))?;

        // ProductPath is optional (newer products)
        let product_path = row
            .get_by_name("ProductPath", schema)
            .and_then(|v| v.as_string())
            .map(std::string::ToString::to_string);

        Ok(CdnEndpoint {
            host: host.to_string(),
            path: path.to_string(),
            product_path,
            scheme: None, // Defaults to https in production
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cache::ProtocolCache;
    use crate::config::CacheConfig;
    use cascette_formats::bpsv::{BpsvField, BpsvRow, BpsvSchema, BpsvType, BpsvValue};
    use std::sync::Arc;
    use tempfile::TempDir;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_cache() -> Arc<ProtocolCache> {
        let temp_dir = TempDir::new().expect("Operation should succeed");
        let config = CacheConfig {
            cache_dir: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };
        Arc::new(ProtocolCache::new(&config).expect("Operation should succeed"))
    }

    #[test]
    fn test_content_type_display() {
        assert_eq!(ContentType::Config.to_string(), "config");
        assert_eq!(ContentType::Data.to_string(), "data");
        assert_eq!(ContentType::Patch.to_string(), "patch");
    }

    #[test]
    fn test_cdn_endpoint_creation() {
        let endpoint = CdnEndpoint {
            host: "level3.blizzard.com".to_string(),
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: None,
        };

        assert_eq!(endpoint.host, "level3.blizzard.com");
        assert_eq!(endpoint.path, "tpr/wow");
        assert!(endpoint.product_path.is_none());
    }

    #[tokio::test]
    async fn test_cdn_client_creation() {
        let cache = create_test_cache();
        let config = CdnConfig::default();
        let client = CdnClient::new(cache, config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_url_old_format() {
        let endpoint = CdnEndpoint {
            host: "level3.blizzard.com".to_string(),
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let url = CdnClient::build_url(&endpoint, ContentType::Data, &key);

        assert_eq!(
            url,
            "http://level3.blizzard.com/tpr/wow/data/ab/cd/abcdef1234567890"
        );
    }

    #[test]
    fn test_build_url_ignores_product_path() {
        // ProductPath is ONLY for Battle.net launcher product configuration files
        // ALL game content (config, data, patch) uses path field
        let endpoint = CdnEndpoint {
            host: "level3.blizzard.com".to_string(),
            path: "tpr/wow".to_string(),
            product_path: Some("tpr/configs".to_string()), // Should be ignored
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let url = CdnClient::build_url(&endpoint, ContentType::Config, &key);

        // Should use path, not product_path
        assert_eq!(
            url,
            "http://level3.blizzard.com/tpr/wow/config/ab/cd/abcdef1234567890"
        );
    }

    #[tokio::test]
    #[allow(clippy::panic)] // Test assertions may panic
    async fn test_successful_download() {
        let mock_server = MockServer::start().await;
        let test_data = b"test file content";

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_data.to_vec()))
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        // Extract host without scheme for endpoint
        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client.download(&endpoint, ContentType::Data, &key).await;

        match &result {
            Ok(data) => assert_eq!(data, test_data),
            Err(e) => panic!("Download failed: {e:?}"),
        }
    }

    #[tokio::test]
    async fn test_download_with_caching() {
        let mock_server = MockServer::start().await;
        let test_data = b"test file content";

        // Expect only one request due to caching
        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_data.to_vec()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");

        // First download
        let result1 = client.download(&endpoint, ContentType::Data, &key).await;
        assert!(result1.is_ok());

        // Second download should use cache
        let result2 = client.download(&endpoint, ContentType::Data, &key).await;
        assert!(result2.is_ok());
        assert_eq!(
            result1.expect("Operation should succeed"),
            result2.expect("Operation should succeed")
        );
    }

    #[tokio::test]
    async fn test_download_range_request() {
        let mock_server = MockServer::start().await;
        let test_data = b"partial content";

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .and(header("Range", "bytes=100-199"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(test_data.to_vec()))
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client
            .download_range(&endpoint, ContentType::Data, &key, 100, 100)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.expect("Operation should succeed"), test_data);
    }

    #[tokio::test]
    async fn test_download_range_not_supported() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .and(header("Range", "bytes=100-199"))
            .respond_with(ResponseTemplate::new(416)) // Range Not Satisfiable
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client
            .download_range(&endpoint, ContentType::Data, &key, 100, 100)
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::RangeNotSupported
        ));
    }

    #[tokio::test]
    async fn test_download_with_resume_no_offset() {
        let mock_server = MockServer::start().await;
        let test_data = b"full file content";

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_data.to_vec()))
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client
            .download_with_resume(&endpoint, ContentType::Data, &key, None)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.expect("Operation should succeed"), test_data);
    }

    #[tokio::test]
    async fn test_download_with_resume_partial_content() {
        let mock_server = MockServer::start().await;
        let test_data = b"remaining content";

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .and(header("Range", "bytes=100-"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(test_data.to_vec()))
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client
            .download_with_resume(&endpoint, ContentType::Data, &key, Some(100))
            .await;

        assert!(result.is_ok());
        assert_eq!(result.expect("Operation should succeed"), test_data);
    }

    #[tokio::test]
    async fn test_download_with_resume_fallback_full_file() {
        let mock_server = MockServer::start().await;
        let test_data = b"full file content (server ignores Range)";

        // Server doesn't support Range, returns 200 with full file
        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_data.to_vec()))
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client
            .download_with_resume(&endpoint, ContentType::Data, &key, Some(100))
            .await;

        // Should succeed and return full file
        assert!(result.is_ok());
        assert_eq!(result.expect("Operation should succeed"), test_data);
    }

    #[tokio::test]
    async fn test_download_with_progress() {
        let mock_server = MockServer::start().await;
        let test_data = vec![b'x'; 1000]; // 1KB of data

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(test_data.clone())
                    .append_header("content-length", "1000"),
            )
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let config = CdnConfig {
            enable_progress: true,
            ..Default::default()
        };
        let client = CdnClient::new(cache, config).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");

        let mut progress_calls = 0;
        let result = client
            .download_with_progress(&endpoint, ContentType::Data, &key, |downloaded, total| {
                progress_calls += 1;
                assert!(downloaded <= total);
                assert_eq!(total, 1000);
            })
            .await;

        assert!(result.is_ok());
        assert!(progress_calls > 0);
        assert_eq!(result.expect("Operation should succeed"), test_data);
    }

    #[tokio::test]
    async fn test_download_server_error_retry() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mock_server = MockServer::start().await;
        let test_data = b"success after retry";

        // Use a stateful mock that fails first, then succeeds
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        Mock::given(method("GET"))
            .and(path("/tpr/wow/data/ab/cd/abcdef1234567890"))
            .respond_with(move |_req: &wiremock::Request| {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First request fails
                    ResponseTemplate::new(500)
                } else {
                    // Subsequent requests succeed
                    ResponseTemplate::new(200).set_body_bytes(test_data.to_vec())
                }
            })
            .expect(2..) // Expect at least 2 calls
            .mount(&mock_server)
            .await;

        let cache = create_test_cache();
        let client = CdnClient::new(cache, CdnConfig::default()).expect("Operation should succeed");

        let host = mock_server.uri().replace("http://", "");
        let endpoint = CdnEndpoint {
            host,
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("http".to_string()),
        };

        let key = hex::decode("abcdef1234567890").expect("Operation should succeed");
        let result = client.download(&endpoint, ContentType::Data, &key).await;

        assert!(result.is_ok(), "Download should succeed after retry");
        assert_eq!(result.expect("Operation should succeed"), test_data);

        // Verify that we made at least 2 requests (initial + retry)
        assert!(
            counter.load(Ordering::SeqCst) >= 2,
            "Should have made at least 2 requests"
        );
    }

    #[test]
    fn test_endpoint_from_bpsv_row() {
        let schema = BpsvSchema::new(vec![
            BpsvField::new("Name", BpsvType::String(0)),
            BpsvField::new("Hosts", BpsvType::String(0)),
            BpsvField::new("Path", BpsvType::String(0)),
            BpsvField::new("ProductPath", BpsvType::String(0)),
        ]);

        let row = BpsvRow::from_values(vec![
            BpsvValue::String("us".to_string()),
            BpsvValue::String("level3.blizzard.com".to_string()),
            BpsvValue::String("tpr/wow".to_string()),
            BpsvValue::String("wow/retail/us".to_string()),
        ]);

        let result = CdnClient::endpoint_from_bpsv_row(&row, &schema);
        assert!(result.is_ok());

        let endpoint = result.expect("Operation should succeed");
        assert_eq!(endpoint.host, "level3.blizzard.com");
        assert_eq!(endpoint.path, "tpr/wow");
        assert_eq!(endpoint.product_path, Some("wow/retail/us".to_string()));
    }

    #[test]
    fn test_endpoint_from_bpsv_row_missing_fields() {
        let schema = BpsvSchema::new(vec![BpsvField::new("Name", BpsvType::String(0))]);

        let row = BpsvRow::from_values(vec![BpsvValue::String("us".to_string())]);

        let result = CdnClient::endpoint_from_bpsv_row(&row, &schema);
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::Parse(_)
        ));
    }

    #[test]
    fn test_cdn_config_access() {
        let cache = create_test_cache();
        let config = CdnConfig {
            max_concurrent: 10,
            chunk_size: 8 * 1024 * 1024,
            enable_progress: true,
            pool_size: 50,
        };

        let client = CdnClient::new(cache, config).expect("Operation should succeed");
        let client_config = client.config();

        assert_eq!(client_config.max_concurrent, 10);
        assert_eq!(client_config.chunk_size, 8 * 1024 * 1024);
        assert!(client_config.enable_progress);
        assert_eq!(client_config.pool_size, 50);
    }
}

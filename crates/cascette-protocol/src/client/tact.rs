//! TACT HTTP/HTTPS protocol implementation

use cascette_formats::CascFormat;
use cascette_formats::bpsv::BpsvDocument;
use reqwest::{Client, StatusCode};
use std::time::Duration;

use crate::error::{ProtocolError, Result};

/// TACT HTTP/HTTPS client
pub struct TactClient {
    client: Client,
    base_url: String,
    // On WASM, timeout is stored for API compatibility but not enforced
    // (browser manages timeouts via Fetch API)
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    timeout: Duration,
}

impl TactClient {
    /// Create a new TACT client
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(base_url: String, _use_https: bool) -> Result<Self> {
        crate::transport::ensure_crypto_provider();
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            base_url,
            timeout: Duration::from_secs(30),
        })
    }

    /// Create a new TACT client (WASM version)
    ///
    /// On WASM, connection pooling and timeout settings are not supported
    /// as the browser manages these via the Fetch API.
    #[cfg(target_arch = "wasm32")]
    pub fn new(base_url: String, _use_https: bool) -> Result<Self> {
        let client = Client::builder().build()?;

        Ok(Self {
            client,
            base_url,
            timeout: Duration::from_secs(30), // Stored for API compatibility but not enforced
        })
    }

    /// Query TACT endpoint
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn query(&self, endpoint: &str) -> Result<BpsvDocument> {
        // Transform TCP Ribbit endpoint format to TACT format
        // TCP: v1/products/{product}/versions -> TACT: /{product}/versions
        let tact_endpoint = if endpoint.starts_with("v1/products/") {
            endpoint.strip_prefix("v1/products").unwrap_or(endpoint)
        } else {
            endpoint
        };

        // Ensure proper URL construction with slash
        let url = if tact_endpoint.starts_with('/') {
            format!("{}{}", self.base_url, tact_endpoint)
        } else {
            format!("{}/{}", self.base_url, tact_endpoint)
        };

        tracing::debug!("TACT request URL: {}", url);

        let response = self.client.get(&url).timeout(self.timeout).send().await?;

        // Check status
        match response.status() {
            StatusCode::OK => {
                let body = response.bytes().await?;
                <BpsvDocument as CascFormat>::parse(&body)
                    .map_err(|e| ProtocolError::Parse(format!("BPSV parse error: {e}")))
            }
            StatusCode::TOO_MANY_REQUESTS => Err(ProtocolError::RateLimited),
            StatusCode::SERVICE_UNAVAILABLE => Err(ProtocolError::ServiceUnavailable),
            status if status.is_server_error() => Err(ProtocolError::ServerError(status)),
            status => Err(ProtocolError::HttpStatus(status)),
        }
    }

    /// Query TACT endpoint (WASM version)
    ///
    /// On WASM, timeout is not supported on the request builder, so we
    /// rely on the browser's default timeout behavior.
    #[cfg(target_arch = "wasm32")]
    pub async fn query(&self, endpoint: &str) -> Result<BpsvDocument> {
        // Transform TCP Ribbit endpoint format to TACT format
        // TCP: v1/products/{product}/versions -> TACT: /{product}/versions
        let tact_endpoint = if endpoint.starts_with("v1/products/") {
            endpoint.strip_prefix("v1/products").unwrap_or(endpoint)
        } else {
            endpoint
        };

        // Ensure proper URL construction with slash
        let url = if tact_endpoint.starts_with('/') {
            format!("{}{}", self.base_url, tact_endpoint)
        } else {
            format!("{}/{}", self.base_url, tact_endpoint)
        };

        tracing::debug!("TACT request URL: {}", url);

        // On WASM, timeout() is not available on the request builder
        let response = self.client.get(&url).send().await?;

        // Check status
        match response.status() {
            StatusCode::OK => {
                let body = response.bytes().await?;
                <BpsvDocument as CascFormat>::parse(&body)
                    .map_err(|e| ProtocolError::Parse(format!("BPSV parse error: {e}")))
            }
            StatusCode::TOO_MANY_REQUESTS => Err(ProtocolError::RateLimited),
            StatusCode::SERVICE_UNAVAILABLE => Err(ProtocolError::ServiceUnavailable),
            status if status.is_server_error() => Err(ProtocolError::ServerError(status)),
            status => Err(ProtocolError::HttpStatus(status)),
        }
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::error::ProtocolError;
    use reqwest::StatusCode;
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_valid_bpsv() -> &'static str {
        "##seqn!DEC:4|region!STRING:0|buildconfig!HEX:16|cdnconfig!HEX:16|keyring!HEX:16|buildid!DEC:4|versionsname!STRING:0|productconfig!HEX:16\n1|us|abcd1234abcd1234|cdef5678cdef5678|def90123def90123|12345|1.0.0|fedcba09fedcba09\n"
    }

    #[tokio::test]
    async fn test_tact_client_creation() {
        let client = TactClient::new("https://example.com".to_string(), true);
        assert!(client.is_ok());

        let client = TactClient::new("http://example.com".to_string(), false);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_successful_query() {
        let mock_server = MockServer::start().await;
        let bpsv_data = create_valid_bpsv();

        Mock::given(method("GET"))
            .and(path("/wow/versions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(bpsv_data))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_ok());
        let doc = result.expect("Operation should succeed");
        assert!(!doc.rows().is_empty());
    }

    #[tokio::test]
    async fn test_query_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/nonexistent"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/nonexistent").await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::HttpStatus(StatusCode::NOT_FOUND)
        ));
    }

    #[tokio::test]
    async fn test_query_rate_limited() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/wow/versions"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::RateLimited
        ));
    }

    #[tokio::test]
    async fn test_query_service_unavailable() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/wow/versions"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::ServiceUnavailable
        ));
    }

    #[tokio::test]
    async fn test_query_server_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/wow/versions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::ServerError(StatusCode::INTERNAL_SERVER_ERROR)
        ));
    }

    #[tokio::test]
    async fn test_query_invalid_bpsv() {
        let mock_server = MockServer::start().await;
        let invalid_bpsv = "invalid bpsv data";

        Mock::given(method("GET"))
            .and(path("/wow/versions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(invalid_bpsv))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::Parse(_)
        ));
    }

    #[tokio::test]
    async fn test_url_construction() {
        let mock_server = MockServer::start().await;
        let bpsv_data = create_valid_bpsv();

        // TACT endpoints transform v1/products/ to direct product path
        Mock::given(method("GET"))
            .and(path("/wow/versions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(bpsv_data))
            .mount(&mock_server)
            .await;

        let client = TactClient::new(mock_server.uri(), true).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeout_configuration() {
        let client = TactClient::new("https://example.com".to_string(), true)
            .expect("Operation should succeed");
        assert_eq!(client.timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_https_vs_http() {
        // Test that both HTTPS and HTTP clients can be created
        let https_client = TactClient::new("https://example.com".to_string(), true);
        let plain_http_client = TactClient::new("http://example.com".to_string(), false);

        assert!(https_client.is_ok());
        assert!(plain_http_client.is_ok());
    }
}

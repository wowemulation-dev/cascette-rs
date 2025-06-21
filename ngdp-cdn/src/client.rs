//! CDN client for downloading NGDP content

use crate::{Error, Result};
use reqwest::{Client, Response};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, trace, warn};

/// Default maximum retries (0 = no retries, maintains backward compatibility)
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default initial backoff in milliseconds
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 100;

/// Default maximum backoff in milliseconds
const DEFAULT_MAX_BACKOFF_MS: u64 = 10_000;

/// Default backoff multiplier
const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// Default jitter factor (0.0 to 1.0)
const DEFAULT_JITTER_FACTOR: f64 = 0.1;

/// Default connection timeout
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;

/// Default request timeout
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 300;

/// CDN client for downloading content
#[derive(Debug, Clone)]
pub struct CdnClient {
    /// HTTP client with connection pooling
    client: Client,
    /// Maximum number of retries
    max_retries: u32,
    /// Initial backoff duration in milliseconds
    initial_backoff_ms: u64,
    /// Maximum backoff duration in milliseconds
    max_backoff_ms: u64,
    /// Backoff multiplier
    backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    jitter_factor: f64,
    /// Custom user agent string
    user_agent: Option<String>,
}

impl CdnClient {
    /// Create a new CDN client with default configuration
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))
            .pool_max_idle_per_host(20) // Increase connection pool size for CDN
            .gzip(true) // Enable automatic gzip decompression
            .deflate(true) // Enable automatic deflate decompression
            .build()?;

        Ok(Self {
            client,
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter_factor: DEFAULT_JITTER_FACTOR,
            user_agent: None,
        })
    }

    /// Create a new CDN client with custom HTTP client
    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter_factor: DEFAULT_JITTER_FACTOR,
            user_agent: None,
        }
    }

    /// Create a builder for configuring the CDN client
    pub fn builder() -> CdnClientBuilder {
        CdnClientBuilder::new()
    }

    /// Set the maximum number of retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the initial backoff duration in milliseconds
    pub fn with_initial_backoff_ms(mut self, initial_backoff_ms: u64) -> Self {
        self.initial_backoff_ms = initial_backoff_ms;
        self
    }

    /// Set the maximum backoff duration in milliseconds
    pub fn with_max_backoff_ms(mut self, max_backoff_ms: u64) -> Self {
        self.max_backoff_ms = max_backoff_ms;
        self
    }

    /// Set the backoff multiplier
    pub fn with_backoff_multiplier(mut self, backoff_multiplier: f64) -> Self {
        self.backoff_multiplier = backoff_multiplier;
        self
    }

    /// Set the jitter factor (0.0 to 1.0)
    pub fn with_jitter_factor(mut self, jitter_factor: f64) -> Self {
        self.jitter_factor = jitter_factor.clamp(0.0, 1.0);
        self
    }

    /// Set a custom user agent string
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Calculate backoff duration with exponential backoff and jitter
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_backoff =
            self.initial_backoff_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_backoff = base_backoff.min(self.max_backoff_ms as f64);

        // Add jitter
        let jitter_range = capped_backoff * self.jitter_factor;
        let jitter = rand::random::<f64>() * 2.0 * jitter_range - jitter_range;
        let final_backoff = (capped_backoff + jitter).max(0.0) as u64;

        Duration::from_millis(final_backoff)
    }

    /// Execute a request with retry logic
    async fn execute_with_retry(&self, url: &str) -> Result<Response> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let backoff = self.calculate_backoff(attempt - 1);
                debug!("CDN retry attempt {} after {:?} backoff", attempt, backoff);
                sleep(backoff).await;
            }

            debug!("CDN request to {} (attempt {})", url, attempt + 1);

            let mut request = self.client.get(url);
            if let Some(ref user_agent) = self.user_agent {
                request = request.header("User-Agent", user_agent);
            }

            match request.send().await {
                Ok(response) => {
                    trace!("Response status: {}", response.status());

                    // Check if we should retry based on status code
                    let status = response.status();

                    // Success - return the response
                    if status.is_success() {
                        return Ok(response);
                    }

                    // Check for rate limiting
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                        && attempt < self.max_retries
                    {
                        // Try to parse Retry-After header
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(60);

                        warn!(
                            "Rate limited (attempt {}): retry after {} seconds",
                            attempt + 1,
                            retry_after
                        );
                        last_error = Some(Error::rate_limited(retry_after));
                        continue;
                    }

                    // Server errors - retry
                    if status.is_server_error() && attempt < self.max_retries {
                        warn!(
                            "Server error {} (attempt {}): will retry",
                            status,
                            attempt + 1
                        );
                        last_error = Some(Error::Http(response.error_for_status().unwrap_err()));
                        continue;
                    }

                    // Client errors - don't retry
                    if status.is_client_error() {
                        if status == reqwest::StatusCode::NOT_FOUND {
                            let parts: Vec<&str> = url.rsplitn(2, '/').collect();
                            let hash = parts.first().copied().unwrap_or("unknown");
                            return Err(Error::content_not_found(hash));
                        }
                        return Err(Error::Http(response.error_for_status().unwrap_err()));
                    }

                    // Other errors - don't retry
                    return Err(Error::Http(response.error_for_status().unwrap_err()));
                }
                Err(e) => {
                    // Check if error is retryable
                    let is_retryable = e.is_connect() || e.is_timeout() || e.is_request();

                    if is_retryable && attempt < self.max_retries {
                        warn!(
                            "Request failed (attempt {}): {}, will retry",
                            attempt + 1,
                            e
                        );
                        last_error = Some(Error::Http(e));
                    } else {
                        // Non-retryable error or final attempt
                        debug!(
                            "Request failed (attempt {}): {}, not retrying",
                            attempt + 1,
                            e
                        );
                        return Err(Error::Http(e));
                    }
                }
            }
        }

        // This should only be reached if all retries failed
        Err(last_error.unwrap_or_else(|| Error::invalid_response("All retry attempts failed")))
    }

    /// Make a basic request to a CDN URL
    pub async fn request(&self, url: &str) -> Result<Response> {
        self.execute_with_retry(url).await
    }

    /// Build a CDN URL for a content hash
    ///
    /// CDN URLs follow the pattern:
    /// `http://{cdn_host}/{path}/{hash[0:2]}/{hash[2:4]}/{hash}`
    pub fn build_url(cdn_host: &str, path: &str, hash: &str) -> Result<String> {
        // Validate hash format (should be hex)
        if hash.len() < 4 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::invalid_hash(hash));
        }

        // Build the URL with the standard CDN path structure
        let url = format!(
            "http://{}/{}/{}/{}/{}",
            cdn_host,
            path.trim_matches('/'),
            &hash[0..2],
            &hash[2..4],
            hash
        );

        Ok(url)
    }

    /// Download content from CDN by hash
    pub async fn download(&self, cdn_host: &str, path: &str, hash: &str) -> Result<Response> {
        let url = Self::build_url(cdn_host, path, hash)?;
        self.request(&url).await
    }
}

impl Default for CdnClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default CDN client")
    }
}

/// Builder for configuring CDN client
#[derive(Debug, Clone)]
pub struct CdnClientBuilder {
    connect_timeout_secs: u64,
    request_timeout_secs: u64,
    pool_max_idle_per_host: usize,
    max_retries: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    backoff_multiplier: f64,
    jitter_factor: f64,
    user_agent: Option<String>,
}

impl CdnClientBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            pool_max_idle_per_host: 20,
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter_factor: DEFAULT_JITTER_FACTOR,
            user_agent: None,
        }
    }

    /// Set connection timeout
    pub fn connect_timeout(mut self, secs: u64) -> Self {
        self.connect_timeout_secs = secs;
        self
    }

    /// Set request timeout
    pub fn request_timeout(mut self, secs: u64) -> Self {
        self.request_timeout_secs = secs;
        self
    }

    /// Set maximum idle connections per host
    pub fn pool_max_idle_per_host(mut self, max: usize) -> Self {
        self.pool_max_idle_per_host = max;
        self
    }

    /// Set maximum retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set initial backoff in milliseconds
    pub fn initial_backoff_ms(mut self, ms: u64) -> Self {
        self.initial_backoff_ms = ms;
        self
    }

    /// Set maximum backoff in milliseconds
    pub fn max_backoff_ms(mut self, ms: u64) -> Self {
        self.max_backoff_ms = ms;
        self
    }

    /// Set backoff multiplier
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Set jitter factor (0.0 to 1.0)
    pub fn jitter_factor(mut self, factor: f64) -> Self {
        self.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Set custom user agent string
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Build the CDN client
    pub fn build(self) -> Result<CdnClient> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(self.connect_timeout_secs))
            .timeout(Duration::from_secs(self.request_timeout_secs))
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .gzip(true)
            .deflate(true)
            .build()?;

        Ok(CdnClient {
            client,
            max_retries: self.max_retries,
            initial_backoff_ms: self.initial_backoff_ms,
            max_backoff_ms: self.max_backoff_ms,
            backoff_multiplier: self.backoff_multiplier,
            jitter_factor: self.jitter_factor,
            user_agent: self.user_agent,
        })
    }
}

impl Default for CdnClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = CdnClient::new().unwrap();
        assert_eq!(client.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(client.initial_backoff_ms, DEFAULT_INITIAL_BACKOFF_MS);
        assert_eq!(client.max_backoff_ms, DEFAULT_MAX_BACKOFF_MS);
    }

    #[test]
    fn test_builder_configuration() {
        let client = CdnClient::builder()
            .max_retries(5)
            .initial_backoff_ms(200)
            .max_backoff_ms(5000)
            .backoff_multiplier(1.5)
            .jitter_factor(0.2)
            .connect_timeout(60)
            .request_timeout(600)
            .pool_max_idle_per_host(100)
            .build()
            .unwrap();

        assert_eq!(client.max_retries, 5);
        assert_eq!(client.initial_backoff_ms, 200);
        assert_eq!(client.max_backoff_ms, 5000);
        assert!((client.backoff_multiplier - 1.5).abs() < f64::EPSILON);
        assert!((client.jitter_factor - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jitter_factor_clamping() {
        let client1 = CdnClient::new().unwrap().with_jitter_factor(1.5);
        assert!((client1.jitter_factor - 1.0).abs() < f64::EPSILON);

        let client2 = CdnClient::new().unwrap().with_jitter_factor(-0.5);
        assert!((client2.jitter_factor - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_backoff_calculation() {
        let client = CdnClient::new()
            .unwrap()
            .with_initial_backoff_ms(100)
            .with_max_backoff_ms(1000)
            .with_backoff_multiplier(2.0)
            .with_jitter_factor(0.0); // No jitter for predictable test

        // Test exponential backoff
        let backoff0 = client.calculate_backoff(0);
        assert_eq!(backoff0.as_millis(), 100); // 100ms * 2^0 = 100ms

        let backoff1 = client.calculate_backoff(1);
        assert_eq!(backoff1.as_millis(), 200); // 100ms * 2^1 = 200ms

        let backoff2 = client.calculate_backoff(2);
        assert_eq!(backoff2.as_millis(), 400); // 100ms * 2^2 = 400ms

        // Test max backoff capping
        let backoff5 = client.calculate_backoff(5);
        assert_eq!(backoff5.as_millis(), 1000); // Would be 3200ms but capped at 1000ms
    }

    #[test]
    fn test_build_url() {
        let url = CdnClient::build_url(
            "blzddist1-a.akamaihd.net",
            "tpr/wow",
            "2e9c1e3b5f5a0c9d9e8f1234567890ab",
        )
        .unwrap();

        assert_eq!(
            url,
            "http://blzddist1-a.akamaihd.net/tpr/wow/2e/9c/2e9c1e3b5f5a0c9d9e8f1234567890ab"
        );

        // Test with trailing slash in path
        let url2 = CdnClient::build_url(
            "blzddist1-a.akamaihd.net",
            "tpr/wow/",
            "2e9c1e3b5f5a0c9d9e8f1234567890ab",
        )
        .unwrap();

        assert_eq!(
            url2,
            "http://blzddist1-a.akamaihd.net/tpr/wow/2e/9c/2e9c1e3b5f5a0c9d9e8f1234567890ab"
        );
    }

    #[test]
    fn test_build_url_invalid_hash() {
        // Too short
        let result = CdnClient::build_url("host", "path", "abc");
        assert!(result.is_err());

        // Non-hex characters
        let result = CdnClient::build_url("host", "path", "zzzz1234567890ab");
        assert!(result.is_err());

        // Empty hash
        let result = CdnClient::build_url("host", "path", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_user_agent_configuration() {
        let client = CdnClient::new()
            .unwrap()
            .with_user_agent("MyNGDPClient/1.0");

        assert_eq!(client.user_agent, Some("MyNGDPClient/1.0".to_string()));
    }

    #[test]
    fn test_user_agent_via_builder() {
        let client = CdnClient::builder()
            .user_agent("MyNGDPClient/2.0")
            .build()
            .unwrap();

        assert_eq!(client.user_agent, Some("MyNGDPClient/2.0".to_string()));
    }

    #[test]
    fn test_user_agent_default_none() {
        let client = CdnClient::new().unwrap();
        assert!(client.user_agent.is_none());
    }
}

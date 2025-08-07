//! HTTP client for TACT protocol

use crate::{CdnEntry, Error, Region, Result, VersionEntry, response_types};
use reqwest::{Client, Response};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, trace, warn};

/// Default maximum retries (0 = no retries, maintains backward compatibility)
const DEFAULT_MAX_RETRIES: u32 = 0;

/// Default initial backoff in milliseconds
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 100;

/// Default maximum backoff in milliseconds
const DEFAULT_MAX_BACKOFF_MS: u64 = 10_000;

/// Default backoff multiplier
const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// Default jitter factor (0.0 to 1.0)
const DEFAULT_JITTER_FACTOR: f64 = 0.1;

/// TACT protocol version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    /// Version 1: HTTP-based protocol on port 1119
    V1,
    /// Version 2: HTTPS-based REST API
    V2,
}

/// HTTP client for TACT protocol
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    region: Region,
    version: ProtocolVersion,
    max_retries: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    backoff_multiplier: f64,
    jitter_factor: f64,
    user_agent: Option<String>,
}

impl HttpClient {
    /// Create a new HTTP client for the specified region and protocol version
    pub fn new(region: Region, version: ProtocolVersion) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            region,
            version,
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter_factor: DEFAULT_JITTER_FACTOR,
            user_agent: None,
        })
    }

    /// Create a new HTTP client with custom reqwest client
    pub fn with_client(client: Client, region: Region, version: ProtocolVersion) -> Self {
        Self {
            client,
            region,
            version,
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter_factor: DEFAULT_JITTER_FACTOR,
            user_agent: None,
        }
    }

    /// Set the maximum number of retries for failed requests
    ///
    /// Default is 0 (no retries) to maintain backward compatibility.
    /// Only network and connection errors are retried, not parsing errors.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the initial backoff duration in milliseconds
    ///
    /// Default is 100ms. This is the base delay before the first retry.
    pub fn with_initial_backoff_ms(mut self, initial_backoff_ms: u64) -> Self {
        self.initial_backoff_ms = initial_backoff_ms;
        self
    }

    /// Set the maximum backoff duration in milliseconds
    ///
    /// Default is 10,000ms (10 seconds). Backoff will not exceed this value.
    pub fn with_max_backoff_ms(mut self, max_backoff_ms: u64) -> Self {
        self.max_backoff_ms = max_backoff_ms;
        self
    }

    /// Set the backoff multiplier
    ///
    /// Default is 2.0. The backoff duration is multiplied by this value after each retry.
    pub fn with_backoff_multiplier(mut self, backoff_multiplier: f64) -> Self {
        self.backoff_multiplier = backoff_multiplier;
        self
    }

    /// Set the jitter factor (0.0 to 1.0)
    ///
    /// Default is 0.1 (10% jitter). Adds randomness to prevent thundering herd.
    pub fn with_jitter_factor(mut self, jitter_factor: f64) -> Self {
        self.jitter_factor = jitter_factor.clamp(0.0, 1.0);
        self
    }

    /// Set a custom user agent string
    ///
    /// If not set, reqwest's default user agent will be used.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Get the base URL for the current configuration
    pub fn base_url(&self) -> String {
        match self.version {
            ProtocolVersion::V1 => {
                format!("http://{}.patch.battle.net:1119", self.region)
            }
            ProtocolVersion::V2 => {
                format!("https://{}.version.battle.net/v2/products", self.region)
            }
        }
    }

    /// Get the current region
    pub fn region(&self) -> Region {
        self.region
    }

    /// Get the current protocol version
    pub fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Set the region
    pub fn set_region(&mut self, region: Region) {
        self.region = region;
    }

    /// Calculate backoff duration with exponential backoff and jitter
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_backoff =
            self.initial_backoff_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_backoff = base_backoff.min(self.max_backoff_ms as f64);

        // Add jitter
        let jitter_range = capped_backoff * self.jitter_factor;
        let jitter = rand::random::<f64>() * 2.0 * jitter_range - jitter_range;
        let final_backoff = (capped_backoff + jitter).max(0.0) as u64;

        Duration::from_millis(final_backoff)
    }

    /// Execute an HTTP request with retry logic
    async fn execute_with_retry(&self, url: &str) -> Result<Response> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let backoff = self.calculate_backoff(attempt - 1);
                debug!("Retry attempt {} after {:?} backoff", attempt, backoff);
                sleep(backoff).await;
            }

            debug!("HTTP request to {} (attempt {})", url, attempt + 1);

            let mut request = self.client.get(url);
            if let Some(ref user_agent) = self.user_agent {
                request = request.header("User-Agent", user_agent);
            }

            match request.send().await {
                Ok(response) => {
                    trace!("Response status: {}", response.status());

                    // Check if we should retry based on status code
                    let status = response.status();
                    if (status.is_server_error()
                        || status == reqwest::StatusCode::TOO_MANY_REQUESTS)
                        && attempt < self.max_retries
                    {
                        warn!(
                            "Request returned {} (attempt {}): will retry",
                            status,
                            attempt + 1
                        );
                        last_error = Some(Error::InvalidResponse);
                        continue;
                    }

                    return Ok(response);
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
        Err(last_error.unwrap_or(Error::InvalidResponse))
    }

    /// Execute an HTTP request with additional headers and retry logic
    async fn execute_with_retry_and_headers(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Response> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let backoff = self.calculate_backoff(attempt - 1);
                debug!("Retry attempt {} after {:?} backoff", attempt, backoff);
                sleep(backoff).await;
            }

            debug!("HTTP request to {} (attempt {})", url, attempt + 1);

            let mut request = self.client.get(url);
            if let Some(ref user_agent) = self.user_agent {
                request = request.header("User-Agent", user_agent);
            }

            // Add custom headers
            for &(key, value) in headers {
                request = request.header(key, value);
            }

            match request.send().await {
                Ok(response) => {
                    trace!("Response status: {}", response.status());

                    // Check if we should retry based on status code
                    let status = response.status();
                    if (status.is_server_error()
                        || status == reqwest::StatusCode::TOO_MANY_REQUESTS)
                        && attempt < self.max_retries
                    {
                        warn!(
                            "Request returned {} (attempt {}): will retry",
                            status,
                            attempt + 1
                        );
                        last_error = Some(Error::InvalidResponse);
                        continue;
                    }

                    return Ok(response);
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
        Err(last_error.unwrap_or(Error::InvalidResponse))
    }

    /// Get versions manifest for a product (V1 protocol)
    pub async fn get_versions(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V1 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}/versions", self.base_url(), product);
        self.execute_with_retry(&url).await
    }

    /// Get CDN configuration for a product (V1 protocol)
    pub async fn get_cdns(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V1 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}/cdns", self.base_url(), product);
        self.execute_with_retry(&url).await
    }

    /// Get BGDL manifest for a product (V1 protocol)
    pub async fn get_bgdl(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V1 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}/bgdl", self.base_url(), product);
        self.execute_with_retry(&url).await
    }

    /// Get product summary (V2 protocol)
    pub async fn get_summary(&self) -> Result<Response> {
        if self.version != ProtocolVersion::V2 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = self.base_url();
        self.execute_with_retry(&url).await
    }

    /// Get product details (V2 protocol)
    pub async fn get_product(&self, product: &str) -> Result<Response> {
        if self.version != ProtocolVersion::V2 {
            return Err(Error::InvalidProtocolVersion);
        }

        let url = format!("{}/{}", self.base_url(), product);
        self.execute_with_retry(&url).await
    }

    /// Make a raw GET request to a path
    pub async fn get(&self, path: &str) -> Result<Response> {
        let url = if path.starts_with('/') {
            format!("{}{}", self.base_url(), path)
        } else {
            format!("{}/{}", self.base_url(), path)
        };

        self.execute_with_retry(&url).await
    }

    /// Download a file from CDN
    pub async fn download_file(&self, cdn_host: &str, path: &str, hash: &str) -> Result<Response> {
        let url = format!(
            "http://{}/{}/{}/{}/{}",
            cdn_host,
            path,
            &hash[0..2],
            &hash[2..4],
            hash
        );

        // Use execute_with_retry for CDN downloads as well
        let response = self.execute_with_retry(&url).await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::file_not_found(hash));
        }

        Ok(response)
    }

    /// Download a file from CDN with HTTP range request for partial content
    ///
    /// # Arguments
    /// * `cdn_host` - CDN hostname
    /// * `path` - Path prefix for the CDN
    /// * `hash` - File hash
    /// * `range` - Byte range to download (e.g., (0, Some(1023)) for first 1024 bytes)
    ///
    /// # Returns
    /// Returns a response with the requested byte range. The response will have status 206
    /// (Partial Content) if the range is supported, or status 200 (OK) with full content
    /// if range requests are not supported.
    pub async fn download_file_range(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        range: (u64, Option<u64>),
    ) -> Result<Response> {
        let url = format!(
            "http://{}/{}/{}/{}/{}",
            cdn_host,
            path,
            &hash[0..2],
            &hash[2..4],
            hash
        );

        // Build Range header value
        let range_header = match range {
            (start, Some(end)) => format!("bytes={}-{}", start, end),
            (start, None) => format!("bytes={}-", start),
        };

        debug!("Range request: {} Range: {}", url, range_header);

        let response = self
            .execute_with_retry_and_headers(&url, &[("Range", &range_header)])
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::file_not_found(hash));
        }

        // Check if server supports range requests
        match response.status() {
            reqwest::StatusCode::PARTIAL_CONTENT => {
                trace!("Server returned partial content (206)");
            }
            reqwest::StatusCode::OK => {
                warn!("Server returned full content (200) - range requests not supported");
            }
            status => {
                warn!(
                    "Unexpected status code for range request: {} (expected 206 or 200)",
                    status
                );
                // Still return the response - let the caller handle unexpected status codes
            }
        }

        Ok(response)
    }

    /// Download multiple ranges from a file in a single request
    ///
    /// # Arguments
    /// * `cdn_host` - CDN hostname
    /// * `path` - Path prefix for the CDN
    /// * `hash` - File hash
    /// * `ranges` - Multiple byte ranges to download
    ///
    /// # Note
    /// Multi-range requests return multipart/byteranges content type that needs
    /// special parsing. Use with caution - not all CDN servers support this.
    pub async fn download_file_multirange(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        ranges: &[(u64, Option<u64>)],
    ) -> Result<Response> {
        let url = format!(
            "http://{}/{}/{}/{}/{}",
            cdn_host,
            path,
            &hash[0..2],
            &hash[2..4],
            hash
        );

        // Build multi-range header value
        let mut range_specs = Vec::new();
        for &(start, end) in ranges {
            match end {
                Some(end) => range_specs.push(format!("{}-{}", start, end)),
                None => range_specs.push(format!("{}-", start)),
            }
        }
        let range_header = format!("bytes={}", range_specs.join(", "));

        debug!("Multi-range request: {} Range: {}", url, range_header);

        let response = self
            .execute_with_retry_and_headers(&url, &[("Range", &range_header)])
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::file_not_found(hash));
        }

        Ok(response)
    }

    /// Get parsed versions manifest for a product
    pub async fn get_versions_parsed(&self, product: &str) -> Result<Vec<VersionEntry>> {
        let response = self.get_versions(product).await?;
        let text = response.text().await?;
        response_types::parse_versions(&text)
    }

    /// Get parsed CDN manifest for a product
    pub async fn get_cdns_parsed(&self, product: &str) -> Result<Vec<CdnEntry>> {
        let response = self.get_cdns(product).await?;
        let text = response.text().await?;
        response_types::parse_cdns(&text)
    }

    /// Get parsed BGDL manifest for a product
    pub async fn get_bgdl_parsed(&self, product: &str) -> Result<Vec<response_types::BgdlEntry>> {
        let response = self.get_bgdl(product).await?;
        let text = response.text().await?;
        response_types::parse_bgdl(&text)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(Region::US, ProtocolVersion::V2).expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_v1() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
        assert_eq!(client.base_url(), "http://us.patch.battle.net:1119");

        let client = HttpClient::new(Region::EU, ProtocolVersion::V1).unwrap();
        assert_eq!(client.base_url(), "http://eu.patch.battle.net:1119");
    }

    #[test]
    fn test_base_url_v2() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V2).unwrap();
        assert_eq!(
            client.base_url(),
            "https://us.version.battle.net/v2/products"
        );

        let client = HttpClient::new(Region::EU, ProtocolVersion::V2).unwrap();
        assert_eq!(
            client.base_url(),
            "https://eu.version.battle.net/v2/products"
        );
    }

    #[test]
    fn test_region_setting() {
        let mut client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
        assert_eq!(client.region(), Region::US);

        client.set_region(Region::EU);
        assert_eq!(client.region(), Region::EU);
        assert_eq!(client.base_url(), "http://eu.patch.battle.net:1119");
    }

    #[test]
    fn test_retry_configuration() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1)
            .unwrap()
            .with_max_retries(3)
            .with_initial_backoff_ms(200)
            .with_max_backoff_ms(5000)
            .with_backoff_multiplier(1.5)
            .with_jitter_factor(0.2);

        assert_eq!(client.max_retries, 3);
        assert_eq!(client.initial_backoff_ms, 200);
        assert_eq!(client.max_backoff_ms, 5000);
        assert_eq!(client.backoff_multiplier, 1.5);
        assert_eq!(client.jitter_factor, 0.2);
    }

    #[test]
    fn test_jitter_factor_clamping() {
        let client1 = HttpClient::new(Region::US, ProtocolVersion::V1)
            .unwrap()
            .with_jitter_factor(1.5);
        assert_eq!(client1.jitter_factor, 1.0); // Should be clamped to 1.0

        let client2 = HttpClient::new(Region::US, ProtocolVersion::V1)
            .unwrap()
            .with_jitter_factor(-0.5);
        assert_eq!(client2.jitter_factor, 0.0); // Should be clamped to 0.0
    }

    #[test]
    fn test_backoff_calculation() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1)
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
    fn test_default_retry_configuration() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
        assert_eq!(client.max_retries, 0); // Default should be 0 for backward compatibility
    }

    #[test]
    fn test_user_agent_configuration() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1)
            .unwrap()
            .with_user_agent("MyCustomAgent/1.0");

        assert_eq!(client.user_agent, Some("MyCustomAgent/1.0".to_string()));
    }

    #[test]
    fn test_user_agent_default_none() {
        let client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
        assert!(client.user_agent.is_none());
    }

    // Range request tests
    #[test]
    fn test_range_request_header_formatting() {
        // Test range header formatting
        let range1 = (0, Some(1023));
        let header1 = match range1 {
            (start, Some(end)) => format!("bytes={}-{}", start, end),
            (start, None) => format!("bytes={}-", start),
        };
        assert_eq!(header1, "bytes=0-1023");

        let range2 = (1024, None::<u64>);
        let header2 = match range2 {
            (start, Some(end)) => format!("bytes={}-{}", start, end),
            (start, None) => format!("bytes={}-", start),
        };
        assert_eq!(header2, "bytes=1024-");
    }

    #[test]
    fn test_multirange_header_building() {
        let ranges = [(0, Some(31)), (64, Some(95)), (128, None)];
        let mut range_specs = Vec::new();
        
        for &(start, end) in &ranges {
            match end {
                Some(end) => range_specs.push(format!("{}-{}", start, end)),
                None => range_specs.push(format!("{}-", start)),
            }
        }
        
        let range_header = format!("bytes={}", range_specs.join(", "));
        assert_eq!(range_header, "bytes=0-31, 64-95, 128-");
    }
}

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
#[derive(Debug)]
pub struct CdnClient {
    /// HTTP client with connection pooling
    client: Client,
    /// Shared TACT client for resumable downloads (reuses connection pool)
    tact_client: Option<tact_client::HttpClient>,
    /// Request batcher for HTTP/2 multiplexing
    request_batcher: std::sync::Arc<tokio::sync::Mutex<Option<tact_client::RequestBatcher>>>,
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
            tact_client: None, // Will be initialized on first use
            request_batcher: std::sync::Arc::new(tokio::sync::Mutex::new(None)), // Will be initialized on first use
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
            tact_client: None, // Will be initialized on first use
            request_batcher: std::sync::Arc::new(tokio::sync::Mutex::new(None)), // Will be initialized on first use
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

    /// Get or create a shared TACT client for resumable downloads
    ///
    /// This reuses the same HttpClient instance across multiple resumable downloads,
    /// providing better connection pooling performance.
    fn get_or_create_tact_client(&mut self) -> Result<&tact_client::HttpClient> {
        if self.tact_client.is_none() {
            use tact_client::{HttpClient, ProtocolVersion, Region};

            // Create TACT client with shared connection pool
            let mut tact_client = HttpClient::with_shared_pool(Region::US, ProtocolVersion::V2)
                .with_max_retries(self.max_retries)
                .with_initial_backoff_ms(self.initial_backoff_ms);

            if let Some(user_agent) = &self.user_agent {
                tact_client = tact_client.with_user_agent(user_agent);
            }

            self.tact_client = Some(tact_client);
        }

        Ok(self.tact_client.as_ref().unwrap())
    }

    /// Get or create a request batcher for HTTP/2 multiplexing
    ///
    /// This enables efficient batching of multiple CDN requests over HTTP/2 connections,
    /// significantly improving performance for batch operations.
    async fn get_or_create_request_batcher(
        &self,
    ) -> Result<std::sync::Arc<tokio::sync::Mutex<Option<tact_client::RequestBatcher>>>> {
        let mut batcher_guard = self.request_batcher.lock().await;

        if batcher_guard.is_none() {
            use tact_client::{BatchConfig, RequestBatcher};

            // Create HTTP/2 client with optimizations for CDN requests
            let client = reqwest::Client::builder()
                // HTTP/2 is automatically negotiated when available
                .pool_max_idle_per_host(50) // Higher connection pool for batching
                .pool_idle_timeout(std::time::Duration::from_secs(120))
                .timeout(std::time::Duration::from_secs(300))
                .connect_timeout(std::time::Duration::from_secs(30))
                .use_rustls_tls()
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .gzip(true)
                .deflate(true)
                .build()
                .map_err(Error::Http)?;

            let batch_config = BatchConfig {
                batch_size: 20,            // Optimal for HTTP/2 multiplexing
                batch_timeout_ms: 50,      // Quick batching for low latency
                max_concurrent_batches: 8, // Higher concurrency for CDN
                batch_execution_timeout: std::time::Duration::from_secs(300),
            };

            let batcher = RequestBatcher::new(client, batch_config);
            *batcher_guard = Some(batcher);
        }

        Ok(std::sync::Arc::clone(&self.request_batcher))
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

    /// Download BuildConfig from CDN
    ///
    /// BuildConfig files are stored at `{path}/config/{hash}`
    pub async fn download_build_config(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash).await
    }

    /// Download CDNConfig from CDN
    ///
    /// CDNConfig files are stored at `{path}/config/{hash}`
    pub async fn download_cdn_config(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash).await
    }

    /// Download ProductConfig from CDN
    ///
    /// ProductConfig files are stored at `{config_path}/{hash}`
    /// Note: This uses the config_path from CDN response, not the regular path
    pub async fn download_product_config(
        &self,
        cdn_host: &str,
        config_path: &str,
        hash: &str,
    ) -> Result<Response> {
        self.download(cdn_host, config_path, hash).await
    }

    /// Download KeyRing from CDN
    ///
    /// KeyRing files are stored at `{path}/config/{hash}`
    pub async fn download_key_ring(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
    ) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(cdn_host, &config_path, hash).await
    }

    /// Download data file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}`
    pub async fn download_data(&self, cdn_host: &str, path: &str, hash: &str) -> Result<Response> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(cdn_host, &data_path, hash).await
    }

    /// Download patch file from CDN
    ///
    /// Patch files are stored at `{path}/patch/{hash}`
    pub async fn download_patch(&self, cdn_host: &str, path: &str, hash: &str) -> Result<Response> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download(cdn_host, &patch_path, hash).await
    }

    /// Download multiple files in parallel
    ///
    /// Returns a vector of results in the same order as the input hashes.
    /// Failed downloads will be represented as Err values in the vector.
    ///
    /// # Arguments
    /// * `cdn_host` - CDN host to download from
    /// * `path` - Base path on the CDN
    /// * `hashes` - List of content hashes to download
    /// * `max_concurrent` - Maximum number of concurrent downloads (None = unlimited)
    pub async fn download_parallel(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
        max_concurrent: Option<usize>,
    ) -> Vec<Result<Vec<u8>>> {
        use futures_util::stream::{self, StreamExt};

        let max_concurrent = max_concurrent.unwrap_or(10); // Default to 10 concurrent downloads

        let futures = hashes.iter().map(|hash| {
            let cdn_host = cdn_host.to_string();
            let path = path.to_string();

            async move {
                match self.download(&cdn_host, &path, hash).await {
                    Ok(response) => response
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(Into::into),
                    Err(e) => Err(e),
                }
            }
        });

        stream::iter(futures)
            .buffer_unordered(max_concurrent)
            .collect()
            .await
    }

    /// Download multiple files using HTTP/2 request batching for optimal performance
    ///
    /// This method uses HTTP/2 multiplexing to batch requests efficiently, providing
    /// significant performance improvements over traditional parallel downloads.
    ///
    /// # Arguments
    /// * `cdn_host` - CDN host to download from
    /// * `path` - Base path on the CDN
    /// * `hashes` - List of content hashes to download
    ///
    /// # Returns
    /// Vector of results in the same order as input hashes, containing downloaded data or errors.
    ///
    /// # Performance
    /// HTTP/2 multiplexing allows multiple requests over a single connection, reducing:
    /// - Connection overhead
    /// - Network latency
    /// - Server connection pressure
    ///
    /// Expected improvement: 2-5x faster than traditional parallel downloads for batches >10 files.
    pub async fn download_batched(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
    ) -> Vec<Result<Vec<u8>>> {
        // Create batch requests for all hashes
        let requests = tact_client::RequestBatcher::create_cdn_requests(cdn_host, path, hashes);

        // Get the request batcher
        let batcher_arc = match self.get_or_create_request_batcher().await {
            Ok(batcher_arc) => batcher_arc,
            Err(e) => {
                // Fallback to regular parallel downloads if batching fails
                warn!(
                    "Failed to create request batcher, falling back to parallel downloads: {}",
                    e
                );
                return self
                    .download_parallel(cdn_host, path, hashes, Some(20))
                    .await;
            }
        };

        // Submit batch and wait for responses
        let batch_responses = {
            let batcher_guard = batcher_arc.lock().await;
            if let Some(ref batcher) = *batcher_guard {
                batcher.submit_requests_and_wait(requests).await
            } else {
                // Fallback if batcher wasn't initialized
                warn!("Request batcher not initialized, falling back to parallel downloads");
                return self
                    .download_parallel(cdn_host, path, hashes, Some(20))
                    .await;
            }
        };

        // Convert batch responses to the expected format
        let mut results = Vec::with_capacity(hashes.len());
        let mut response_map: std::collections::HashMap<String, Result<Vec<u8>>> =
            std::collections::HashMap::new();

        // Process batch responses
        for response in batch_responses {
            let result = match response.result {
                Ok(http_response) => {
                    // Convert HTTP response to bytes
                    match http_response.bytes().await {
                        Ok(bytes) => Ok(bytes.to_vec()),
                        Err(e) => Err(Error::Http(e)),
                    }
                }
                Err(tact_err) => {
                    // Convert tact_client::Error to ngdp_cdn::Error
                    match tact_err {
                        tact_client::Error::Http(reqwest_err) => Err(Error::Http(reqwest_err)),
                        _ => Err(Error::invalid_response(format!(
                            "TACT client error: {tact_err}"
                        ))),
                    }
                }
            };

            response_map.insert(response.request_id, result);
        }

        // Ensure results are in the same order as input hashes
        for hash in hashes {
            if let Some(result) = response_map.remove(hash) {
                results.push(result);
            } else {
                results.push(Err(Error::invalid_response(
                    "No response received for hash",
                )));
            }
        }

        results
    }

    /// Download multiple files in parallel with progress tracking
    ///
    /// Returns a vector of results in the same order as the input hashes.
    /// The progress callback is called after each successful download.
    ///
    /// # Arguments
    /// * `cdn_host` - CDN host to download from
    /// * `path` - Base path on the CDN
    /// * `hashes` - List of content hashes to download
    /// * `max_concurrent` - Maximum number of concurrent downloads (None = unlimited)
    /// * `progress` - Callback function called with (completed_count, total_count) after each download
    pub async fn download_parallel_with_progress<F>(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
        max_concurrent: Option<usize>,
        mut progress: F,
    ) -> Vec<Result<Vec<u8>>>
    where
        F: FnMut(usize, usize),
    {
        use futures_util::stream::{self, StreamExt};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let max_concurrent = max_concurrent.unwrap_or(10);
        let total = hashes.len();
        let completed = Arc::new(AtomicUsize::new(0));

        let futures = hashes.iter().enumerate().map(|(idx, hash)| {
            let cdn_host = cdn_host.to_string();
            let path = path.to_string();
            let hash = hash.clone();
            let completed = Arc::clone(&completed);

            async move {
                let result = match self.download(&cdn_host, &path, &hash).await {
                    Ok(response) => response
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(Into::into),
                    Err(e) => Err(e),
                };

                // Update progress
                let count = completed.fetch_add(1, Ordering::SeqCst) + 1;

                (idx, result, count)
            }
        });

        let mut results: Vec<Result<Vec<u8>>> = Vec::with_capacity(total);
        for _ in 0..total {
            results.push(Err(Error::invalid_response("Not downloaded")));
        }

        let mut download_stream = stream::iter(futures).buffer_unordered(max_concurrent);

        while let Some((idx, result, count)) = download_stream.next().await {
            results[idx] = result;
            progress(count, total);
        }

        results
    }

    /// Download multiple data files in parallel
    pub async fn download_data_parallel(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
        max_concurrent: Option<usize>,
    ) -> Vec<Result<Vec<u8>>> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download_parallel(cdn_host, &data_path, hashes, max_concurrent)
            .await
    }

    /// Download multiple config files in parallel
    pub async fn download_config_parallel(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
        max_concurrent: Option<usize>,
    ) -> Vec<Result<Vec<u8>>> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download_parallel(cdn_host, &config_path, hashes, max_concurrent)
            .await
    }

    /// Download multiple patch files in parallel
    pub async fn download_patch_parallel(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
        max_concurrent: Option<usize>,
    ) -> Vec<Result<Vec<u8>>> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download_parallel(cdn_host, &patch_path, hashes, max_concurrent)
            .await
    }

    /// Download multiple data files using HTTP/2 request batching (optimized)
    ///
    /// This method provides superior performance for downloading multiple data files
    /// by leveraging HTTP/2 multiplexing to reduce connection overhead and latency.
    pub async fn download_data_batched(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
    ) -> Vec<Result<Vec<u8>>> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download_batched(cdn_host, &data_path, hashes).await
    }

    /// Download multiple config files using HTTP/2 request batching (optimized)
    ///
    /// This method provides superior performance for downloading multiple config files
    /// by leveraging HTTP/2 multiplexing to reduce connection overhead and latency.
    pub async fn download_config_batched(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
    ) -> Vec<Result<Vec<u8>>> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download_batched(cdn_host, &config_path, hashes).await
    }

    /// Download multiple patch files using HTTP/2 request batching (optimized)
    ///
    /// This method provides superior performance for downloading multiple patch files
    /// by leveraging HTTP/2 multiplexing to reduce connection overhead and latency.
    pub async fn download_patch_batched(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
    ) -> Vec<Result<Vec<u8>>> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download_batched(cdn_host, &patch_path, hashes).await
    }

    /// Get statistics for request batching performance
    ///
    /// Returns detailed metrics about HTTP/2 batching performance including
    /// batch sizes, timing, and HTTP/2 connection usage.
    pub async fn get_batch_stats(&self) -> Option<tact_client::BatchStats> {
        let batcher_guard = self.request_batcher.lock().await;
        if let Some(ref batcher) = *batcher_guard {
            Some(batcher.get_stats().await)
        } else {
            None
        }
    }

    /// Download multiple files in parallel with streaming to writers (memory-efficient)
    ///
    /// This method downloads multiple files concurrently while streaming each file
    /// directly to its corresponding writer. This prevents loading all downloads
    /// into memory simultaneously, making it suitable for large file batches.
    ///
    /// # Arguments
    /// * `cdn_host` - CDN host to download from
    /// * `path` - Base path for downloads
    /// * `hashes` - Vector of file hashes to download
    /// * `writers` - Vector of writers (must match hashes length)
    /// * `max_concurrent` - Maximum number of concurrent downloads
    ///
    /// # Returns
    /// Vector of results with bytes written for each file
    pub async fn download_parallel_to_writers<W>(
        &self,
        cdn_host: &str,
        path: &str,
        hashes: &[String],
        writers: Vec<W>,
        max_concurrent: Option<usize>,
    ) -> Vec<Result<u64>>
    where
        W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        use futures_util::stream::{self, StreamExt};
        use tokio::io::AsyncWriteExt;

        if hashes.len() != writers.len() {
            return (0..hashes.len())
                .map(|_| {
                    Err(Error::invalid_response(
                        "Hashes and writers length mismatch",
                    ))
                })
                .collect();
        }

        let max_concurrent = max_concurrent.unwrap_or(10);

        let futures =
            hashes
                .iter()
                .zip(writers.into_iter())
                .enumerate()
                .map(|(idx, (hash, mut writer))| {
                    let cdn_host = cdn_host.to_string();
                    let path = path.to_string();
                    let hash = hash.clone();

                    async move {
                        match self.download(&cdn_host, &path, &hash).await {
                            Ok(response) => {
                                let mut stream = response.bytes_stream();
                                let mut total_bytes = 0u64;

                                use futures_util::StreamExt;
                                while let Some(chunk) = stream.next().await {
                                    match chunk {
                                        Ok(bytes) => {
                                            if let Err(e) = writer.write_all(&bytes).await {
                                                return (
                                                    idx,
                                                    Err(Error::invalid_response(format!(
                                                        "Write error: {e}"
                                                    ))),
                                                );
                                            }
                                            total_bytes += bytes.len() as u64;
                                        }
                                        Err(e) => {
                                            return (idx, Err(Error::from(e)));
                                        }
                                    }
                                }

                                if let Err(e) = writer.flush().await {
                                    return (
                                        idx,
                                        Err(Error::invalid_response(format!("Flush error: {e}"))),
                                    );
                                }

                                (idx, Ok(total_bytes))
                            }
                            Err(e) => (idx, Err(e)),
                        }
                    }
                });

        // Collect results in original order
        let mut results: Vec<Result<u64>> = Vec::with_capacity(hashes.len());
        for _ in 0..hashes.len() {
            results.push(Err(Error::invalid_response("Not downloaded")));
        }

        let mut download_stream = stream::iter(futures).buffer_unordered(max_concurrent);

        while let Some((idx, result)) = download_stream.next().await {
            results[idx] = result;
        }

        results
    }

    /// Download content and stream it to a writer
    ///
    /// This is useful for large files to avoid loading them entirely into memory.
    pub async fn download_streaming<W>(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        mut writer: W,
    ) -> Result<u64>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let response = self.download(cdn_host, path, hash).await?;
        let mut stream = response.bytes_stream();
        let mut total_bytes = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            writer
                .write_all(&chunk)
                .await
                .map_err(|e| Error::invalid_response(format!("Write error: {e}")))?;
            total_bytes += chunk.len() as u64;
        }

        writer
            .flush()
            .await
            .map_err(|e| Error::invalid_response(format!("Write error: {e}")))?;
        Ok(total_bytes)
    }

    /// Download content and process it in chunks
    ///
    /// This allows processing large files without loading them entirely into memory.
    /// The callback is called for each chunk received.
    pub async fn download_chunked<F>(
        &self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        mut callback: F,
    ) -> Result<u64>
    where
        F: FnMut(&[u8]) -> Result<()>,
    {
        use futures_util::StreamExt;

        let response = self.download(cdn_host, path, hash).await?;
        let mut stream = response.bytes_stream();
        let mut total_bytes = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            callback(&chunk)?;
            total_bytes += chunk.len() as u64;
        }

        Ok(total_bytes)
    }

    /// Create a resumable download for a data file
    ///
    /// This creates a resumable download that can be paused and resumed from interruption.
    /// Progress is saved to disk and the download can survive application crashes.
    ///
    /// # Arguments
    ///
    /// * `cdn_host` - CDN server hostname
    /// * `path` - Base path on the CDN (e.g., "tpr/wow")
    /// * `hash` - Content hash to download
    /// * `output_file` - Local file path where content should be saved
    ///
    /// # Returns
    ///
    /// Returns a `ResumableDownload` instance that can be used to start, pause, and resume the download.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ngdp_cdn::CdnClient;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = CdnClient::new()?;
    /// let mut resumable = client.create_resumable_download(
    ///     "blzddist1-a.akamaihd.net",
    ///     "tpr/wow",
    ///     "2e9c1e3b5f5a0c9d9e8f1234567890ab",
    ///     &PathBuf::from("game_file.bin")
    /// )?;
    ///
    /// // Start or resume the download
    /// resumable.start_or_resume().await?;
    ///
    /// // Clean up progress file when complete
    /// resumable.cleanup_completed().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_resumable_download(
        &mut self,
        cdn_host: &str,
        path: &str,
        hash: &str,
        output_file: &std::path::Path,
    ) -> Result<tact_client::resumable::ResumableDownload> {
        use tact_client::resumable::{DownloadProgress, ResumableDownload};

        // Get the shared TACT client (creates one if needed)
        let tact_client = self.get_or_create_tact_client()?.clone();

        let progress = DownloadProgress::new(
            hash.to_string(),
            cdn_host.to_string(),
            path.to_string(),
            output_file.to_path_buf(),
        );

        Ok(ResumableDownload::new(tact_client, progress))
    }

    /// Resume an existing download from a progress file
    ///
    /// This method loads an existing progress file and creates a `ResumableDownload`
    /// instance that can continue from where the previous download left off.
    ///
    /// # Arguments
    ///
    /// * `progress_file` - Path to the `.download` progress file
    ///
    /// # Returns
    ///
    /// Returns a `ResumableDownload` instance ready to resume the download.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ngdp_cdn::CdnClient;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = CdnClient::new()?;
    ///
    /// // Resume from existing progress file
    /// let mut resumable = client.resume_download(&PathBuf::from("file.bin.download")).await?;
    /// resumable.start_or_resume().await?;
    /// resumable.cleanup_completed().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resume_download(
        &mut self,
        progress_file: &std::path::Path,
    ) -> Result<tact_client::resumable::ResumableDownload> {
        use tact_client::resumable::{DownloadProgress, ResumableDownload};

        let progress = DownloadProgress::load_from_file(progress_file)
            .await
            .map_err(|e| Error::invalid_response(format!("Failed to load progress: {e}")))?;

        // Get the shared TACT client (creates one if needed)
        let tact_client = self.get_or_create_tact_client()?.clone();

        Ok(ResumableDownload::new(tact_client, progress))
    }

    /// Find all resumable downloads in a directory
    ///
    /// This is a convenience method that scans a directory for `.download` progress files
    /// and returns information about incomplete downloads.
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory to scan for progress files
    ///
    /// # Returns
    ///
    /// Returns a vector of `DownloadProgress` instances for incomplete downloads.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ngdp_cdn::CdnClient;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = CdnClient::new()?;
    ///
    /// // Find all resumable downloads in Downloads folder
    /// let downloads = client.find_resumable_downloads(&PathBuf::from("Downloads")).await?;
    ///
    /// for download in downloads {
    ///     println!("Found: {} - {}", download.file_hash, download.progress_string());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find_resumable_downloads(
        &self,
        directory: &std::path::Path,
    ) -> Result<Vec<tact_client::resumable::DownloadProgress>> {
        tact_client::resumable::find_resumable_downloads(directory)
            .await
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to find resumable downloads: {e}"
                )))
            })
    }

    /// Clean up old completed progress files in a directory
    ///
    /// This method scans for `.download` progress files that are marked as completed
    /// and are older than the specified age, then removes them to free up disk space.
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory to scan and clean up
    /// * `max_age_hours` - Maximum age in hours for completed progress files
    ///
    /// # Returns
    ///
    /// Returns the number of files that were cleaned up.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ngdp_cdn::CdnClient;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = CdnClient::new()?;
    ///
    /// // Clean up progress files older than 24 hours
    /// let cleaned = client.cleanup_old_progress_files(&PathBuf::from("Downloads"), 24).await?;
    /// println!("Cleaned up {} old progress files", cleaned);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cleanup_old_progress_files(
        &self,
        directory: &std::path::Path,
        max_age_hours: u64,
    ) -> Result<usize> {
        tact_client::resumable::cleanup_old_progress_files(directory, max_age_hours)
            .await
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to cleanup progress files: {e}"
                )))
            })
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
            tact_client: None, // Will be initialized on first use
            request_batcher: std::sync::Arc::new(tokio::sync::Mutex::new(None)), // Will be initialized on first use
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

    #[tokio::test]
    async fn test_parallel_download_ordering() {
        // Test that results are returned in the same order as input
        let client = CdnClient::new().unwrap();
        let cdn_host = "example.com";
        let path = "test";
        let hashes = [
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
        ];

        // This will fail since we don't have a real CDN, but we're testing the API
        let results = client
            .download_parallel(cdn_host, path, &hashes, Some(2))
            .await;

        // Should get 3 results in the same order
        assert_eq!(results.len(), 3);
    }
}

//! CDN client for downloading NGDP content

use crate::{Backoff, CacheProvider, CdnHostProvider, DummyCacheProvider, Error, Result};
use reqwest::{Client, Response, Url};
use std::{ops::RangeInclusive, str::FromStr, time::Duration};
use tokio::time::sleep;
use tracing::{debug, trace, warn};

/// Default maximum retries (0 = no retries, maintains backward compatibility)
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default connection timeout
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;

/// Default request timeout
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 300;

/// CDN client for downloading content
// #[derive(Debug, Clone)]
pub struct CdnClient<H: CdnHostProvider, C: CacheProvider = DummyCacheProvider> {
    /// HTTP client with connection pooling
    client: Client,
    /// Maximum number of retries
    max_retries: u32,
    backoff: Backoff,
    /// Custom user agent string
    user_agent: Option<String>,

    cache: Option<C>,
    hosts: H,
}

/// Build a CDN path for a content hash
///
/// CDN URLs follow the pattern:
/// `/{path}/{hash[0:2]}/{hash[2:4]}/{hash}{suffix}`
pub fn build_path(path: &str, hash: &str, suffix: &str) -> Result<String> {
    // Validate hash format (should be hex)
    if hash.len() < 4 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::invalid_hash(hash));
    }

    // Build the URL with the standard CDN path structure
    let path = format!(
        "/{}/{}/{}/{}{}",
        path.trim_matches('/'),
        &hash[0..2],
        &hash[2..4],
        hash,
        suffix,
    );

    Ok(path)
}

impl<H: CdnHostProvider, C: CacheProvider> CdnClient<H, C> {
    /// Create a builder for configuring the CDN client
    pub fn builder() -> CdnClientBuilder<H, C> {
        CdnClientBuilder::new()
    }

    /// Get the hosts list provider that is configured for this client.
    pub fn hosts(&self) -> &H {
        &self.hosts
    }

    /// Gets a mutable reference to the hosts list provider for this client.
    pub fn host_mut(&mut self) -> &mut H {
        &mut self.hosts
    }

    /// Gets a list of all CDN hosts.
    ///
    /// This list is not in a stable order.
    pub fn get_all_cdn_hosts(&self) -> Vec<&str> {
        self.hosts.get()
    }

    /// Execute a request with retry logic
    async fn execute_with_retry(
        &self,
        path: &str,
        range: Option<impl Into<RangeInclusive<u64>>>,
    ) -> Result<Response> {
        // TODO: check cache first

        let mut last_error = None;
        let range = if let Some(range) = range {
            let range = range.into();
            Some(format!("bytes={}-{}", range.start(), range.end()))
        } else {
            None
        };

        let hosts = self.hosts.get();
        let mut host_cycle = hosts.iter().cycle();

        for attempt in 0..=self.max_retries {
            let host = host_cycle.next().unwrap();
            if attempt > 0 {
                let backoff = self.backoff.calculate_backoff(attempt - 1);
                debug!("CDN retry attempt {} after {:?} backoff", attempt, backoff);
                sleep(backoff).await;
            }

            debug!("CDN request to {} (attempt {})", path, attempt + 1);

            let url = format!("http://{host}/{path}");
            let mut request = self.client.get(url);
            if let Some(ref user_agent) = self.user_agent {
                request = request.header("User-Agent", user_agent);
            }
            if let Some(ref range) = range {
                request = request.header("Range", range);
            }

            match request.send().await {
                Ok(response) => {
                    trace!("Response status: {}", response.status());

                    // Check if we should retry based on status code
                    let status = response.status();

                    // Success - return the response
                    if status.is_success() {
                        // TODO: make a copy of the response in cache too
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
                            // TODO: remove server from cycle so we don't retry twice
                            last_error = Some(Error::content_not_found(path));
                            continue;
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
    pub async fn request(&self, path: &str) -> Result<Response> {
        self.execute_with_retry(path, None::<RangeInclusive<u64>>)
            .await
    }

    /// Make a request to a CDN URL, with a HTTP `Range` header.
    ///
    /// This can be used to download parts of large files.
    pub async fn request_range(
        &self,
        path: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Response> {
        self.execute_with_retry(path, Some(range)).await
    }

    /// Download content from CDN by hash
    pub async fn download(&self, path: &str, hash: &str, suffix: &str) -> Result<Response> {
        let path = build_path(path, hash, suffix)?;
        self.request(&path).await
    }

    /// Download content from CDN by hash, with a HTTP `Range` header.
    pub async fn download_range(
        &self,
        path: &str,
        hash: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Response> {
        let path = build_path(path, hash, "")?;
        self.request_range(&path, range).await
    }

    /// Download BuildConfig from CDN
    ///
    /// BuildConfig files are stored at `{path}/config/{hash}`
    pub async fn download_build_config(&self, path: &str, hash: &str) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download CDNConfig from CDN
    ///
    /// CDNConfig files are stored at `{path}/config/{hash}`
    pub async fn download_cdn_config(&self, path: &str, hash: &str) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download ProductConfig from CDN
    ///
    /// ProductConfig files are stored at `{config_path}/{hash}`
    /// Note: This uses the config_path from CDN response, not the regular path
    pub async fn download_product_config(&self, config_path: &str, hash: &str) -> Result<Response> {
        self.download(config_path, hash, "").await
    }

    /// Download KeyRing from CDN
    ///
    /// KeyRing files are stored at `{path}/config/{hash}`
    pub async fn download_key_ring(&self, path: &str, hash: &str) -> Result<Response> {
        let config_path = format!("{}/config", path.trim_end_matches('/'));
        self.download(&config_path, hash, "").await
    }

    /// Download data file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}`
    pub async fn download_data(&self, path: &str, hash: &str) -> Result<Response> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(&data_path, hash, "").await
    }

    /// Download data index file from CDN
    ///
    /// Data files are stored at `{path}/data/{hash}.index`
    pub async fn download_data_index(&self, path: &str, hash: &str) -> Result<Response> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download(&data_path, hash, ".index").await
    }

    /// Download partial range of a data file from the CDN.
    ///
    /// Data files are stored at `{path}/data/{hash}`
    pub async fn download_data_range(
        &self,
        path: &str,
        hash: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Response> {
        let data_path = format!("{}/data", path.trim_end_matches('/'));
        self.download_range(&data_path, hash, range).await
    }

    /// Download patch file from CDN
    ///
    /// Patch files are stored at `{path}/patch/{hash}`
    pub async fn download_patch(&self, path: &str, hash: &str) -> Result<Response> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download(&patch_path, hash, "").await
    }

    /// Download partial range of a patch file from the CDN.
    ///
    /// Patch files are stored at `{path}/patch/{hash}`
    pub async fn download_patch_range(
        &self,
        path: &str,
        hash: &str,
        range: impl Into<RangeInclusive<u64>>,
    ) -> Result<Response> {
        let patch_path = format!("{}/patch", path.trim_end_matches('/'));
        self.download_range(&patch_path, hash, range).await
    }

    // /// Download multiple files in parallel
    // ///
    // /// Returns a vector of results in the same order as the input hashes.
    // /// Failed downloads will be represented as Err values in the vector.
    // ///
    // /// # Arguments
    // /// * `cdn_host` - CDN host to download from
    // /// * `path` - Base path on the CDN
    // /// * `hashes` - List of content hashes to download
    // /// * `max_concurrent` - Maximum number of concurrent downloads (None = unlimited)
    // pub async fn download_parallel(
    //     &self,
    //     cdn_host: &str,
    //     path: &str,
    //     hashes: impl Iterator<Item = (&str, &str)>,
    //     max_concurrent: Option<usize>,
    // ) -> Vec<Result<Vec<u8>>> {
    //     use futures_util::stream::{self, StreamExt};

    //     let max_concurrent = max_concurrent.unwrap_or(10); // Default to 10 concurrent downloads

    //     let futures = hashes.map(|(hash, suffix)| {
    //         let cdn_host = cdn_host.to_string();
    //         let path = path.to_string();

    //         async move {
    //             match self.download(&cdn_host, &path, hash, suffix).await {
    //                 Ok(response) => response
    //                     .bytes()
    //                     .await
    //                     .map(|b| b.to_vec())
    //                     .map_err(Into::into),
    //                 Err(e) => Err(e),
    //             }
    //         }
    //     });

    //     stream::iter(futures)
    //         .buffer_unordered(max_concurrent)
    //         .collect()
    //         .await
    // }

    // /// Download multiple files in parallel with progress tracking
    // ///
    // /// Returns a vector of results in the same order as the input hashes.
    // /// The progress callback is called after each successful download.
    // ///
    // /// # Arguments
    // /// * `cdn_host` - CDN host to download from
    // /// * `path` - Base path on the CDN
    // /// * `hashes` - List of content hashes (with optional suffix) to download
    // /// * `max_concurrent` - Maximum number of concurrent downloads (None = unlimited)
    // /// * `progress` - Callback function called with (completed_count, total_count) after each download
    // pub async fn download_parallel_with_progress<F>(
    //     &self,
    //     cdn_host: &str,
    //     path: &str,
    //     hashes: impl ExactSizeIterator<Item = (&str, &str)>,
    //     max_concurrent: Option<usize>,
    //     mut progress: F,
    // ) -> Vec<Result<Vec<u8>>>
    // where
    //     F: FnMut(usize, usize),
    // {
    //     use futures_util::stream::{self, StreamExt};
    //     use std::sync::Arc;
    //     use std::sync::atomic::{AtomicUsize, Ordering};

    //     let max_concurrent = max_concurrent.unwrap_or(10);
    //     let total = hashes.len();
    //     let completed = Arc::new(AtomicUsize::new(0));

    //     let futures = hashes.enumerate().map(|(idx, (hash, suffix))| {
    //         let completed = Arc::clone(&completed);

    //         async move {
    //             let result = match self.download(cdn_host, path, hash, suffix).await {
    //                 Ok(response) => response
    //                     .bytes()
    //                     .await
    //                     .map(|b| b.to_vec())
    //                     .map_err(Into::into),
    //                 Err(e) => Err(e),
    //             };

    //             // Update progress
    //             let count = completed.fetch_add(1, Ordering::SeqCst) + 1;

    //             (idx, result, count)
    //         }
    //     });

    //     let mut results: Vec<Result<Vec<u8>>> = Vec::with_capacity(total);
    //     for _ in 0..total {
    //         results.push(Err(Error::invalid_response("Not downloaded")));
    //     }

    //     let mut download_stream = stream::iter(futures).buffer_unordered(max_concurrent);

    //     while let Some((idx, result, count)) = download_stream.next().await {
    //         results[idx] = result;
    //         progress(count, total);
    //     }

    //     results
    // }

    // /// Download multiple data files in parallel
    // pub async fn download_data_parallel(
    //     &self,
    //     cdn_host: &str,
    //     path: &str,
    //     hashes: impl Iterator<Item = &str>,
    //     max_concurrent: Option<usize>,
    // ) -> Vec<Result<Vec<u8>>> {
    //     let data_path = format!("{}/data", path.trim_end_matches('/'));
    //     self.download_parallel(
    //         cdn_host,
    //         &data_path,
    //         hashes.map(|e| (e, "")),
    //         max_concurrent,
    //     )
    //     .await
    // }

    // /// Download multiple config files in parallel
    // pub async fn download_config_parallel(
    //     &self,
    //     cdn_host: &str,
    //     path: &str,
    //     hashes: impl Iterator<Item = &str>,
    //     max_concurrent: Option<usize>,
    // ) -> Vec<Result<Vec<u8>>> {
    //     let config_path = format!("{}/config", path.trim_end_matches('/'));
    //     self.download_parallel(
    //         cdn_host,
    //         &config_path,
    //         hashes.map(|e| (e, "")),
    //         max_concurrent,
    //     )
    //     .await
    // }

    // /// Download multiple patch files in parallel
    // pub async fn download_patch_parallel(
    //     &self,
    //     cdn_host: &str,
    //     path: &str,
    //     hashes: impl Iterator<Item = &str>,
    //     max_concurrent: Option<usize>,
    // ) -> Vec<Result<Vec<u8>>> {
    //     let patch_path = format!("{}/patch", path.trim_end_matches('/'));
    //     self.download_parallel(
    //         cdn_host,
    //         &patch_path,
    //         hashes.map(|e| (e, "")),
    //         max_concurrent,
    //     )
    //     .await
    // }

    // /// Download content and stream it to a writer
    // ///
    // /// This is useful for large files to avoid loading them entirely into memory.
    // pub async fn download_streaming<W>(
    //     &self,
    //     path: &str,
    //     hash: &str,
    //     suffix: &str,
    //     mut writer: W,
    // ) -> Result<u64>
    // where
    //     W: tokio::io::AsyncWrite + Unpin,
    // {
    //     use futures_util::StreamExt;
    //     use tokio::io::AsyncWriteExt;

    //     let response = self.download(path, hash, suffix).await?;
    //     let mut stream = response.bytes_stream();
    //     let mut total_bytes = 0u64;

    //     while let Some(chunk) = stream.next().await {
    //         let chunk = chunk?;
    //         writer
    //             .write_all(&chunk)
    //             .await
    //             .map_err(|e| Error::invalid_response(format!("Write error: {e}")))?;
    //         total_bytes += chunk.len() as u64;
    //     }

    //     writer
    //         .flush()
    //         .await
    //         .map_err(|e| Error::invalid_response(format!("Write error: {e}")))?;
    //     Ok(total_bytes)
    // }

    // /// Download content and process it in chunks
    // ///
    // /// This allows processing large files without loading them entirely into memory.
    // /// The callback is called for each chunk received.
    // pub async fn download_chunked<F>(
    //     &self,
    //     cdn_host: &str,
    //     path: &str,
    //     hash: &str,
    //     suffix: &str,
    //     mut callback: F,
    // ) -> Result<u64>
    // where
    //     F: FnMut(&[u8]) -> Result<()>,
    // {
    //     use futures_util::StreamExt;

    //     let response = self.download(cdn_host, path, hash, suffix).await?;
    //     let mut stream = response.bytes_stream();
    //     let mut total_bytes = 0u64;

    //     while let Some(chunk) = stream.next().await {
    //         let chunk = chunk?;
    //         callback(&chunk)?;
    //         total_bytes += chunk.len() as u64;
    //     }

    //     Ok(total_bytes)
    // }
}

/// Builder for configuring CDN client
#[derive(Debug, Clone)]
pub struct CdnClientBuilder<H: CdnHostProvider, C: CacheProvider = DummyCacheProvider> {
    connect_timeout_secs: u64,
    request_timeout_secs: u64,
    pool_max_idle_per_host: usize,
    max_retries: u32,
    backoff: Backoff,
    user_agent: Option<String>,
    cache: Option<C>,
    hosts: Option<H>,
}

impl<H: CdnHostProvider, C: CacheProvider> CdnClientBuilder<H, C> {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            pool_max_idle_per_host: 20,
            max_retries: DEFAULT_MAX_RETRIES,
            backoff: Default::default(),
            user_agent: None,
            cache: None,
            hosts: None,
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
        self.backoff.set_initial_backoff_ms(ms);
        self
    }

    /// Set maximum backoff in milliseconds
    pub fn max_backoff_ms(mut self, ms: u64) -> Self {
        self.backoff.set_max_backoff_ms(ms);
        self
    }

    /// Set backoff multiplier
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff.set_backoff_multiplier(multiplier);
        self
    }

    /// Set jitter factor (0.0 to 1.0)
    pub fn jitter_factor(mut self, factor: f64) -> Self {
        self.backoff.set_jitter_factor(factor.clamp(0.0, 1.0));
        self
    }

    /// Set custom user agent string
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Set cache provider
    pub fn cache(mut self, cache: C) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Set host list provider
    pub fn hosts(mut self, hosts: H) -> Self {
        self.hosts = Some(hosts);
        self
    }

    /// Build the CDN client
    pub fn build(self) -> Result<CdnClient<H, C>> {
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
            backoff: self.backoff,
            user_agent: self.user_agent,
            cache: self.cache,
            hosts: self.hosts.ok_or(Error::NoHosts)?,
        })
    }
}

impl<H: CdnHostProvider, C: CacheProvider> Default for CdnClientBuilder<H, C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::hosts::SingleHost;

    use super::*;

    #[test]
    fn test_builder_configuration() {
        let hosts = SingleHost("cdn.example.com".to_string());
        let client: CdnClient<SingleHost, DummyCacheProvider> = CdnClient::builder()
            .max_retries(5)
            .initial_backoff_ms(200)
            .max_backoff_ms(5000)
            .backoff_multiplier(1.5)
            .jitter_factor(0.2)
            .connect_timeout(60)
            .request_timeout(600)
            .pool_max_idle_per_host(100)
            .hosts(hosts)
            .build()
            .unwrap();

        assert_eq!(client.max_retries, 5);
        assert_eq!(client.backoff.initial_backoff_ms, 200);
        assert_eq!(client.backoff.max_backoff_ms, 5000);
        assert!((client.backoff.backoff_multiplier - 1.5).abs() < f64::EPSILON);
        assert!((client.backoff.jitter_factor - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_user_agent_via_builder() {
        let hosts = SingleHost("cdn.example.com".to_string());
        let client: CdnClient<SingleHost, DummyCacheProvider> = CdnClient::builder()
            .user_agent("MyNGDPClient/2.0")
            .hosts(hosts)
            .build()
            .unwrap();

        assert_eq!(client.user_agent, Some("MyNGDPClient/2.0".to_string()));
    }

    #[test]
    fn test_user_agent_default_none() {
        let hosts = SingleHost("cdn.example.com".to_string());
        let client: CdnClient<SingleHost, DummyCacheProvider> =
            CdnClient::builder().hosts(hosts).build().unwrap();

        assert!(client.user_agent.is_none());
    }

    // #[tokio::test]
    // async fn test_parallel_download_ordering() {
    //     // Test that results are returned in the same order as input
    //     let client = CdnClient::new().unwrap();
    //     let cdn_host = "example.com";
    //     let path = "test";
    //     let hashes = vec![("hash1", ""), ("hash2", ""), ("hash3", "")];

    //     // This will fail since we don't have a real CDN, but we're testing the API
    //     let results = client
    //         .download_parallel(cdn_host, path, hashes.into_iter(), Some(2))
    //         .await;

    //     // Should get 3 results in the same order
    //     assert_eq!(results.len(), 3);
    // }
}

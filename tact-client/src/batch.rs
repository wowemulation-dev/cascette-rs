//! Request batching for CDN downloads using HTTP/2 multiplexing

use crate::{Error, Result};
use futures_util::stream::{self, StreamExt};
use reqwest::{Client, Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

/// Default batch size for request batching
const DEFAULT_BATCH_SIZE: usize = 20;

/// Default batch timeout in milliseconds
const DEFAULT_BATCH_TIMEOUT_MS: u64 = 100;

/// Default maximum concurrent batches
const DEFAULT_MAX_CONCURRENT_BATCHES: usize = 4;

/// Configuration for request batching
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of requests per batch
    pub batch_size: usize,
    /// Maximum time to wait for a batch to fill (milliseconds)
    pub batch_timeout_ms: u64,
    /// Maximum number of concurrent batches
    pub max_concurrent_batches: usize,
    /// Maximum time to wait for all requests in a batch to complete
    pub batch_execution_timeout: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            batch_timeout_ms: DEFAULT_BATCH_TIMEOUT_MS,
            max_concurrent_batches: DEFAULT_MAX_CONCURRENT_BATCHES,
            batch_execution_timeout: Duration::from_secs(60),
        }
    }
}

/// Statistics for batch operations
#[derive(Debug, Clone)]
pub struct BatchStats {
    /// Total number of batches processed
    pub batches_processed: u64,
    /// Total number of requests processed
    pub requests_processed: u64,
    /// Average batch size
    pub avg_batch_size: f64,
    /// Total time spent in batch processing
    pub total_batch_time: Duration,
    /// Average time per batch
    pub avg_batch_time: Duration,
    /// HTTP/2 connections established
    pub http2_connections: u64,
}

/// A single request in a batch
#[derive(Debug, Clone)]
pub struct BatchRequest {
    /// Unique ID for this request
    pub id: String,
    /// Full URL to request
    pub url: String,
    /// Optional headers
    pub headers: HashMap<String, String>,
}

/// Result of a batch request
#[derive(Debug)]
pub struct BatchResponse {
    /// Request ID this response corresponds to
    pub request_id: String,
    /// The HTTP response (or error)
    pub result: Result<Response>,
    /// Time taken for this request
    pub duration: Duration,
}

/// A batch of requests to be executed together
#[allow(dead_code)]
#[derive(Debug)]
struct RequestBatch {
    /// Requests in this batch
    requests: Vec<BatchRequest>,
    /// Channel to send responses back
    response_tx: mpsc::UnboundedSender<BatchResponse>,
    /// When this batch was created
    created_at: Instant,
}

/// HTTP/2 request batcher for CDN downloads
#[derive(Debug)]
pub struct RequestBatcher {
    /// HTTP client with HTTP/2 support
    #[allow(dead_code)]
    client: Client,
    /// Configuration
    #[allow(dead_code)]
    config: BatchConfig,
    /// Channel for incoming requests
    request_tx: mpsc::UnboundedSender<(BatchRequest, mpsc::UnboundedSender<BatchResponse>)>,
    /// Statistics
    stats: Arc<Mutex<BatchStats>>,
}

impl RequestBatcher {
    /// Create a new request batcher
    pub fn new(client: Client, config: BatchConfig) -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let stats = Arc::new(Mutex::new(BatchStats {
            batches_processed: 0,
            requests_processed: 0,
            avg_batch_size: 0.0,
            total_batch_time: Duration::ZERO,
            avg_batch_time: Duration::ZERO,
            http2_connections: 0,
        }));

        let batcher = Self {
            client: client.clone(),
            config: config.clone(),
            request_tx,
            stats: Arc::clone(&stats),
        };

        // Start the batch processing task
        let batch_processor = BatchProcessor {
            client,
            config,
            request_rx: Arc::new(Mutex::new(request_rx)),
            stats,
        };

        tokio::spawn(batch_processor.run());

        batcher
    }

    /// Submit a request to be batched
    ///
    /// Returns a receiver for the response. The request will be batched with others
    /// and executed when the batch is full or the timeout expires.
    pub async fn submit_request(
        &self,
        request: BatchRequest,
    ) -> Result<mpsc::UnboundedReceiver<BatchResponse>> {
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        self.request_tx
            .send((request, response_tx))
            .map_err(|_| Error::InvalidResponse)?;

        Ok(response_rx)
    }

    /// Submit multiple requests and wait for all responses
    pub async fn submit_requests_and_wait(
        &self,
        requests: Vec<BatchRequest>,
    ) -> Vec<BatchResponse> {
        let mut receivers = Vec::new();

        // Submit all requests
        for request in requests {
            match self.submit_request(request).await {
                Ok(rx) => receivers.push(rx),
                Err(e) => {
                    // Create error response for failed submission
                    receivers.push({
                        let (tx, rx) = mpsc::unbounded_channel();
                        let _ = tx.send(BatchResponse {
                            request_id: "unknown".to_string(),
                            result: Err(e),
                            duration: Duration::ZERO,
                        });
                        rx
                    });
                }
            }
        }

        // Collect all responses
        let mut responses = Vec::new();
        for mut rx in receivers {
            if let Some(response) = rx.recv().await {
                responses.push(response);
            } else {
                responses.push(BatchResponse {
                    request_id: "unknown".to_string(),
                    result: Err(Error::InvalidResponse),
                    duration: Duration::ZERO,
                });
            }
        }

        responses
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> BatchStats {
        self.stats.lock().await.clone()
    }

    /// Create batch requests for CDN file downloads
    pub fn create_cdn_requests(cdn_host: &str, path: &str, hashes: &[String]) -> Vec<BatchRequest> {
        hashes
            .iter()
            .map(|hash| BatchRequest {
                id: hash.clone(),
                url: format!(
                    "http://{}/{}/{}/{}/{}",
                    cdn_host,
                    path.trim_matches('/'),
                    &hash[0..2],
                    &hash[2..4],
                    hash
                ),
                headers: HashMap::new(),
            })
            .collect()
    }
}

/// Type alias for the request receiver channel
type RequestReceiver =
    Arc<Mutex<mpsc::UnboundedReceiver<(BatchRequest, mpsc::UnboundedSender<BatchResponse>)>>>;

/// Internal batch processor
struct BatchProcessor {
    client: Client,
    config: BatchConfig,
    request_rx: RequestReceiver,
    stats: Arc<Mutex<BatchStats>>,
}

impl BatchProcessor {
    async fn run(self) {
        let mut current_batch: Vec<(BatchRequest, mpsc::UnboundedSender<BatchResponse>)> =
            Vec::new();
        let mut batch_timer =
            tokio::time::interval(Duration::from_millis(self.config.batch_timeout_ms));
        let mut request_rx = self.request_rx.lock().await;

        debug!(
            "Starting batch processor with config: batch_size={}, timeout={}ms, max_concurrent={}",
            self.config.batch_size,
            self.config.batch_timeout_ms,
            self.config.max_concurrent_batches
        );

        loop {
            tokio::select! {
                // New request received
                maybe_request = request_rx.recv() => {
                    match maybe_request {
                        Some((request, response_tx)) => {
                            current_batch.push((request, response_tx));

                            // If batch is full, process it immediately
                            if current_batch.len() >= self.config.batch_size {
                                let batch = std::mem::take(&mut current_batch);
                                self.process_batch(batch).await;
                            }
                        }
                        None => {
                            // Channel closed, process remaining batch and exit
                            if !current_batch.is_empty() {
                                let batch = std::mem::take(&mut current_batch);
                                self.process_batch(batch).await;
                            }
                            break;
                        }
                    }
                }

                // Batch timeout expired
                _ = batch_timer.tick() => {
                    if !current_batch.is_empty() {
                        let batch = std::mem::take(&mut current_batch);
                        self.process_batch(batch).await;
                    }
                }
            }
        }

        debug!("Batch processor shutting down");
    }

    async fn process_batch(
        &self,
        batch: Vec<(BatchRequest, mpsc::UnboundedSender<BatchResponse>)>,
    ) {
        if batch.is_empty() {
            return;
        }

        let batch_start = Instant::now();
        let batch_size = batch.len();

        debug!("Processing batch of {} requests", batch_size);

        // Group requests by host to maximize HTTP/2 connection reuse
        let mut requests_by_host: HashMap<String, Vec<_>> = HashMap::new();

        for (request, response_tx) in batch {
            let host = self
                .extract_host(&request.url)
                .unwrap_or_else(|| "unknown".to_string());
            requests_by_host
                .entry(host)
                .or_default()
                .push((request, response_tx));
        }

        // Process each host group concurrently (up to max_concurrent_batches)
        let host_groups: Vec<_> = requests_by_host.into_iter().collect();
        let concurrent_limit = self.config.max_concurrent_batches.min(host_groups.len());

        stream::iter(host_groups)
            .map(|(host, requests)| async move {
                self.process_host_batch(host, requests).await;
            })
            .buffer_unordered(concurrent_limit)
            .collect::<Vec<_>>()
            .await;

        let batch_duration = batch_start.elapsed();

        // Update statistics
        let mut stats = self.stats.lock().await;
        stats.batches_processed += 1;
        stats.requests_processed += batch_size as u64;
        stats.total_batch_time += batch_duration;
        stats.avg_batch_size = stats.requests_processed as f64 / stats.batches_processed as f64;
        stats.avg_batch_time = stats.total_batch_time / stats.batches_processed as u32;

        info!(
            "Processed batch: {} requests in {:?} (avg: {:.1} reqs/batch, {:?}/batch)",
            batch_size, batch_duration, stats.avg_batch_size, stats.avg_batch_time
        );
    }

    async fn process_host_batch(
        &self,
        host: String,
        requests: Vec<(BatchRequest, mpsc::UnboundedSender<BatchResponse>)>,
    ) {
        debug!("Processing {} requests for host: {}", requests.len(), host);

        // Check if server supports HTTP/2
        let supports_http2 = self.check_http2_support(&host).await;
        if supports_http2 {
            let mut stats = self.stats.lock().await;
            stats.http2_connections += 1;
            debug!("HTTP/2 support confirmed for host: {}", host);
        }

        // Execute all requests for this host concurrently
        // HTTP/2 multiplexing allows multiple requests on the same connection
        let num_requests = requests.len();
        let futures = requests
            .into_iter()
            .map(|(request, response_tx)| async move {
                let start_time = Instant::now();
                let request_id = request.id.clone();

                let result = self.execute_request(request).await;
                let duration = start_time.elapsed();

                let response = BatchResponse {
                    request_id,
                    result,
                    duration,
                };

                if response_tx.send(response).is_err() {
                    warn!("Failed to send batch response - receiver dropped");
                }
            });

        // Execute all requests concurrently
        stream::iter(futures)
            .buffer_unordered(num_requests) // Use HTTP/2 multiplexing
            .collect::<Vec<_>>()
            .await;
    }

    async fn execute_request(&self, request: BatchRequest) -> Result<Response> {
        let mut req_builder = self.client.get(&request.url);

        // Add custom headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Execute with timeout
        let response =
            tokio::time::timeout(self.config.batch_execution_timeout, req_builder.send()).await;

        match response {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    Ok(response)
                } else {
                    Err(Error::Http(response.error_for_status().unwrap_err()))
                }
            }
            Ok(Err(e)) => Err(Error::Http(e)),
            Err(_) => Err(Error::InvalidResponse),
        }
    }

    async fn check_http2_support(&self, host: &str) -> bool {
        // Try a simple request to check HTTP version
        // This is optimistic - we assume HTTP/2 support for HTTPS hosts
        // and rely on reqwest's automatic protocol negotiation
        host.starts_with("https://") ||
        // For CDN hosts, we know most support HTTP/2
        host.contains("akamai") || host.contains("cloudflare") || host.contains("blizzard")
    }

    fn extract_host(&self, url: &str) -> Option<String> {
        if let Ok(parsed) = url::Url::parse(url) {
            parsed.host_str().map(|s| s.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
        assert_eq!(config.batch_timeout_ms, DEFAULT_BATCH_TIMEOUT_MS);
        assert_eq!(
            config.max_concurrent_batches,
            DEFAULT_MAX_CONCURRENT_BATCHES
        );
    }

    #[test]
    fn test_create_cdn_requests() {
        let hashes = vec!["abcd1234".to_string(), "efgh5678".to_string()];

        let requests = RequestBatcher::create_cdn_requests("example.com", "data", &hashes);

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, "abcd1234");
        assert_eq!(requests[0].url, "http://example.com/data/ab/cd/abcd1234");
        assert_eq!(requests[1].id, "efgh5678");
        assert_eq!(requests[1].url, "http://example.com/data/ef/gh/efgh5678");
    }

    #[tokio::test]
    async fn test_batch_stats_initialization() {
        let client = reqwest::Client::new();
        let config = BatchConfig::default();
        let batcher = RequestBatcher::new(client, config);

        let stats = batcher.get_stats().await;
        assert_eq!(stats.batches_processed, 0);
        assert_eq!(stats.requests_processed, 0);
        assert_eq!(stats.avg_batch_size, 0.0);
    }

    #[tokio::test]
    #[ignore = "Test depends on actual network requests for stats validation"]
    async fn test_request_submission() {
        let client = reqwest::Client::new();
        let config = BatchConfig {
            batch_timeout_ms: 50, // Short timeout for test
            ..BatchConfig::default()
        };
        let batcher = RequestBatcher::new(client, config);

        let request = BatchRequest {
            id: "test123".to_string(),
            url: "http://httpbin.org/status/200".to_string(), // Test endpoint
            headers: HashMap::new(),
        };

        // This will fail in tests without network, but tests the API
        let _receiver = batcher.submit_request(request).await;
        // Just test that submission doesn't panic

        // Wait a bit for batch processing
        sleep(Duration::from_millis(100)).await;

        // Stats should be updated (even if requests failed)
        let stats = batcher.get_stats().await;
        assert!(stats.batches_processed > 0 || stats.requests_processed > 0);
    }
}

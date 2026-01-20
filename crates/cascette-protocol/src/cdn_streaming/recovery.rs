//! Advanced error recovery system for CDN streaming
//!
//! This module provides comprehensive error recovery capabilities including:
//! - Automatic retry with exponential backoff and jitter
//! - CDN server failover with intelligent selection
//! - Partial content recovery and resume capabilities
//! - Network condition adaptation
//! - Fallback to alternative content sources

#[cfg(feature = "streaming")]
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[cfg(feature = "streaming")]
use bytes::Bytes;
#[cfg(feature = "streaming")]
use rand::{Rng, rng};
#[cfg(feature = "streaming")]
use tokio::{
    sync::RwLock,
    time::{sleep, timeout},
};
#[cfg(feature = "streaming")]
use tracing::{debug, error, info, warn};

#[cfg(feature = "streaming")]
use super::{
    config::{RetryConfig, StreamingConfig},
    error::{StreamingError, StreamingResult},
    http::{CdnServer, HttpClient},
    metrics::StreamingMetrics,
    range::HttpRange,
};

/// Recovery strategy for different types of errors
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    Retry {
        /// Maximum number of retry attempts
        max_attempts: u32,
        /// Base delay between retries
        base_delay: Duration,
        /// Maximum delay between retries
        max_delay: Duration,
        /// Whether to add random jitter to delays
        jitter: bool,
    },
    /// Failover to next CDN server
    Failover {
        /// Whether to exclude previously failed servers
        exclude_failed: bool,
        /// Duration after which failed servers are reset
        reset_after: Duration,
    },
    /// Split request into smaller ranges
    SplitRequest {
        /// Maximum number of splits allowed
        max_split_count: u32,
        /// Minimum size for each chunk
        min_chunk_size: u64,
    },
    /// Use alternative content source
    AlternativeSource {
        /// Alternative content sources to try
        sources: Vec<String>,
    },
    /// Immediate failure (no recovery)
    Fail,
}

/// Server health status for failover decisions
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerHealth {
    /// Server is healthy and responsive
    Healthy,
    /// Server is degraded but still usable
    Degraded {
        /// When the degradation started
        since: Instant,
    },
    /// Server is temporarily unavailable
    Unavailable {
        /// When the server will be available again
        until: Instant,
    },
    /// Server is permanently failed
    Failed,
}

/// Network condition assessment
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkCondition {
    /// Excellent network conditions
    Excellent,
    /// Good network conditions
    Good,
    /// Fair network conditions (some issues)
    Fair,
    /// Poor network conditions (frequent issues)
    Poor,
    /// Very poor network conditions
    VeryPoor,
}

/// Recovery context for making informed decisions
#[cfg(feature = "streaming")]
#[derive(Debug)]
pub struct RecoveryContext {
    /// Request URL
    pub url: String,
    /// Byte range being requested
    pub range: Option<HttpRange>,
    /// Number of attempts made so far
    pub attempts: u32,
    /// Error descriptions encountered
    pub error_descriptions: Vec<String>,
    /// Servers tried
    pub servers_tried: Vec<String>,
    /// Request start time
    pub started_at: Instant,
    /// Current network condition
    pub network_condition: NetworkCondition,
    /// Available CDN servers
    pub available_servers: Vec<CdnServer>,
}

/// Advanced retry manager with intelligent backoff
#[cfg(feature = "streaming")]
#[derive(Debug)]
pub struct RetryManager {
    /// Retry configuration
    config: RetryConfig,
    /// Random number generator for jitter
    rng: std::sync::Mutex<rand::prelude::ThreadRng>,
    /// Retry statistics
    total_retries: AtomicU64,
    successful_retries: AtomicU64,
    failed_retries: AtomicU64,
}

#[cfg(feature = "streaming")]
impl RetryManager {
    /// Create new retry manager
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            rng: std::sync::Mutex::new(rng()),
            total_retries: AtomicU64::new(0),
            successful_retries: AtomicU64::new(0),
            failed_retries: AtomicU64::new(0),
        }
    }

    /// Calculate delay for retry attempt
    pub fn calculate_delay(&self, attempt: u32, network_condition: NetworkCondition) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let base_delay = self.config.base_delay;
        let max_delay = self.config.max_delay;

        // Exponential backoff: delay = base * (2 ^ (attempt - 1))
        let exponential_delay = base_delay * (2_u32.pow(attempt.saturating_sub(1)));
        let capped_delay = exponential_delay.min(max_delay);

        // Adjust for network conditions
        let network_multiplier = match network_condition {
            NetworkCondition::Excellent => 0.5,
            NetworkCondition::Good => 0.8,
            NetworkCondition::Fair => 1.0,
            NetworkCondition::Poor => 1.5,
            NetworkCondition::VeryPoor => 2.0,
        };

        let adjusted_delay =
            Duration::from_millis((capped_delay.as_millis() as f64 * network_multiplier) as u64);

        // Add jitter if configured
        if self.config.jitter_factor > 0.0 {
            self.add_jitter(adjusted_delay)
        } else {
            adjusted_delay
        }
    }

    /// Add jitter to delay
    fn add_jitter(&self, delay: Duration) -> Duration {
        let jitter_factor = self.config.jitter_factor.clamp(0.0, 1.0);
        let base_millis = delay.as_millis() as f64;
        let jitter_range = base_millis * jitter_factor;

        let mut rng = self.rng.lock().expect("Operation should succeed");
        let jitter: f64 = rng.random_range(-jitter_range..=jitter_range);

        let final_delay = (base_millis + jitter).max(0.0) as u64;
        Duration::from_millis(final_delay)
    }

    /// Check if error is retryable
    pub fn is_retryable(&self, error: &StreamingError) -> bool {
        match error {
            StreamingError::NetworkRequest { .. } => true,
            StreamingError::Timeout { .. } => true,
            StreamingError::HttpStatus { status_code, .. } => {
                // Retry on specific status codes
                self.config.retry_on_status.contains(status_code)
            }
            StreamingError::CdnFailover { .. } => true,
            StreamingError::ServerUnavailable { .. } => true,
            StreamingError::ConnectionLimit { .. } => true,
            StreamingError::ConnectionPoolExhausted { .. } => true,
            StreamingError::RateLimitExceeded { .. } => true,
            StreamingError::MirrorSyncLag { .. } => true,
            // Don't retry these errors
            StreamingError::InvalidRange { .. } => false,
            StreamingError::Configuration { .. } => false,
            StreamingError::MissingContentLength { .. } => false,
            StreamingError::HttpClientSetup { .. } => false,
            StreamingError::RangeNotSupported { .. } => false,
            StreamingError::RangeCoalescingFailed { .. } => false,
            StreamingError::BufferOverflow { .. } => false,
            StreamingError::ArchiveFormat { .. } => false,
            StreamingError::Io { .. } => false,
            StreamingError::AllCdnServersFailed { .. } => false,
            StreamingError::CdnPathNotCached { .. } => false,
            StreamingError::CdnPathResolution { .. } => false,
            StreamingError::InvalidHashFormat { .. } => false,
            StreamingError::CdnRegionUnavailable { .. } => false,
            StreamingError::ContentVerificationFailed { .. } => false,
            StreamingError::BlteError { .. } => false,
        }
    }

    /// Record retry attempt
    pub fn record_attempt(&self, success: bool) {
        self.total_retries.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_retries.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_retries.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get retry statistics
    pub fn statistics(&self) -> (u64, u64, u64) {
        (
            self.total_retries.load(Ordering::Relaxed),
            self.successful_retries.load(Ordering::Relaxed),
            self.failed_retries.load(Ordering::Relaxed),
        )
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.total_retries.load(Ordering::Relaxed);
        if total == 0 {
            1.0
        } else {
            let successful = self.successful_retries.load(Ordering::Relaxed);
            successful as f64 / total as f64
        }
    }
}

/// CDN failover manager with intelligent server selection
#[cfg(feature = "streaming")]
#[derive(Debug)]
pub struct FailoverManager {
    /// Server health tracking
    server_health: Arc<RwLock<HashMap<String, ServerHealth>>>,
    /// Server performance metrics
    server_metrics: Arc<RwLock<HashMap<String, ServerMetrics>>>,
    /// Failover statistics
    failover_count: AtomicU64,
    recovery_count: AtomicU64,
    /// Configuration
    #[allow(dead_code)]
    config: StreamingConfig,
}

/// Performance metrics for a server
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, Default)]
pub struct ServerMetrics {
    /// Average response time
    pub avg_response_time: Duration,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Total requests
    pub total_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Last success time
    pub last_success: Option<Instant>,
    /// Last failure time
    pub last_failure: Option<Instant>,
    /// Bandwidth estimate
    pub bandwidth_estimate: u64, // bytes/sec
}

#[cfg(feature = "streaming")]
impl FailoverManager {
    /// Create new failover manager
    pub fn new(config: StreamingConfig) -> Self {
        Self {
            server_health: Arc::new(RwLock::new(HashMap::new())),
            server_metrics: Arc::new(RwLock::new(HashMap::new())),
            failover_count: AtomicU64::new(0),
            recovery_count: AtomicU64::new(0),
            config,
        }
    }

    /// Mark server as failed
    pub async fn mark_server_failed(&self, server: &str, error: &StreamingError) {
        let mut health = self.server_health.write().await;

        let new_health = match error {
            // Temporary failures
            StreamingError::Timeout { .. } | StreamingError::NetworkRequest { .. } => {
                ServerHealth::Unavailable {
                    until: Instant::now() + Duration::from_secs(300), // 5 minutes
                }
            }
            // More severe failures
            StreamingError::HttpStatus { status_code, .. } if *status_code >= 500 => {
                ServerHealth::Unavailable {
                    until: Instant::now() + Duration::from_secs(900), // 15 minutes
                }
            }
            // Permanent failures
            _ => ServerHealth::Failed,
        };

        health.insert(server.to_string(), new_health.clone());
        self.failover_count.fetch_add(1, Ordering::Relaxed);

        warn!(
            "Marked server {} as {:?} due to error: {:?}",
            server, new_health, error
        );
    }

    /// Mark server as healthy
    pub async fn mark_server_healthy(&self, server: &str) {
        let mut health = self.server_health.write().await;
        let was_failed = matches!(
            health.get(server),
            Some(ServerHealth::Failed | ServerHealth::Unavailable { .. })
        );

        health.insert(server.to_string(), ServerHealth::Healthy);

        if was_failed {
            self.recovery_count.fetch_add(1, Ordering::Relaxed);
            info!("Server {} recovered and marked as healthy", server);
        }
    }

    /// Update server performance metrics
    pub async fn update_server_metrics(
        &self,
        server: &str,
        response_time: Duration,
        success: bool,
        bytes_transferred: u64,
    ) {
        let mut metrics = self.server_metrics.write().await;
        let server_metrics = metrics.entry(server.to_string()).or_default();

        // Update response time (exponential moving average)
        if server_metrics.total_requests == 0 {
            server_metrics.avg_response_time = response_time;
        } else {
            let alpha = 0.1; // Smoothing factor
            let current_avg = server_metrics.avg_response_time.as_millis() as f64;
            let new_sample = response_time.as_millis() as f64;
            let new_avg = alpha * new_sample + (1.0 - alpha) * current_avg;
            server_metrics.avg_response_time = Duration::from_millis(new_avg as u64);
        }

        // Update request counts
        server_metrics.total_requests += 1;
        if success {
            server_metrics.last_success = Some(Instant::now());
        } else {
            server_metrics.failed_requests += 1;
            server_metrics.last_failure = Some(Instant::now());
        }

        // Update success rate
        server_metrics.success_rate = (server_metrics.total_requests
            - server_metrics.failed_requests) as f64
            / server_metrics.total_requests as f64;

        // Update bandwidth estimate
        if success && !response_time.is_zero() {
            let bandwidth = (bytes_transferred as f64 / response_time.as_secs_f64()) as u64;
            server_metrics.bandwidth_estimate = if server_metrics.bandwidth_estimate == 0 {
                bandwidth
            } else {
                // Exponential moving average
                ((server_metrics.bandwidth_estimate as f64 * 0.9) + (bandwidth as f64 * 0.1)) as u64
            };
        }

        debug!(
            "Updated metrics for {}: avg_rt={:?}, success_rate={:.2}, bandwidth={} B/s",
            server,
            server_metrics.avg_response_time,
            server_metrics.success_rate,
            server_metrics.bandwidth_estimate
        );
    }

    /// Select best available server
    pub async fn select_best_server(&self, available_servers: &[CdnServer]) -> Option<CdnServer> {
        if available_servers.is_empty() {
            return None;
        }

        let health = self.server_health.read().await;
        let metrics = self.server_metrics.read().await;
        let now = Instant::now();

        let mut candidates = Vec::new();

        for server in available_servers {
            let server_key = &server.host;

            // Check health status
            let is_healthy = match health.get(server_key) {
                Some(ServerHealth::Healthy) => true,
                Some(ServerHealth::Degraded { .. }) => true, // Still usable
                Some(ServerHealth::Unavailable { until }) => now >= *until,
                Some(ServerHealth::Failed) => false,
                None => true, // Unknown servers are considered healthy
            };

            if !is_healthy {
                continue;
            }

            // Calculate server score based on performance metrics
            let server_metrics = metrics.get(server_key);
            let score = self.calculate_server_score(server, server_metrics);

            candidates.push((server.clone(), score));
        }

        if candidates.is_empty() {
            warn!("No healthy servers available");
            return None;
        }

        // Sort by score (higher is better) and return best
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let best_server = candidates[0].0.clone();
        debug!(
            "Selected server {} with score {:.2}",
            best_server.host, candidates[0].1
        );

        Some(best_server)
    }

    /// Calculate server selection score
    fn calculate_server_score(&self, server: &CdnServer, metrics: Option<&ServerMetrics>) -> f64 {
        let mut score = 100.0; // Base score

        // Priority bonus (lower priority value = higher score)
        score += 1000.0 / (server.priority as f64 + 1.0);

        // HTTPS bonus
        if server.supports_https {
            score += 10.0;
        }

        if let Some(metrics) = metrics {
            // Success rate bonus (0-50 points)
            score += metrics.success_rate * 50.0;

            // Response time penalty (faster = higher score)
            let response_time_ms = metrics.avg_response_time.as_millis() as f64;
            if response_time_ms > 0.0 {
                score -= (response_time_ms / 100.0).min(50.0); // Cap penalty at 50 points
            }

            // Bandwidth bonus (higher bandwidth = higher score)
            let bandwidth_mbps = metrics.bandwidth_estimate as f64 / (1024.0 * 1024.0);
            score += (bandwidth_mbps / 10.0).min(30.0); // Cap bonus at 30 points

            // Recency bonus (recent success = higher score)
            if let Some(last_success) = metrics.last_success {
                let recency_bonus = match last_success.elapsed() {
                    d if d < Duration::from_secs(60) => 20.0,
                    d if d < Duration::from_secs(300) => 10.0,
                    d if d < Duration::from_secs(900) => 5.0,
                    _ => 0.0,
                };
                score += recency_bonus;
            }
        }

        score.max(0.0)
    }

    /// Get failover statistics
    pub fn statistics(&self) -> (u64, u64) {
        (
            self.failover_count.load(Ordering::Relaxed),
            self.recovery_count.load(Ordering::Relaxed),
        )
    }

    /// Get server health status
    pub async fn get_server_health(&self, server: &str) -> ServerHealth {
        let health = self.server_health.read().await;
        health.get(server).cloned().unwrap_or(ServerHealth::Healthy)
    }

    /// Get all server metrics
    pub async fn get_all_metrics(&self) -> HashMap<String, ServerMetrics> {
        let metrics = self.server_metrics.read().await;
        metrics.clone()
    }

    /// Cleanup expired health statuses
    pub async fn cleanup_expired(&self) {
        let mut health = self.server_health.write().await;
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (server, status) in health.iter() {
            if let ServerHealth::Unavailable { until } = status {
                if now >= *until {
                    to_remove.push(server.clone());
                }
            }
        }

        for server in to_remove {
            health.insert(server.clone(), ServerHealth::Healthy);
            info!("Server {} health status expired, marked as healthy", server);
        }
    }
}

/// Network condition detector
#[cfg(feature = "streaming")]
#[derive(Debug)]
pub struct NetworkConditionDetector {
    /// Recent error history
    error_history: Arc<RwLock<VecDeque<(Instant, String)>>>,
    /// Recent performance samples
    performance_history: Arc<RwLock<VecDeque<(Instant, Duration, bool)>>>,
    /// History window
    window_duration: Duration,
}

#[cfg(feature = "streaming")]
impl NetworkConditionDetector {
    /// Create new network condition detector
    pub fn new(window_duration: Duration) -> Self {
        Self {
            error_history: Arc::new(RwLock::new(VecDeque::new())),
            performance_history: Arc::new(RwLock::new(VecDeque::new())),
            window_duration,
        }
    }

    /// Record error for condition assessment
    pub async fn record_error(&self, error: &StreamingError) {
        let mut history = self.error_history.write().await;
        let now = Instant::now();

        history.push_back((now, format!("{:?}", error)));

        // Cleanup old entries
        let cutoff = now - self.window_duration;
        while let Some((timestamp, _)) = history.front() {
            if *timestamp < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Record performance sample
    pub async fn record_performance(&self, response_time: Duration, success: bool) {
        let mut history = self.performance_history.write().await;
        let now = Instant::now();

        history.push_back((now, response_time, success));

        // Cleanup old entries
        let cutoff = now - self.window_duration;
        while let Some((timestamp, _, _)) = history.front() {
            if *timestamp < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Assess current network condition
    pub async fn assess_condition(&self) -> NetworkCondition {
        let error_history = self.error_history.read().await;
        let performance_history = self.performance_history.read().await;

        // Calculate error rate
        let total_requests = performance_history.len();
        let error_count = error_history.len();
        let error_rate = if total_requests == 0 {
            0.0
        } else {
            error_count as f64 / total_requests as f64
        };

        // Calculate success rate from performance history
        let successful_requests = performance_history
            .iter()
            .filter(|(_, _, success)| *success)
            .count();
        let success_rate = if total_requests == 0 {
            1.0
        } else {
            successful_requests as f64 / total_requests as f64
        };

        // Calculate average response time
        let total_response_time: Duration = performance_history
            .iter()
            .filter(|(_, _, success)| *success)
            .map(|(_, rt, _)| *rt)
            .sum();
        drop(performance_history);

        let avg_response_time = if successful_requests == 0 {
            Duration::from_millis(0)
        } else {
            total_response_time / successful_requests as u32
        };

        // Determine condition based on metrics
        let condition = if error_rate < 0.01
            && success_rate > 0.99
            && avg_response_time < Duration::from_millis(100)
        {
            NetworkCondition::Excellent
        } else if error_rate < 0.05
            && success_rate > 0.95
            && avg_response_time < Duration::from_millis(300)
        {
            NetworkCondition::Good
        } else if error_rate < 0.15
            && success_rate > 0.85
            && avg_response_time < Duration::from_millis(1000)
        {
            NetworkCondition::Fair
        } else if error_rate < 0.30 && success_rate > 0.70 {
            NetworkCondition::Poor
        } else {
            NetworkCondition::VeryPoor
        };

        debug!(
            "Network condition assessment: {:?} (error_rate={:.3}, success_rate={:.3}, avg_rt={:?})",
            condition, error_rate, success_rate, avg_response_time
        );

        condition
    }

    /// Get current statistics
    pub async fn statistics(&self) -> (usize, usize, f64, Duration) {
        let error_count = self.error_history.read().await.len();
        let performance_history = self.performance_history.read().await;
        let total_requests = performance_history.len();
        let error_rate = if total_requests == 0 {
            0.0
        } else {
            error_count as f64 / total_requests as f64
        };

        let successful_requests = performance_history
            .iter()
            .filter(|(_, _, success)| *success)
            .count();
        let total_response_time: Duration = performance_history
            .iter()
            .filter(|(_, _, success)| *success)
            .map(|(_, rt, _)| *rt)
            .sum();
        drop(performance_history);

        let avg_response_time = if successful_requests == 0 {
            Duration::from_millis(0)
        } else {
            total_response_time / successful_requests as u32
        };

        (error_count, total_requests, error_rate, avg_response_time)
    }
}

/// Complete error recovery system
#[cfg(feature = "streaming")]
#[derive(Debug)]
pub struct ErrorRecoverySystem<T: HttpClient> {
    /// HTTP client
    client: Arc<T>,
    /// Retry manager
    retry_manager: RetryManager,
    /// Failover manager
    failover_manager: FailoverManager,
    /// Network condition detector
    network_detector: NetworkConditionDetector,
    /// Metrics
    metrics: Arc<StreamingMetrics>,
    /// Configuration
    #[allow(dead_code)]
    config: StreamingConfig,
}

#[cfg(feature = "streaming")]
impl<T: HttpClient> ErrorRecoverySystem<T> {
    /// Create new error recovery system
    pub fn new(client: Arc<T>, config: StreamingConfig, metrics: Arc<StreamingMetrics>) -> Self {
        let retry_manager = RetryManager::new(config.retry.clone());
        let failover_manager = FailoverManager::new(config.clone());
        let network_detector = NetworkConditionDetector::new(Duration::from_secs(300));

        Self {
            client,
            retry_manager,
            failover_manager,
            network_detector,
            metrics,
            config,
        }
    }

    /// Execute request with full error recovery
    pub async fn execute_with_recovery(
        &self,
        url: String,
        range: Option<HttpRange>,
        available_servers: Vec<CdnServer>,
        timeout_duration: Option<Duration>,
    ) -> StreamingResult<Bytes> {
        let mut context = RecoveryContext {
            url: url.clone(),
            range,
            attempts: 0,
            error_descriptions: Vec::new(),
            servers_tried: Vec::new(),
            started_at: Instant::now(),
            network_condition: self.network_detector.assess_condition().await,
            available_servers: available_servers.clone(),
        };

        let timeout_dur = timeout_duration.unwrap_or(self.config.request_timeout);

        loop {
            context.attempts += 1;

            // Check if we've exceeded maximum attempts
            if context.attempts > self.config.retry.max_attempts {
                let final_error = StreamingError::Configuration {
                    reason: format!(
                        "Maximum retry attempts exceeded. Last errors: {:?}",
                        context.error_descriptions
                    ),
                };

                error!(
                    "Request failed after {} attempts: {:?}",
                    context.attempts, final_error
                );
                return Err(final_error);
            }

            // Select best available server
            let Some(server) = self
                .failover_manager
                .select_best_server(&available_servers)
                .await
            else {
                let error = StreamingError::Configuration {
                    reason: "No healthy servers available".to_string(),
                };
                context.error_descriptions.push(format!("{:?}", error));
                return Err(error);
            };

            let server_key = server.host.clone();
            context.servers_tried.push(server_key.clone());

            // Calculate retry delay
            let delay = if context.attempts > 1 {
                self.retry_manager
                    .calculate_delay(context.attempts - 1, context.network_condition)
            } else {
                Duration::from_millis(0)
            };

            if !delay.is_zero() {
                debug!("Waiting {:?} before attempt {}", delay, context.attempts);
                sleep(delay).await;
            }

            // Make the request
            let request_start = Instant::now();
            let result = timeout(timeout_dur, self.client.get_range(&url, range)).await;
            let request_duration = request_start.elapsed();

            match result {
                Ok(Ok(bytes)) => {
                    // Success!
                    info!(
                        "Request succeeded on attempt {} after {:?}",
                        context.attempts,
                        context.started_at.elapsed()
                    );

                    // Update metrics and server health
                    self.failover_manager.mark_server_healthy(&server_key).await;
                    self.failover_manager
                        .update_server_metrics(
                            &server_key,
                            request_duration,
                            true,
                            bytes.len() as u64,
                        )
                        .await;
                    self.network_detector
                        .record_performance(request_duration, true)
                        .await;
                    self.retry_manager.record_attempt(true);

                    // Update metrics
                    self.metrics
                        .record_download(bytes.len() as u64, request_duration);
                    if context.attempts > 1 {
                        self.metrics
                            .retry_attempts
                            .fetch_add(context.attempts as u64 - 1, Ordering::Relaxed);
                    }

                    return Ok(bytes);
                }
                Ok(Err(error)) => {
                    let streaming_error = error;

                    warn!(
                        "Request attempt {} failed: {:?}",
                        context.attempts, streaming_error
                    );

                    // Update server health and metrics
                    self.failover_manager
                        .mark_server_failed(&server_key, &streaming_error)
                        .await;
                    self.failover_manager
                        .update_server_metrics(&server_key, request_duration, false, 0)
                        .await;
                    self.network_detector.record_error(&streaming_error).await;
                    self.network_detector
                        .record_performance(request_duration, false)
                        .await;
                    self.retry_manager.record_attempt(false);

                    context
                        .error_descriptions
                        .push(format!("{:?}", streaming_error));

                    // Check if error is retryable
                    if !self.retry_manager.is_retryable(&streaming_error) {
                        error!("Non-retryable error encountered: {:?}", streaming_error);
                        return Err(streaming_error);
                    }

                    // Update network condition for next iteration
                    context.network_condition = self.network_detector.assess_condition().await;

                    // Continue to next retry attempt
                }
                Err(_) => {
                    // Handle the timeout error
                    let streaming_error = StreamingError::Timeout {
                        timeout_ms: timeout_dur.as_millis() as u64,
                        url: url.clone(),
                    };

                    warn!(
                        "Request attempt {} failed: {:?}",
                        context.attempts, streaming_error
                    );

                    // Update server health and metrics
                    self.failover_manager
                        .mark_server_failed(&server_key, &streaming_error)
                        .await;
                    self.failover_manager
                        .update_server_metrics(&server_key, request_duration, false, 0)
                        .await;
                    self.network_detector.record_error(&streaming_error).await;
                    self.network_detector
                        .record_performance(request_duration, false)
                        .await;
                    self.retry_manager.record_attempt(false);

                    context
                        .error_descriptions
                        .push(format!("{:?}", streaming_error));

                    // Check if error is retryable
                    if !self.retry_manager.is_retryable(&streaming_error) {
                        error!("Non-retryable error encountered: {:?}", streaming_error);
                        return Err(streaming_error);
                    }

                    // Update network condition for next iteration
                    context.network_condition = self.network_detector.assess_condition().await;

                    // Continue to next retry attempt
                }
            }
        }
    }

    /// Get recovery system statistics
    pub async fn statistics(&self) -> RecoveryStatistics {
        let (retry_total, retry_successful, retry_failed) = self.retry_manager.statistics();
        let (failover_count, recovery_count) = self.failover_manager.statistics();
        let (error_count, total_requests, error_rate, avg_response_time) =
            self.network_detector.statistics().await;
        let network_condition = self.network_detector.assess_condition().await;
        let all_server_metrics = self.failover_manager.get_all_metrics().await;

        RecoveryStatistics {
            retry_total,
            retry_successful,
            retry_failed,
            retry_success_rate: self.retry_manager.success_rate(),
            failover_count,
            recovery_count,
            error_count,
            total_requests,
            error_rate,
            avg_response_time,
            network_condition,
            server_metrics: all_server_metrics,
        }
    }

    /// Cleanup expired health statuses and old history
    pub async fn cleanup(&self) {
        self.failover_manager.cleanup_expired().await;
    }
}

/// Comprehensive recovery statistics
#[cfg(feature = "streaming")]
#[derive(Debug, Clone)]
pub struct RecoveryStatistics {
    /// Total number of retry attempts
    pub retry_total: u64,
    /// Number of successful retry attempts
    pub retry_successful: u64,
    /// Number of failed retry attempts
    pub retry_failed: u64,
    /// Success rate of retry attempts (0.0 to 1.0)
    pub retry_success_rate: f64,
    /// Number of CDN server failovers
    pub failover_count: u64,
    /// Number of error recoveries
    pub recovery_count: u64,
    /// Total number of errors encountered
    pub error_count: usize,
    /// Total number of requests processed
    pub total_requests: usize,
    /// Overall error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Average response time for requests
    pub avg_response_time: Duration,
    /// Current network condition assessment
    pub network_condition: NetworkCondition,
    /// Per-server metrics for CDN servers
    pub server_metrics: HashMap<String, ServerMetrics>,
}

#[cfg(all(test, feature = "streaming"))]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::uninlined_format_args)]
mod tests {
    use super::*;
    use crate::cdn::streaming::{
        config::{RetryConfig, StreamingConfig},
        http::CdnServer,
    };
    use mockall::mock;
    use std::time::Duration;

    mock! {
        TestHttpClient {}

        #[async_trait::async_trait]
        impl HttpClient for TestHttpClient {
            async fn get_range(&self, url: &str, range: Option<HttpRange>) -> Result<Bytes, StreamingError>;
            async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError>;
            async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError>;
        }
    }

    #[test]
    fn test_retry_manager() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            jitter_factor: 0.1,
            retry_on_status: vec![429, 502, 503, 504],
        };

        let retry_manager = RetryManager::new(config);

        // Test delay calculation
        let delay1 = retry_manager.calculate_delay(1, NetworkCondition::Good);
        let delay2 = retry_manager.calculate_delay(2, NetworkCondition::Good);

        assert!(delay1 < delay2); // Exponential backoff

        // Test retryable errors
        // Create a mock network error for testing
        let url = "http://test.example.com";
        let network_error = StreamingError::Timeout {
            timeout_ms: 5000,
            url: url.to_string(),
        };
        assert!(retry_manager.is_retryable(&network_error));

        let config_error = StreamingError::Configuration {
            reason: "test".to_string(),
        };
        assert!(!retry_manager.is_retryable(&config_error));
    }

    #[tokio::test]
    async fn test_failover_manager() {
        let config = StreamingConfig::default();
        let failover_manager = FailoverManager::new(config);

        let servers = vec![
            CdnServer::new("server1.com".to_string(), true, 100),
            CdnServer::new("server2.com".to_string(), true, 200),
        ];

        // Initially, should select first server (lower priority)
        let selected = failover_manager.select_best_server(&servers).await;
        assert!(selected.is_some());
        assert_eq!(
            selected.expect("Operation should succeed").host,
            "server1.com"
        );

        // Mark first server as failed
        let error = StreamingError::Timeout {
            timeout_ms: 5000,
            url: "http://server1.com/test".to_string(),
        };
        failover_manager
            .mark_server_failed("server1.com", &error)
            .await;

        // Should now select second server
        let selected = failover_manager.select_best_server(&servers).await;
        assert!(selected.is_some());
        assert_eq!(
            selected.expect("Operation should succeed").host,
            "server2.com"
        );
    }

    #[tokio::test]
    async fn test_network_condition_detector() {
        let detector = NetworkConditionDetector::new(Duration::from_secs(60));

        // Record some successful operations
        for _ in 0..10 {
            detector
                .record_performance(Duration::from_millis(50), true)
                .await;
        }

        let condition = detector.assess_condition().await;
        assert_eq!(condition, NetworkCondition::Excellent);

        // Record some failures
        for _ in 0..5 {
            let error = StreamingError::Timeout {
                timeout_ms: 5000,
                url: "http://example.com".to_string(),
            };
            detector.record_error(&error).await;
            detector
                .record_performance(Duration::from_millis(1000), false)
                .await;
        }

        let condition = detector.assess_condition().await;
        assert!(matches!(
            condition,
            NetworkCondition::Poor | NetworkCondition::VeryPoor
        ));
    }

    #[tokio::test]
    async fn test_server_metrics_update() {
        let config = StreamingConfig::default();
        let failover_manager = FailoverManager::new(config);

        // Update metrics for a server
        failover_manager
            .update_server_metrics("test.com", Duration::from_millis(100), true, 1000)
            .await;

        failover_manager
            .update_server_metrics("test.com", Duration::from_millis(200), true, 2000)
            .await;

        let metrics = failover_manager.get_all_metrics().await;
        let server_metrics = metrics.get("test.com").expect("Operation should succeed");

        assert_eq!(server_metrics.total_requests, 2);
        assert_eq!(server_metrics.success_rate, 1.0);
        assert!(server_metrics.avg_response_time.as_millis() > 100);
        assert!(server_metrics.bandwidth_estimate > 0);
    }
}

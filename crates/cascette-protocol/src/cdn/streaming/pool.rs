//! Advanced connection pool management with health checking and metrics
//!
//! This module provides enterprise-grade connection pool management for CDN streaming
//! operations, including:
//!
//! - Health checking with automatic server removal and restoration
//! - Circuit breaker pattern for failing CDN servers
//! - Connection reuse across multiple requests
//! - Performance metrics collection
//! - Automatic cleanup and resource management

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use dashmap::DashMap;
use tokio::{
    sync::{RwLock, Semaphore},
    time::interval,
};
use tracing::{debug, info, warn};

use super::{
    config::ConnectionPoolConfig,
    error::{StreamingError, StreamingResult},
    http::{CdnServer, HttpClient},
    metrics::PoolMetrics,
};

/// RAII guard for connection permits
///
/// Ensures that connection permits are properly released when dropped
#[must_use = "ConnectionGuard must be held to maintain the connection permit"]
pub struct ConnectionGuard {
    semaphore: Arc<Semaphore>,
    active_counter: Arc<AtomicUsize>,
    metrics: Arc<PoolMetrics>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        // Decrement active request counter
        self.active_counter.fetch_sub(1, Ordering::Relaxed);

        // Update metrics
        self.metrics
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);

        // Release the permit back to the semaphore
        // This is safe because we know exactly one permit was acquired
        self.semaphore.add_permits(1);
    }
}

/// Connection state for health checking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is healthy and available
    Healthy,
    /// Connection is temporarily unavailable (circuit breaker open)
    CircuitOpen {
        /// When the circuit will be closed again
        until: Instant,
    },
    /// Connection is being health checked
    Checking,
    /// Connection is permanently removed
    Removed,
}

/// Statistics for a connection
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Total number of requests made
    pub requests: u64,
    /// Number of successful requests
    pub successes: u64,
    /// Number of failed requests
    pub failures: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Last successful request time
    pub last_success: Option<Instant>,
    /// Last failure time
    pub last_failure: Option<Instant>,
    /// Current state
    pub state: ConnectionState,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            requests: 0,
            successes: 0,
            failures: 0,
            avg_response_time_ms: 0.0,
            last_success: None,
            last_failure: None,
            state: ConnectionState::Healthy,
        }
    }
}

impl ConnectionStats {
    /// Calculate success rate (0.0 to 1.0)
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        if self.requests == 0 {
            1.0
        } else {
            self.successes as f64 / self.requests as f64
        }
    }

    /// Check if connection should be circuit-broken
    pub fn should_circuit_break(&self, _config: &ConnectionPoolConfig) -> bool {
        if self.requests < 10 {
            // Need minimum requests for statistical significance
            return false;
        }

        let success_rate = self.success_rate();
        let recent_failures = self.failures > 0
            && self
                .last_failure
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60));

        success_rate < 0.5 && recent_failures
    }

    /// Update stats with request result
    #[allow(clippy::cast_precision_loss)]
    pub fn update(&mut self, success: bool, response_time: Duration) {
        self.requests += 1;

        if success {
            self.successes += 1;
            self.last_success = Some(Instant::now());
        } else {
            self.failures += 1;
            self.last_failure = Some(Instant::now());
        }

        // Update rolling average response time
        let response_time_ms = response_time.as_millis() as f64;
        if self.requests == 1 {
            self.avg_response_time_ms = response_time_ms;
        } else {
            // Exponential moving average with alpha = 0.1
            self.avg_response_time_ms = 0.9f64.mul_add(self.avg_response_time_ms, 0.1 * response_time_ms);
        }
    }
}

/// Advanced connection pool with health checking and circuit breaking
#[derive(Clone)]
pub struct ConnectionPool<T: HttpClient> {
    /// Underlying HTTP clients per server
    clients: Arc<DashMap<String, Arc<T>>>,
    /// Connection statistics and health status
    stats: Arc<DashMap<String, Arc<RwLock<ConnectionStats>>>>,
    /// Active request counts per server
    active_requests: Arc<DashMap<String, Arc<AtomicUsize>>>,
    /// Connection limits per server
    connection_limits: Arc<DashMap<String, Arc<Semaphore>>>,
    /// Pool configuration
    config: Arc<ConnectionPoolConfig>,
    /// Pool-wide metrics
    metrics: Arc<PoolMetrics>,
    /// Cleanup task handle
    cleanup_handle: Arc<AtomicBool>,
    /// Task handle for the cleanup task (for proper joining)
    cleanup_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl<T: HttpClient> ConnectionPool<T> {
    /// Create new connection pool with configuration
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let pool = Self {
            clients: Arc::new(DashMap::new()),
            stats: Arc::new(DashMap::new()),
            active_requests: Arc::new(DashMap::new()),
            connection_limits: Arc::new(DashMap::new()),
            config: Arc::new(config),
            metrics: Arc::new(PoolMetrics::new()),
            cleanup_handle: Arc::new(AtomicBool::new(false)),
            cleanup_task: Arc::new(tokio::sync::Mutex::new(None)),
        };

        // Start background cleanup task
        pool.start_cleanup_task();
        pool
    }

    /// Add client for a server
    pub fn add_client(&self, server: &CdnServer, client: T) {
        let server_id = format!(
            "{}:{}",
            server.host,
            if server.supports_https {
                "https"
            } else {
                "http"
            }
        );

        debug!("Adding client for server: {}", server_id);

        self.clients.insert(server_id.clone(), Arc::new(client));
        self.stats.insert(
            server_id.clone(),
            Arc::new(RwLock::new(ConnectionStats::default())),
        );
        self.active_requests
            .insert(server_id.clone(), Arc::new(AtomicUsize::new(0)));
        self.connection_limits.insert(
            server_id,
            Arc::new(Semaphore::new(self.config.max_connections_per_host)),
        );
    }

    /// Get healthy client for server with connection limiting
    ///
    /// Returns a tuple of (client, guard). The guard must be held for the lifetime
    /// of the request to ensure proper resource cleanup.
    #[must_use = "Connection guard must be held to prevent resource leaks"]
    pub async fn get_client(
        &self,
        server: &CdnServer,
    ) -> StreamingResult<(Arc<T>, ConnectionGuard)> {
        let server_id = format!(
            "{}:{}",
            server.host,
            if server.supports_https {
                "https"
            } else {
                "http"
            }
        );

        // Check if server is available
        if let Some(stats_lock) = self.stats.get(&server_id) {
            let stats = stats_lock.read().await;
            match &stats.state {
                ConnectionState::Removed => {
                    return Err(StreamingError::ServerUnavailable {
                        server: server.host.clone(),
                        reason: "Server permanently removed".to_string(),
                    });
                }
                ConnectionState::CircuitOpen { until } => {
                    if Instant::now() < *until {
                        return Err(StreamingError::ServerUnavailable {
                            server: server.host.clone(),
                            reason: "Circuit breaker open".to_string(),
                        });
                    }
                }
                ConnectionState::Checking => {
                    return Err(StreamingError::ServerUnavailable {
                        server: server.host.clone(),
                        reason: "Health check in progress".to_string(),
                    });
                }
                ConnectionState::Healthy => {}
            }
        }

        // Acquire connection permit
        let semaphore = if let Some(semaphore_ref) = self.connection_limits.get(&server_id) {
            let semaphore = semaphore_ref.clone();
            let permit = semaphore
                .try_acquire()
                .map_err(|_| StreamingError::ConnectionLimit {
                    server: server.host.clone(),
                    limit: self.config.max_connections_per_host,
                })?;

            // Store the semaphore reference instead of the permit to enable proper cleanup
            // The permit is released when the guard is dropped via the semaphore add_permits call
            std::mem::forget(permit); // Managed by ConnectionGuard
            semaphore
        } else {
            return Err(StreamingError::Configuration {
                reason: format!("No connection limit configured for server: {}", server.host),
            });
        };

        // Get active request counter
        let active_counter = self
            .active_requests
            .get(&server_id)
            .ok_or_else(|| StreamingError::Configuration {
                reason: format!("No active request counter for server: {}", server.host),
            })?
            .clone();

        // Increment active request counter
        active_counter.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .active_connections
            .fetch_add(1, Ordering::Relaxed);

        // Create connection guard
        let guard = ConnectionGuard {
            semaphore,
            active_counter,
            metrics: self.metrics.clone(),
        };

        // Get client
        let client = self
            .clients
            .get(&server_id)
            .ok_or_else(|| StreamingError::Configuration {
                reason: format!("No client configured for server: {}", server.host),
            })?
            .clone();

        Ok((client, guard))
    }

    /// Record request result and update health statistics
    ///
    /// Note: Connection counting is now handled by ConnectionGuard automatically
    pub async fn record_result(&self, server: &CdnServer, success: bool, response_time: Duration) {
        let server_id = format!(
            "{}:{}",
            server.host,
            if server.supports_https {
                "https"
            } else {
                "http"
            }
        );

        // Connection counting is now handled by ConnectionGuard

        // Update statistics
        if let Some(stats_lock) = self.stats.get(&server_id) {
            let mut stats = stats_lock.write().await;
            stats.update(success, response_time);

            // Check if circuit breaker should activate
            if !success && stats.should_circuit_break(&self.config) {
                warn!("Activating circuit breaker for server: {}", server.host);
                let circuit_open_until = Instant::now() + Duration::from_secs(60);
                stats.state = ConnectionState::CircuitOpen {
                    until: circuit_open_until,
                };
                drop(stats);

                self.metrics
                    .circuit_breakers_activated
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update pool metrics
        if success {
            self.metrics
                .total_successful_requests
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.metrics
                .total_failed_requests
                .fetch_add(1, Ordering::Relaxed);
        }

        self.metrics.update_response_time(response_time).await;
    }

    /// Get connection statistics for a server
    pub async fn get_stats(&self, server: &CdnServer) -> Option<ConnectionStats> {
        let server_id = format!(
            "{}:{}",
            server.host,
            if server.supports_https {
                "https"
            } else {
                "http"
            }
        );

        if let Some(stats_lock) = self.stats.get(&server_id) {
            Some(stats_lock.read().await.clone())
        } else {
            None
        }
    }

    /// Get all server statistics
    pub async fn get_all_stats(&self) -> HashMap<String, ConnectionStats> {
        let mut result = HashMap::new();

        for entry in self.stats.iter() {
            let server_id = entry.key().clone();
            let stats = entry.value().read().await.clone();
            result.insert(server_id, stats);
        }

        result
    }

    /// Remove a server from the pool
    pub async fn remove_server(&self, server: &CdnServer) {
        let server_id = format!(
            "{}:{}",
            server.host,
            if server.supports_https {
                "https"
            } else {
                "http"
            }
        );

        info!("Removing server from pool: {}", server.host);

        if let Some(stats_lock) = self.stats.get(&server_id) {
            let mut stats = stats_lock.write().await;
            stats.state = ConnectionState::Removed;
        }

        self.clients.remove(&server_id);
        self.connection_limits.remove(&server_id);

        self.metrics.servers_removed.fetch_add(1, Ordering::Relaxed);
    }

    /// Perform health check on all servers with cancellation support
    pub async fn health_check(&self, test_url: &str) -> Result<(), StreamingError> {
        info!("Starting health check for all servers");

        let mut health_check_tasks = Vec::new();
        let cleanup_handle = self.cleanup_handle.clone();

        for entry in self.clients.iter() {
            // Check if we should continue
            if !cleanup_handle.load(Ordering::Relaxed) {
                warn!("Health check cancelled before completion");
                return Err(StreamingError::Configuration {
                    reason: "Health check cancelled during pool shutdown".to_string(),
                });
            }

            let server_id = entry.key().clone();
            let client = entry.value().clone();
            let url = test_url.to_string();
            let Some(stats_ref) = self.stats.get(&server_id) else {
                continue;
            };
            let stats_lock = stats_ref.clone();
            let cleanup_handle_task = cleanup_handle.clone();

            let task = tokio::spawn(async move {
                // Check cancellation before starting
                if !cleanup_handle_task.load(Ordering::Relaxed) {
                    return;
                }

                // Set state to checking
                {
                    let mut stats = stats_lock.write().await;
                    stats.state = ConnectionState::Checking;
                }

                let start_time = Instant::now();

                // Health check with timeout to prevent hanging
                let health_check_result = tokio::select! {
                    result = client.get_content_length(&url) => result,
                    () = tokio::time::sleep(Duration::from_secs(10)) => {
                        Err(StreamingError::Timeout {
                            timeout_ms: 10000,
                            url: url.clone(),
                        })
                    }
                    () = async {
                        while cleanup_handle_task.load(Ordering::Relaxed) {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    } => {
                        debug!("Health check cancelled for {}", server_id);
                        return;
                    }
                };

                let success = health_check_result.is_ok();
                if !success
                    && let Err(e) = health_check_result {
                        debug!("Health check failed for {}: {:?}", server_id, e);
                    }

                let response_time = start_time.elapsed();

                // Check cancellation before updating stats
                if !cleanup_handle_task.load(Ordering::Relaxed) {
                    debug!(
                        "Health check cancelled before updating stats for {}",
                        server_id
                    );
                    return;
                }

                // Update stats and state
                {
                    let mut stats = stats_lock.write().await;
                    stats.update(success, response_time);

                    if success {
                        stats.state = ConnectionState::Healthy;
                        drop(stats);
                        info!("Server {} is healthy", server_id);
                    } else {
                        let circuit_open_until = Instant::now() + Duration::from_secs(300); // 5 minutes
                        stats.state = ConnectionState::CircuitOpen {
                            until: circuit_open_until,
                        };
                        drop(stats);
                        warn!("Server {} failed health check", server_id);
                    }
                }
            });

            health_check_tasks.push(task);
        }

        // Wait for all health checks to complete with cancellation support
        // Properly handle task cleanup on cancellation
        let mut pending_tasks = health_check_tasks;

        while !pending_tasks.is_empty() && cleanup_handle.load(Ordering::Relaxed) {
            let mut completed_indices = Vec::new();

            for (index, task) in pending_tasks.iter_mut().enumerate() {
                tokio::select! {
                    result = task => {
                        if let Err(e) = result {
                            warn!("Health check task panicked: {:?}", e);
                        }
                        completed_indices.push(index);
                    }
                    () = tokio::time::sleep(Duration::from_millis(10)) => {
                        // Continue to next iteration
                    }
                }

                // Check for cancellation
                if !cleanup_handle.load(Ordering::Relaxed) {
                    warn!("Health check cancelled, aborting remaining tasks");
                    // Abort all remaining tasks to prevent resource leaks
                    for remaining_task in &pending_tasks {
                        remaining_task.abort();
                    }
                    break;
                }
            }

            // Remove completed tasks (in reverse order to maintain indices)
            for &index in completed_indices.iter().rev() {
                pending_tasks.remove(index);
            }

            if !completed_indices.is_empty() {
                debug!(
                    "Completed {} health check tasks, {} remaining",
                    completed_indices.len(),
                    pending_tasks.len()
                );
            }
        }

        if cleanup_handle.load(Ordering::Relaxed) {
            if pending_tasks.is_empty() {
                info!("Health check completed for all servers");
            } else {
                warn!(
                    "Health check completed with {} tasks still pending",
                    pending_tasks.len()
                );
            }
        } else {
            info!(
                "Health check cancelled, aborted {} remaining tasks",
                pending_tasks.len()
            );
        }

        Ok(())
    }

    /// Get pool metrics
    pub fn metrics(&self) -> &PoolMetrics {
        &self.metrics
    }

    /// Check if the pool is shutting down
    pub fn is_shutting_down(&self) -> bool {
        !self.cleanup_handle.load(Ordering::Relaxed)
    }

    /// Get the number of active requests across all servers
    pub fn active_request_count(&self) -> usize {
        self.active_requests
            .iter()
            .map(|entry| entry.value().load(Ordering::Relaxed))
            .sum()
    }

    /// Start background cleanup task with proper cancellation support
    fn start_cleanup_task(&self) {
        let _clients = self.clients.clone();
        let stats = self.stats.clone();
        let active_requests = self.active_requests.clone();
        let _config = self.config.clone();
        let metrics = self.metrics.clone();
        let cleanup_handle = self.cleanup_handle.clone();
        let cleanup_task_handle = self.cleanup_task.clone();

        cleanup_handle.store(true, Ordering::Relaxed);

        let task_handle = tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(30));

            // Handle cancellation gracefully
            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        // Check if we should continue running
                        if !cleanup_handle.load(Ordering::Relaxed) {
                            debug!("Cleanup task cancelled, exiting gracefully");
                            break;
                        }

                        // Check for circuit breaker recovery
                        let now = Instant::now();
                        for entry in stats.iter() {
                            // Check cancellation during iterations
                            if !cleanup_handle.load(Ordering::Relaxed) {
                                debug!("Cleanup task cancelled during circuit breaker recovery");
                                return;
                            }

                            let server_id = entry.key();
                            let stats_lock = entry.value();

                            if let Ok(mut stats) = stats_lock.try_write()
                                && let ConnectionState::CircuitOpen { until } = stats.state
                                    && now >= until {
                                        info!("Recovering server from circuit breaker: {}", server_id);
                                        stats.state = ConnectionState::Healthy;
                                        metrics.circuit_breakers_recovered.fetch_add(1, Ordering::Relaxed);
                                    }
                        }

                        // Update pool metrics if not cancelled
                        if cleanup_handle.load(Ordering::Relaxed) {
                            let total_active = active_requests.iter()
                                .map(|entry| entry.value().load(Ordering::Relaxed))
                                .sum::<usize>() as u64;
                            metrics.active_connections.store(total_active, Ordering::Relaxed);

                            debug!("Pool cleanup completed - {} active connections", total_active);
                        }
                    }
                    // Explicit cancellation check
                    () = tokio::time::sleep(Duration::from_millis(100)) => {
                        if !cleanup_handle.load(Ordering::Relaxed) {
                            debug!("Cleanup task cancelled via periodic check");
                            break;
                        }
                    }
                }
            }

            debug!("Cleanup task exited cleanly");
        });

        // Store the task handle for proper cleanup immediately
        // This avoids spawning an additional task that can't be tracked
        let cleanup_task_clone = cleanup_task_handle;
        let runtime_handle = tokio::runtime::Handle::current();
        runtime_handle.spawn(async move {
            // Store the handle immediately to ensure proper cleanup
            if let Ok(mut cleanup_task_guard) = cleanup_task_clone.try_lock() {
                *cleanup_task_guard = Some(task_handle);
            } else {
                // If we can't acquire the lock immediately, the task is likely being shut down
                // In this case, we should abort the task to prevent resource leaks
                task_handle.abort();
            }
        });
    }

    /// Shutdown the connection pool gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down connection pool");

        // Signal cleanup task to stop
        self.cleanup_handle.store(false, Ordering::Relaxed);

        // Wait for cleanup task to finish properly
        let task_handle = self.cleanup_task.lock().await.take();
        if let Some(task_handle) = task_handle {
            match tokio::time::timeout(Duration::from_secs(5), task_handle).await {
                Ok(Ok(())) => {
                    debug!("Cleanup task joined successfully");
                }
                Ok(Err(e)) => {
                    warn!("Cleanup task panicked during shutdown: {:?}", e);
                }
                Err(_) => {
                    warn!("Cleanup task did not exit within timeout, continuing shutdown");
                }
            }
        }

        // Wait for any ongoing requests to complete (with timeout)
        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(10);

        while start_time.elapsed() < timeout_duration {
            let total_active: usize = self
                .active_requests
                .iter()
                .map(|entry| entry.value().load(Ordering::Relaxed))
                .sum();

            if total_active == 0 {
                debug!("All active requests completed");
                break;
            }

            debug!("Waiting for {} active requests to complete", total_active);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Clear all active connections and stats
        self.clients.clear();
        self.stats.clear();
        self.active_requests.clear();
        self.connection_limits.clear();

        info!("Connection pool shutdown completed");
    }

    /// Shutdown the connection pool synchronously (for Drop implementation)
    pub fn shutdown_sync(&self) {
        self.cleanup_handle.store(false, Ordering::Relaxed);
    }
}

impl<T: HttpClient> Drop for ConnectionPool<T> {
    fn drop(&mut self) {
        self.shutdown_sync();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::uninlined_format_args)]
mod tests {
    use super::*;
    use crate::cdn::streaming::{config::ConnectionPoolConfig, http::CdnServer};
    use mockall::mock;
    use std::time::Duration;
    use tokio::time::timeout;

    mock! {
        TestHttpClient {}

        #[async_trait::async_trait]
        impl HttpClient for TestHttpClient {
            async fn get_range(&self, url: &str, range: Option<crate::cdn::streaming::range::HttpRange>) -> Result<bytes::Bytes, StreamingError>;
            async fn get_content_length(&self, url: &str) -> Result<u64, StreamingError>;
            async fn supports_ranges(&self, url: &str) -> Result<bool, StreamingError>;
        }
    }

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let config = ConnectionPoolConfig {
            max_total_connections: 100,
            max_connections_per_host: 10,
            keep_alive_timeout: Duration::from_secs(30),
            tcp_keepalive: true,
            tcp_keepalive_interval: Duration::from_secs(60),
            enable_pooling: true,
        };

        let pool: ConnectionPool<MockTestHttpClient> = ConnectionPool::new(config);

        let server = CdnServer::https("example.com".to_string());
        let client = MockTestHttpClient::new();

        pool.add_client(&server, client);

        // Should be able to get client with guard
        let result = pool.get_client(&server).await;
        assert!(result.is_ok());

        let (_client, _guard) = result.expect("Operation should succeed");
        // Guard is automatically dropped at end of scope
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = ConnectionPoolConfig {
            max_total_connections: 100,
            max_connections_per_host: 10,
            keep_alive_timeout: Duration::from_secs(30),
            tcp_keepalive: true,
            tcp_keepalive_interval: Duration::from_secs(60),
            enable_pooling: true,
        };

        let pool: ConnectionPool<MockTestHttpClient> = ConnectionPool::new(config);

        let server = CdnServer::https("example.com".to_string());
        let client = MockTestHttpClient::new();

        pool.add_client(&server, client);

        // Record multiple failures to trigger circuit breaker
        for _ in 0..15 {
            pool.record_result(&server, false, Duration::from_millis(100))
                .await;
        }

        let stats = pool
            .get_stats(&server)
            .await
            .expect("Operation should succeed");
        assert!(matches!(stats.state, ConnectionState::CircuitOpen { .. }));

        // Should not be able to get client when circuit is open
        let result = pool.get_client(&server).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let config = ConnectionPoolConfig {
            max_total_connections: 100,
            max_connections_per_host: 10,
            keep_alive_timeout: Duration::from_secs(30),
            tcp_keepalive: true,
            tcp_keepalive_interval: Duration::from_secs(60),
            enable_pooling: true,
        };

        let pool: ConnectionPool<MockTestHttpClient> = ConnectionPool::new(config);

        let server = CdnServer::https("example.com".to_string());
        let client = MockTestHttpClient::new();

        pool.add_client(&server, client);

        // Record some successful and failed requests
        pool.record_result(&server, true, Duration::from_millis(100))
            .await;
        pool.record_result(&server, true, Duration::from_millis(150))
            .await;
        pool.record_result(&server, false, Duration::from_millis(200))
            .await;

        let stats = pool
            .get_stats(&server)
            .await
            .expect("Operation should succeed");
        assert_eq!(stats.requests, 3);
        assert_eq!(stats.successes, 2);
        assert_eq!(stats.failures, 1);
        assert!((stats.success_rate() - 0.666).abs() < 0.001);
        assert!(stats.avg_response_time_ms > 100.0 && stats.avg_response_time_ms < 200.0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = ConnectionPoolConfig {
            max_total_connections: 100,
            max_connections_per_host: 10,
            keep_alive_timeout: Duration::from_secs(30),
            tcp_keepalive: true,
            tcp_keepalive_interval: Duration::from_secs(60),
            enable_pooling: true,
        };

        let pool: ConnectionPool<MockTestHttpClient> = ConnectionPool::new(config);

        let server = CdnServer::https("example.com".to_string());
        let mut client = MockTestHttpClient::new();

        client.expect_get_content_length().returning(|_| Ok(1024));

        pool.add_client(&server, client);

        // Run health check
        let health_check_future = pool.health_check("https://example.com/test");
        let result = timeout(Duration::from_secs(5), health_check_future).await;
        assert!(result.is_ok());

        let health_result = result.expect("Operation should succeed");
        assert!(health_result.is_ok());

        let stats = pool
            .get_stats(&server)
            .await
            .expect("Operation should succeed");
        assert!(matches!(stats.state, ConnectionState::Healthy));
        assert_eq!(stats.successes, 1);
    }

    #[tokio::test]
    async fn test_server_removal() {
        let config = ConnectionPoolConfig {
            max_total_connections: 100,
            max_connections_per_host: 10,
            keep_alive_timeout: Duration::from_secs(30),
            tcp_keepalive: true,
            tcp_keepalive_interval: Duration::from_secs(60),
            enable_pooling: true,
        };

        let pool: ConnectionPool<MockTestHttpClient> = ConnectionPool::new(config);

        let server = CdnServer::https("example.com".to_string());
        let client = MockTestHttpClient::new();

        pool.add_client(&server, client);

        // Should be able to get client initially
        let result = pool.get_client(&server).await;
        assert!(result.is_ok());
        drop(result); // Drop the guard to release resources

        // Remove server
        pool.remove_server(&server).await;

        // Should not be able to get client after removal
        let result = pool.get_client(&server).await;
        assert!(result.is_err());

        let stats = pool
            .get_stats(&server)
            .await
            .expect("Operation should succeed");
        assert!(matches!(stats.state, ConnectionState::Removed));
    }
}

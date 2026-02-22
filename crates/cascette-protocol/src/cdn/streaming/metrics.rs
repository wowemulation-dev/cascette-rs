//! Metrics and monitoring for streaming CDN operations
//!
//! This module provides detailed metrics collection for monitoring CDN streaming
//! performance, connection health, and system behavior in production environments.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use dashmap::DashMap;
use prometheus::{Counter, Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Registry};
use tokio::sync::RwLock;

/// Pool-level metrics for connection management
#[derive(Debug)]
pub struct PoolMetrics {
    /// Total successful requests across all servers
    pub total_successful_requests: AtomicU64,
    /// Total failed requests across all servers
    pub total_failed_requests: AtomicU64,
    /// Number of active connections
    pub active_connections: AtomicU64,
    /// Number of circuit breakers activated
    pub circuit_breakers_activated: AtomicU64,
    /// Number of circuit breakers recovered
    pub circuit_breakers_recovered: AtomicU64,
    /// Number of servers removed from pool
    pub servers_removed: AtomicU64,
    /// Response time tracking
    response_times: Arc<RwLock<Vec<Duration>>>,
    /// Creation time for calculating uptime
    created_at: Instant,
}

impl PoolMetrics {
    /// Create new pool metrics
    pub fn new() -> Self {
        Self {
            total_successful_requests: AtomicU64::new(0),
            total_failed_requests: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            circuit_breakers_activated: AtomicU64::new(0),
            circuit_breakers_recovered: AtomicU64::new(0),
            servers_removed: AtomicU64::new(0),
            response_times: Arc::new(RwLock::new(Vec::with_capacity(1000))),
            created_at: Instant::now(),
        }
    }

    /// Update response time metrics (keeps last 1000 measurements)
    pub async fn update_response_time(&self, response_time: Duration) {
        let mut times = self.response_times.write().await;
        times.push(response_time);

        // Keep only last 1000 measurements
        if times.len() > 1000 {
            let excess = times.len() - 1000;
            times.drain(0..excess);
        }
    }

    /// Get average response time
    pub async fn average_response_time(&self) -> Option<Duration> {
        let times = self.response_times.read().await;
        if times.is_empty() {
            None
        } else {
            let total: Duration = times.iter().sum();
            Some(total / times.len() as u32)
        }
    }

    /// Get 95th percentile response time
    pub async fn p95_response_time(&self) -> Option<Duration> {
        let mut times = self.response_times.read().await.clone();
        if times.is_empty() {
            return None;
        }

        times.sort();
        #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
        // 0.95 factor ensures positive result
        let index = (times.len() as f64 * 0.95).round() as usize;
        times.get(index.min(times.len() - 1)).copied()
    }

    /// Get success rate (0.0 to 1.0)
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        let successes = self.total_successful_requests.load(Ordering::Relaxed);
        let failures = self.total_failed_requests.load(Ordering::Relaxed);
        let total = successes + failures;

        if total == 0 {
            1.0
        } else {
            successes as f64 / total as f64
        }
    }

    /// Get uptime duration
    pub fn uptime(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Get total request count
    pub fn total_requests(&self) -> u64 {
        self.total_successful_requests.load(Ordering::Relaxed)
            + self.total_failed_requests.load(Ordering::Relaxed)
    }
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance metrics for CDN streaming operations
#[derive(Debug)]
pub struct StreamingMetrics {
    /// Bytes downloaded counter
    pub bytes_downloaded: AtomicU64,
    /// Bytes uploaded counter (for range requests)
    pub bytes_uploaded: AtomicU64,
    /// Number of range requests made
    pub range_requests: AtomicU64,
    /// Number of ranges coalesced
    pub ranges_coalesced: AtomicU64,
    /// Number of CDN failovers
    pub cdn_failovers: AtomicU64,
    /// Number of retry attempts
    pub retry_attempts: AtomicU64,
    /// Current bandwidth usage (bytes/sec)
    pub current_bandwidth: AtomicU64,
    /// Peak bandwidth achieved
    pub peak_bandwidth: AtomicU64,
    /// Memory usage tracking
    pub memory_used: AtomicU64,
    /// Cache hit ratio for various caches
    pub cache_stats: Arc<DashMap<String, CacheStats>>,
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: AtomicU64,
    /// Number of cache misses
    pub misses: AtomicU64,
    /// Current cache size
    pub size: AtomicU64,
    /// Number of evictions
    pub evictions: AtomicU64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            size: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }
}

impl Clone for CacheStats {
    fn clone(&self) -> Self {
        Self {
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.misses.load(Ordering::Relaxed)),
            size: AtomicU64::new(self.size.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.evictions.load(Ordering::Relaxed)),
        }
    }
}

impl CacheStats {
    /// Get cache hit ratio (0.0 to 1.0)
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_ratio(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            1.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl StreamingMetrics {
    /// Create new streaming metrics
    pub fn new() -> Self {
        Self {
            bytes_downloaded: AtomicU64::new(0),
            bytes_uploaded: AtomicU64::new(0),
            range_requests: AtomicU64::new(0),
            ranges_coalesced: AtomicU64::new(0),
            cdn_failovers: AtomicU64::new(0),
            retry_attempts: AtomicU64::new(0),
            current_bandwidth: AtomicU64::new(0),
            peak_bandwidth: AtomicU64::new(0),
            memory_used: AtomicU64::new(0),
            cache_stats: Arc::new(DashMap::new()),
        }
    }

    /// Record downloaded bytes and update bandwidth
    pub fn record_download(&self, bytes: u64, duration: Duration) {
        self.bytes_downloaded.fetch_add(bytes, Ordering::Relaxed);

        if duration.as_secs() > 0 {
            let bandwidth = bytes / duration.as_secs();
            self.current_bandwidth.store(bandwidth, Ordering::Relaxed);

            // Update peak bandwidth
            let current_peak = self.peak_bandwidth.load(Ordering::Relaxed);
            if bandwidth > current_peak {
                self.peak_bandwidth.store(bandwidth, Ordering::Relaxed);
            }
        }
    }

    /// Get cache statistics for a named cache
    pub fn cache_stats(&self, name: &str) -> CacheStats {
        self.cache_stats
            .get(name)
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }

    /// Record cache hit
    pub fn record_cache_hit(&self, cache_name: &str) {
        let stats = self.cache_stats.entry(cache_name.to_string()).or_default();
        stats.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cache miss
    pub fn record_cache_miss(&self, cache_name: &str) {
        let stats = self.cache_stats.entry(cache_name.to_string()).or_default();
        stats.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Update cache size
    pub fn update_cache_size(&self, cache_name: &str, size: u64) {
        let stats = self.cache_stats.entry(cache_name.to_string()).or_default();
        stats.size.store(size, Ordering::Relaxed);
    }

    /// Record cache eviction
    pub fn record_cache_eviction(&self, cache_name: &str) {
        let stats = self.cache_stats.entry(cache_name.to_string()).or_default();
        stats.evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Get bandwidth efficiency (downloaded/uploaded ratio)
    #[allow(clippy::cast_precision_loss)]
    pub fn bandwidth_efficiency(&self) -> f64 {
        let downloaded = self.bytes_downloaded.load(Ordering::Relaxed);
        let uploaded = self.bytes_uploaded.load(Ordering::Relaxed);

        if uploaded == 0 {
            f64::INFINITY
        } else {
            downloaded as f64 / uploaded as f64
        }
    }
}

impl Default for StreamingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Prometheus metrics exporter for CDN streaming
#[derive(Debug)]
pub struct PrometheusExporter {
    registry: Registry,

    // Connection pool metrics
    pool_active_connections: IntGauge,
    pool_successful_requests: IntCounter,
    pool_failed_requests: IntCounter,
    pool_circuit_breakers: IntGauge,
    pool_response_time: Histogram,

    // Streaming metrics
    bytes_downloaded: Counter,
    bytes_uploaded: Counter,
    range_requests: IntCounter,
    ranges_coalesced: IntCounter,
    cdn_failovers: IntCounter,
    retry_attempts: IntCounter,
    current_bandwidth: Gauge,
    memory_usage: Gauge,

    // Cache metrics
    cache_hits: IntCounter,
    cache_misses: IntCounter,
    cache_size: Gauge,
    cache_evictions: IntCounter,
}

impl PrometheusExporter {
    /// Create new Prometheus exporter
    #[allow(clippy::too_many_lines)] // Prometheus metrics initialization requires many lines
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Connection pool metrics
        let pool_active_connections = IntGauge::new(
            "cascette_pool_active_connections",
            "Number of active connections in the pool",
        )?;
        registry.register(Box::new(pool_active_connections.clone()))?;

        let pool_successful_requests = IntCounter::new(
            "cascette_pool_successful_requests_total",
            "Total number of successful requests",
        )?;
        registry.register(Box::new(pool_successful_requests.clone()))?;

        let pool_failed_requests = IntCounter::new(
            "cascette_pool_failed_requests_total",
            "Total number of failed requests",
        )?;
        registry.register(Box::new(pool_failed_requests.clone()))?;

        let pool_circuit_breakers = IntGauge::new(
            "cascette_pool_circuit_breakers",
            "Number of active circuit breakers",
        )?;
        registry.register(Box::new(pool_circuit_breakers.clone()))?;

        let pool_response_time = Histogram::with_opts(
            HistogramOpts::new(
                "cascette_pool_response_time_seconds",
                "Response time distribution for pool requests",
            )
            .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0]),
        )?;
        registry.register(Box::new(pool_response_time.clone()))?;

        // Streaming metrics
        let bytes_downloaded =
            Counter::new("cascette_bytes_downloaded_total", "Total bytes downloaded")?;
        registry.register(Box::new(bytes_downloaded.clone()))?;

        let bytes_uploaded = Counter::new(
            "cascette_bytes_uploaded_total",
            "Total bytes uploaded (range requests)",
        )?;
        registry.register(Box::new(bytes_uploaded.clone()))?;

        let range_requests = IntCounter::new(
            "cascette_range_requests_total",
            "Total number of range requests made",
        )?;
        registry.register(Box::new(range_requests.clone()))?;

        let ranges_coalesced = IntCounter::new(
            "cascette_ranges_coalesced_total",
            "Total number of ranges coalesced",
        )?;
        registry.register(Box::new(ranges_coalesced.clone()))?;

        let cdn_failovers = IntCounter::new(
            "cascette_cdn_failovers_total",
            "Total number of CDN failovers",
        )?;
        registry.register(Box::new(cdn_failovers.clone()))?;

        let retry_attempts = IntCounter::new(
            "cascette_retry_attempts_total",
            "Total number of retry attempts",
        )?;
        registry.register(Box::new(retry_attempts.clone()))?;

        let current_bandwidth = Gauge::new(
            "cascette_current_bandwidth_bytes_per_sec",
            "Current bandwidth usage in bytes per second",
        )?;
        registry.register(Box::new(current_bandwidth.clone()))?;

        let memory_usage = Gauge::new(
            "cascette_memory_usage_bytes",
            "Current memory usage in bytes",
        )?;
        registry.register(Box::new(memory_usage.clone()))?;

        // Cache metrics
        let cache_hits =
            IntCounter::new("cascette_cache_hits_total", "Total number of cache hits")?;
        registry.register(Box::new(cache_hits.clone()))?;

        let cache_misses = IntCounter::new(
            "cascette_cache_misses_total",
            "Total number of cache misses",
        )?;
        registry.register(Box::new(cache_misses.clone()))?;

        let cache_size = Gauge::new("cascette_cache_size_bytes", "Current cache size in bytes")?;
        registry.register(Box::new(cache_size.clone()))?;

        let cache_evictions = IntCounter::new(
            "cascette_cache_evictions_total",
            "Total number of cache evictions",
        )?;
        registry.register(Box::new(cache_evictions.clone()))?;

        Ok(Self {
            registry,
            pool_active_connections,
            pool_successful_requests,
            pool_failed_requests,
            pool_circuit_breakers,
            pool_response_time,
            bytes_downloaded,
            bytes_uploaded,
            range_requests,
            ranges_coalesced,
            cdn_failovers,
            retry_attempts,
            current_bandwidth,
            memory_usage,
            cache_hits,
            cache_misses,
            cache_size,
            cache_evictions,
        })
    }

    /// Update metrics from pool metrics
    #[allow(clippy::cast_possible_wrap)]
    pub fn update_from_pool_metrics(&self, metrics: &PoolMetrics) {
        self.pool_active_connections
            .set(metrics.active_connections.load(Ordering::Relaxed) as i64);
        self.pool_successful_requests
            .inc_by(metrics.total_successful_requests.load(Ordering::Relaxed));
        self.pool_failed_requests
            .inc_by(metrics.total_failed_requests.load(Ordering::Relaxed));

        // Circuit breakers is calculated as activated - recovered
        let active_breakers = metrics.circuit_breakers_activated.load(Ordering::Relaxed)
            - metrics.circuit_breakers_recovered.load(Ordering::Relaxed);
        self.pool_circuit_breakers.set(active_breakers as i64);
    }

    /// Update metrics from streaming metrics
    #[allow(clippy::cast_precision_loss)]
    pub fn update_from_streaming_metrics(&self, metrics: &StreamingMetrics) {
        self.bytes_downloaded
            .inc_by(metrics.bytes_downloaded.load(Ordering::Relaxed) as f64);
        self.bytes_uploaded
            .inc_by(metrics.bytes_uploaded.load(Ordering::Relaxed) as f64);
        self.range_requests
            .inc_by(metrics.range_requests.load(Ordering::Relaxed));
        self.ranges_coalesced
            .inc_by(metrics.ranges_coalesced.load(Ordering::Relaxed));
        self.cdn_failovers
            .inc_by(metrics.cdn_failovers.load(Ordering::Relaxed));
        self.retry_attempts
            .inc_by(metrics.retry_attempts.load(Ordering::Relaxed));
        self.current_bandwidth
            .set(metrics.current_bandwidth.load(Ordering::Relaxed) as f64);
        self.memory_usage
            .set(metrics.memory_used.load(Ordering::Relaxed) as f64);

        // Aggregate cache metrics
        let mut total_hits = 0;
        let mut total_misses = 0;
        let mut total_size = 0;
        let mut total_evictions = 0;

        for entry in metrics.cache_stats.iter() {
            let stats = entry.value();
            total_hits += stats.hits.load(Ordering::Relaxed);
            total_misses += stats.misses.load(Ordering::Relaxed);
            total_size += stats.size.load(Ordering::Relaxed);
            total_evictions += stats.evictions.load(Ordering::Relaxed);
        }

        self.cache_hits.inc_by(total_hits);
        self.cache_misses.inc_by(total_misses);
        self.cache_size.set(total_size as f64);
        self.cache_evictions.inc_by(total_evictions);
    }

    /// Record response time
    pub fn record_response_time(&self, duration: Duration) {
        self.pool_response_time.observe(duration.as_secs_f64());
    }

    /// Get Prometheus registry for serving metrics
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Gather all metrics in Prometheus format
    pub fn gather(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode_to_string(&metric_families)
            .unwrap_or_default()
    }
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        #[allow(clippy::expect_used)]
        // expect_used: PrometheusExporter::new() only fails if prometheus registry
        // operations fail, which should never happen with default configuration.
        Self::new().expect("PrometheusExporter creation failed")
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp, clippy::cast_precision_loss)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_pool_metrics() {
        let metrics = PoolMetrics::new();

        assert_eq!(metrics.total_requests(), 0);
        assert_eq!(metrics.success_rate(), 1.0);

        metrics
            .total_successful_requests
            .store(7, Ordering::Relaxed);
        metrics.total_failed_requests.store(3, Ordering::Relaxed);

        assert_eq!(metrics.total_requests(), 10);
        assert!((metrics.success_rate() - 0.7).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_response_time_tracking() {
        let metrics = PoolMetrics::new();

        metrics
            .update_response_time(Duration::from_millis(100))
            .await;
        metrics
            .update_response_time(Duration::from_millis(200))
            .await;
        metrics
            .update_response_time(Duration::from_millis(150))
            .await;

        let avg = metrics
            .average_response_time()
            .await
            .expect("Operation should succeed");
        assert!((avg.as_millis() as f64 - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_streaming_metrics() {
        let metrics = StreamingMetrics::new();

        metrics.record_download(1000, Duration::from_secs(1));
        assert_eq!(metrics.bytes_downloaded.load(Ordering::Relaxed), 1000);
        assert_eq!(metrics.current_bandwidth.load(Ordering::Relaxed), 1000);
        assert_eq!(metrics.peak_bandwidth.load(Ordering::Relaxed), 1000);

        metrics.record_download(2000, Duration::from_secs(1));
        assert_eq!(metrics.peak_bandwidth.load(Ordering::Relaxed), 2000);
    }

    #[test]
    fn test_cache_stats() {
        let metrics = StreamingMetrics::new();

        metrics.record_cache_hit("test_cache");
        metrics.record_cache_hit("test_cache");
        metrics.record_cache_miss("test_cache");

        let stats = metrics.cache_stats("test_cache");
        assert_eq!(stats.hits.load(Ordering::Relaxed), 2);
        assert_eq!(stats.misses.load(Ordering::Relaxed), 1);
        assert!((stats.hit_ratio() - 0.666).abs() < 0.001);
    }

    #[test]
    fn test_prometheus_exporter() {
        let exporter = PrometheusExporter::new().expect("Operation should succeed");

        let pool_metrics = PoolMetrics::new();
        pool_metrics
            .total_successful_requests
            .store(100, Ordering::Relaxed);
        pool_metrics
            .total_failed_requests
            .store(10, Ordering::Relaxed);

        exporter.update_from_pool_metrics(&pool_metrics);

        let output = exporter.gather();
        assert!(output.contains("cascette_pool_successful_requests_total"));
        assert!(output.contains("cascette_pool_failed_requests_total"));
    }
}

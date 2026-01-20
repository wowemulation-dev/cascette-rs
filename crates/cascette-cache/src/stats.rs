//! Cache statistics and metrics types
//!
//! This module provides comprehensive statistics and metrics tracking for
//! cache performance monitoring, debugging, and optimization.
//! Optimized for NGDP workload patterns with efficient atomic operations.

#![allow(missing_docs)]
#![allow(clippy::cast_precision_loss)] // Statistics calculations intentionally accept precision loss

use std::{
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

// Use cache-aligned atomics to reduce false sharing
#[repr(align(64))] // Cache line alignment
#[derive(Debug)]
struct CacheAlignedAtomicU64(AtomicU64);

#[repr(align(64))] // Cache line alignment
#[derive(Debug)]
struct CacheAlignedAtomicUsize(AtomicUsize);

impl CacheAlignedAtomicU64 {
    fn new(value: u64) -> Self {
        Self(AtomicU64::new(value))
    }

    #[inline]
    fn load(&self, ordering: Ordering) -> u64 {
        self.0.load(ordering)
    }

    #[inline]
    fn store(&self, value: u64, ordering: Ordering) {
        self.0.store(value, ordering);
    }

    #[inline]
    fn fetch_add(&self, value: u64, ordering: Ordering) -> u64 {
        self.0.fetch_add(value, ordering)
    }

    #[inline]
    #[allow(dead_code)]
    fn fetch_sub(&self, value: u64, ordering: Ordering) -> u64 {
        self.0.fetch_sub(value, ordering)
    }

    #[inline]
    #[allow(dead_code)]
    fn fetch_max(&self, value: u64, ordering: Ordering) -> u64 {
        self.0.fetch_max(value, ordering)
    }
}

impl CacheAlignedAtomicUsize {
    fn new(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }

    #[inline]
    fn load(&self, ordering: Ordering) -> usize {
        self.0.load(ordering)
    }

    #[inline]
    fn store(&self, value: usize, ordering: Ordering) {
        self.0.store(value, ordering);
    }

    #[inline]
    fn fetch_add(&self, value: usize, ordering: Ordering) -> usize {
        self.0.fetch_add(value, ordering)
    }

    #[inline]
    fn fetch_sub(&self, value: usize, ordering: Ordering) -> usize {
        self.0.fetch_sub(value, ordering)
    }

    #[inline]
    fn fetch_max(&self, value: usize, ordering: Ordering) -> usize {
        self.0.fetch_max(value, ordering)
    }
}

/// Cache statistics snapshot
///
/// Contains point-in-time statistics about cache performance and usage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)] // Fields like memory_usage_bytes follow common naming convention
pub struct CacheStats {
    /// Total number of get operations
    pub get_count: u64,
    /// Number of successful cache hits
    pub hit_count: u64,
    /// Number of cache misses
    pub miss_count: u64,
    /// Total number of put operations
    pub put_count: u64,
    /// Total number of remove operations
    pub remove_count: u64,
    /// Total number of evictions
    pub eviction_count: u64,
    /// Total number of expired entries removed
    pub expiration_count: u64,
    /// Current number of entries in cache
    pub entry_count: usize,
    /// Total memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Maximum memory usage observed
    pub max_memory_usage_bytes: usize,
    /// Cache creation time
    pub created_at: Instant,
    /// Time of last statistics update
    pub updated_at: Instant,
    /// Average response time for get operations
    pub avg_get_time: Duration,
    /// Average response time for put operations
    pub avg_put_time: Duration,
}

impl CacheStats {
    /// Create new empty cache statistics
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            get_count: 0,
            hit_count: 0,
            miss_count: 0,
            put_count: 0,
            remove_count: 0,
            eviction_count: 0,
            expiration_count: 0,
            entry_count: 0,
            memory_usage_bytes: 0,
            max_memory_usage_bytes: 0,
            created_at: now,
            updated_at: now,
            avg_get_time: Duration::ZERO,
            avg_put_time: Duration::ZERO,
        }
    }

    /// Calculate hit rate (hits / total gets)
    #[inline]
    pub fn hit_rate(&self) -> f64 {
        if self.get_count == 0 {
            0.0
        } else {
            self.hit_count as f64 / self.get_count as f64
        }
    }

    /// Calculate miss rate (misses / total gets)
    #[inline]
    pub fn miss_rate(&self) -> f64 {
        if self.get_count == 0 {
            0.0
        } else {
            self.miss_count as f64 / self.get_count as f64
        }
    }

    /// Calculate cache utilization based on maximum entries
    #[inline]
    pub fn utilization(&self, max_entries: usize) -> f64 {
        if max_entries == 0 {
            0.0
        } else {
            self.entry_count as f64 / max_entries as f64
        }
    }

    /// Calculate memory utilization based on maximum memory
    #[inline]
    pub fn memory_utilization(&self, max_memory_bytes: usize) -> f64 {
        if max_memory_bytes == 0 {
            0.0
        } else {
            self.memory_usage_bytes as f64 / max_memory_bytes as f64
        }
    }

    /// Get cache age (time since creation)
    #[inline]
    pub fn age(&self) -> Duration {
        Instant::now() - self.created_at
    }

    /// Merge statistics from another cache (for multi-layer stats)
    pub fn merge(&mut self, other: &CacheStats) {
        let prev_get_count = self.get_count;
        let prev_put_count = self.put_count;

        self.get_count = self.get_count.saturating_add(other.get_count);
        self.hit_count = self.hit_count.saturating_add(other.hit_count);
        self.miss_count = self.miss_count.saturating_add(other.miss_count);
        self.put_count = self.put_count.saturating_add(other.put_count);
        self.remove_count = self.remove_count.saturating_add(other.remove_count);
        self.eviction_count = self.eviction_count.saturating_add(other.eviction_count);
        self.expiration_count = self.expiration_count.saturating_add(other.expiration_count);
        self.entry_count = self.entry_count.saturating_add(other.entry_count);
        self.memory_usage_bytes = self
            .memory_usage_bytes
            .saturating_add(other.memory_usage_bytes);
        self.max_memory_usage_bytes = self
            .max_memory_usage_bytes
            .max(other.max_memory_usage_bytes);
        self.updated_at = Instant::now();

        // Weighted average for response times - avoid overflow
        if self.get_count > 0 {
            let total_get_time_nanos = (self.avg_get_time.as_nanos() as u64)
                .saturating_mul(prev_get_count)
                .saturating_add(
                    (other.avg_get_time.as_nanos() as u64).saturating_mul(other.get_count),
                );
            self.avg_get_time = Duration::from_nanos(total_get_time_nanos / self.get_count);
        }

        if self.put_count > 0 {
            let total_put_time_nanos = (self.avg_put_time.as_nanos() as u64)
                .saturating_mul(prev_put_count)
                .saturating_add(
                    (other.avg_put_time.as_nanos() as u64).saturating_mul(other.put_count),
                );
            self.avg_put_time = Duration::from_nanos(total_put_time_nanos / self.put_count);
        }
    }
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

/// High-performance atomic cache metrics for thread-safe statistics collection
///
/// Used internally by cache implementations to track metrics efficiently
/// across multiple threads. Optimized for NGDP workloads with cache-aligned
/// atomics to reduce false sharing and improve performance.
#[derive(Debug)]
pub struct AtomicCacheMetrics {
    // Group frequently updated counters together in separate cache lines
    get_count: CacheAlignedAtomicU64,
    hit_count: CacheAlignedAtomicU64,
    miss_count: CacheAlignedAtomicU64,

    put_count: CacheAlignedAtomicU64,
    remove_count: CacheAlignedAtomicU64,
    eviction_count: CacheAlignedAtomicU64,
    expiration_count: CacheAlignedAtomicU64,

    // Memory-related counters in separate cache lines
    entry_count: CacheAlignedAtomicUsize,
    memory_usage_bytes: CacheAlignedAtomicUsize,
    max_memory_usage_bytes: CacheAlignedAtomicUsize,

    // Timing information - less frequently updated
    total_get_time_nanos: CacheAlignedAtomicU64,
    total_put_time_nanos: CacheAlignedAtomicU64,

    // Creation time (immutable after construction)
    created_at: Instant,
}

impl AtomicCacheMetrics {
    /// Create new atomic cache metrics
    pub fn new() -> Self {
        Self {
            get_count: CacheAlignedAtomicU64::new(0),
            hit_count: CacheAlignedAtomicU64::new(0),
            miss_count: CacheAlignedAtomicU64::new(0),
            put_count: CacheAlignedAtomicU64::new(0),
            remove_count: CacheAlignedAtomicU64::new(0),
            eviction_count: CacheAlignedAtomicU64::new(0),
            expiration_count: CacheAlignedAtomicU64::new(0),
            entry_count: CacheAlignedAtomicUsize::new(0),
            memory_usage_bytes: CacheAlignedAtomicUsize::new(0),
            max_memory_usage_bytes: CacheAlignedAtomicUsize::new(0),
            total_get_time_nanos: CacheAlignedAtomicU64::new(0),
            total_put_time_nanos: CacheAlignedAtomicU64::new(0),
            created_at: Instant::now(),
        }
    }

    /// Record a cache get operation with optimized hot path
    #[inline]
    pub fn record_get(&self, hit: bool, duration: Duration) {
        // Use acquire ordering only where necessary for correctness
        self.get_count.fetch_add(1, Ordering::Relaxed);

        if hit {
            self.hit_count.fetch_add(1, Ordering::Relaxed);
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
        }

        // Only update timing if duration is meaningful (> 1µs)
        let duration_nanos = duration.as_nanos() as u64;
        if duration_nanos > 1000 {
            self.total_get_time_nanos
                .fetch_add(duration_nanos, Ordering::Relaxed);
        }
    }

    /// Record a cache put operation with memory tracking
    #[inline]
    pub fn record_put(&self, size_bytes: usize, duration: Duration) {
        self.put_count.fetch_add(1, Ordering::Relaxed);
        self.entry_count.fetch_add(1, Ordering::Relaxed);

        // Update memory usage and max atomically
        let new_memory = self
            .memory_usage_bytes
            .fetch_add(size_bytes, Ordering::Relaxed)
            + size_bytes;
        self.max_memory_usage_bytes
            .fetch_max(new_memory, Ordering::Relaxed);

        // Only update timing if duration is meaningful
        let duration_nanos = duration.as_nanos() as u64;
        if duration_nanos > 1000 {
            self.total_put_time_nanos
                .fetch_add(duration_nanos, Ordering::Relaxed);
        }
    }

    /// Record a cache remove operation
    #[inline]
    pub fn record_remove(&self, size_bytes: usize) {
        self.remove_count.fetch_add(1, Ordering::Relaxed);
        self.entry_count.fetch_sub(1, Ordering::Relaxed);
        self.memory_usage_bytes
            .fetch_sub(size_bytes, Ordering::Relaxed);
    }

    /// Record a cache eviction
    #[inline]
    pub fn record_eviction(&self, size_bytes: usize) {
        self.eviction_count.fetch_add(1, Ordering::Relaxed);
        self.entry_count.fetch_sub(1, Ordering::Relaxed);
        self.memory_usage_bytes
            .fetch_sub(size_bytes, Ordering::Relaxed);
    }

    /// Record an expiration
    #[inline]
    pub fn record_expiration(&self, size_bytes: usize) {
        self.expiration_count.fetch_add(1, Ordering::Relaxed);
        self.entry_count.fetch_sub(1, Ordering::Relaxed);
        self.memory_usage_bytes
            .fetch_sub(size_bytes, Ordering::Relaxed);
    }

    /// Batch record multiple get operations for efficiency
    /// Optimized for NGDP workloads where multiple keys are often accessed together
    #[inline]
    pub fn record_batch_gets(&self, operations: &[(bool, Duration)]) {
        if operations.is_empty() {
            return;
        }

        let mut total_gets = 0u64;
        let mut total_hits = 0u64;
        let mut total_time_nanos = 0u64;

        for &(hit, duration) in operations {
            total_gets += 1;
            if hit {
                total_hits += 1;
            }

            let duration_nanos = duration.as_nanos() as u64;
            if duration_nanos > 1000 {
                total_time_nanos = total_time_nanos.saturating_add(duration_nanos);
            }
        }

        // Batch update atomics
        self.get_count.fetch_add(total_gets, Ordering::Relaxed);
        self.hit_count.fetch_add(total_hits, Ordering::Relaxed);
        self.miss_count
            .fetch_add(total_gets - total_hits, Ordering::Relaxed);

        if total_time_nanos > 0 {
            self.total_get_time_nanos
                .fetch_add(total_time_nanos, Ordering::Relaxed);
        }
    }

    /// Get current statistics snapshot with optimized reads
    pub fn snapshot(&self) -> CacheStats {
        // Use acquire ordering for consistent reads across all metrics
        let get_count = self.get_count.load(Ordering::Acquire);
        let hit_count = self.hit_count.load(Ordering::Relaxed);
        let miss_count = self.miss_count.load(Ordering::Relaxed);
        let put_count = self.put_count.load(Ordering::Relaxed);
        let remove_count = self.remove_count.load(Ordering::Relaxed);
        let eviction_count = self.eviction_count.load(Ordering::Relaxed);
        let expiration_count = self.expiration_count.load(Ordering::Relaxed);
        let entry_count = self.entry_count.load(Ordering::Relaxed);
        let memory_usage_bytes = self.memory_usage_bytes.load(Ordering::Relaxed);
        let max_memory_usage_bytes = self.max_memory_usage_bytes.load(Ordering::Relaxed);
        let total_get_time_nanos = self.total_get_time_nanos.load(Ordering::Relaxed);
        let total_put_time_nanos = self.total_put_time_nanos.load(Ordering::Relaxed);

        let avg_get_time = if get_count > 0 {
            Duration::from_nanos(total_get_time_nanos / get_count)
        } else {
            Duration::ZERO
        };

        let avg_put_time = if put_count > 0 {
            Duration::from_nanos(total_put_time_nanos / put_count)
        } else {
            Duration::ZERO
        };

        CacheStats {
            get_count,
            hit_count,
            miss_count,
            put_count,
            remove_count,
            eviction_count,
            expiration_count,
            entry_count,
            memory_usage_bytes,
            max_memory_usage_bytes,
            created_at: self.created_at,
            updated_at: Instant::now(),
            avg_get_time,
            avg_put_time,
        }
    }

    /// Get lightweight metrics for hot paths (reduced precision)
    /// Optimized for high-frequency monitoring in NGDP systems
    #[inline]
    pub fn fast_snapshot(&self) -> FastCacheMetrics {
        FastCacheMetrics {
            get_count: self.get_count.load(Ordering::Relaxed),
            hit_count: self.hit_count.load(Ordering::Relaxed),
            entry_count: self.entry_count.load(Ordering::Relaxed) as u64,
            memory_usage_mb: (self.memory_usage_bytes.load(Ordering::Relaxed) / (1024 * 1024))
                as u32,
        }
    }

    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.get_count.store(0, Ordering::Relaxed);
        self.hit_count.store(0, Ordering::Relaxed);
        self.miss_count.store(0, Ordering::Relaxed);
        self.put_count.store(0, Ordering::Relaxed);
        self.remove_count.store(0, Ordering::Relaxed);
        self.eviction_count.store(0, Ordering::Relaxed);
        self.expiration_count.store(0, Ordering::Relaxed);
        self.entry_count.store(0, Ordering::Relaxed);
        self.memory_usage_bytes.store(0, Ordering::Relaxed);
        self.max_memory_usage_bytes.store(0, Ordering::Relaxed);
        self.total_get_time_nanos.store(0, Ordering::Relaxed);
        self.total_put_time_nanos.store(0, Ordering::Relaxed);
    }

    /// Get hit rate without full snapshot (optimized for monitoring)
    #[inline]
    pub fn fast_hit_rate(&self) -> f32 {
        let get_count = self.get_count.load(Ordering::Relaxed);
        if get_count == 0 {
            0.0
        } else {
            let hit_count = self.hit_count.load(Ordering::Relaxed);
            (hit_count as f32) / (get_count as f32)
        }
    }

    /// Get memory utilization without full snapshot
    #[inline]
    pub fn fast_memory_utilization(&self, max_memory_bytes: usize) -> f32 {
        if max_memory_bytes == 0 {
            0.0
        } else {
            let current_memory = self.memory_usage_bytes.load(Ordering::Relaxed);
            (current_memory as f32) / (max_memory_bytes as f32)
        }
    }
}

impl Default for AtomicCacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight cache metrics for high-frequency monitoring
/// Optimized for NGDP hot paths where full statistics are too expensive
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastCacheMetrics {
    /// Total get operations
    pub get_count: u64,
    /// Cache hits
    pub hit_count: u64,
    /// Current entries
    pub entry_count: u64,
    /// Memory usage in MB (reduced precision for efficiency)
    pub memory_usage_mb: u32,
}

impl FastCacheMetrics {
    /// Calculate hit rate
    #[inline]
    pub fn hit_rate(&self) -> f32 {
        if self.get_count == 0 {
            0.0
        } else {
            (self.hit_count as f32) / (self.get_count as f32)
        }
    }

    /// Get memory usage in bytes (estimated)
    #[inline]
    pub fn memory_usage_bytes(&self) -> usize {
        (self.memory_usage_mb as usize) * 1024 * 1024
    }
}

/// Multi-layer cache statistics
///
/// Aggregates statistics from multiple cache layers for hierarchical caches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiLayerStats {
    /// Statistics for each layer (L1, L2, L3, etc.)
    pub layer_stats: Vec<CacheStats>,
    /// Aggregated statistics across all layers
    pub total_stats: CacheStats,
    /// Promotion statistics between layers
    pub promotion_stats: PromotionStats,
}

impl MultiLayerStats {
    /// Create new multi-layer statistics
    pub fn new(layers: usize) -> Self {
        Self {
            layer_stats: vec![CacheStats::new(); layers],
            total_stats: CacheStats::new(),
            promotion_stats: PromotionStats::new(),
        }
    }

    /// Update statistics for a specific layer
    pub fn update_layer(&mut self, layer_index: usize, stats: CacheStats) {
        if layer_index < self.layer_stats.len() {
            self.layer_stats[layer_index] = stats;
            self.recalculate_totals();
        }
    }

    /// Record a promotion between layers
    pub fn record_promotion(&mut self, from_layer: usize, to_layer: usize) {
        self.promotion_stats.record_promotion(from_layer, to_layer);
    }

    /// Recalculate total statistics from all layers
    fn recalculate_totals(&mut self) {
        self.total_stats = CacheStats::new();
        for layer_stats in &self.layer_stats {
            self.total_stats.merge(layer_stats);
        }
    }

    /// Get aggregated hit rate across all layers
    #[inline]
    pub fn overall_hit_rate(&self) -> f64 {
        self.total_stats.hit_rate()
    }
}

/// Statistics for promotions between cache layers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionStats {
    /// Total number of promotions
    pub total_promotions: u64,
    /// Promotions per layer pair (from_layer -> to_layer)
    pub layer_promotions: std::collections::HashMap<(usize, usize), u64>,
}

impl PromotionStats {
    /// Create new promotion statistics
    pub fn new() -> Self {
        Self {
            total_promotions: 0,
            layer_promotions: std::collections::HashMap::new(),
        }
    }

    /// Record a promotion between layers
    pub fn record_promotion(&mut self, from_layer: usize, to_layer: usize) {
        self.total_promotions = self.total_promotions.saturating_add(1);
        *self
            .layer_promotions
            .entry((from_layer, to_layer))
            .or_insert(0) = self
            .layer_promotions
            .get(&(from_layer, to_layer))
            .unwrap_or(&0)
            .saturating_add(1);
    }

    /// Get promotion count between specific layers
    pub fn get_promotion_count(&self, from_layer: usize, to_layer: usize) -> u64 {
        self.layer_promotions
            .get(&(from_layer, to_layer))
            .copied()
            .unwrap_or(0)
    }
}

impl Default for PromotionStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance metrics for cache operations
///
/// Tracks detailed performance metrics for different types of cache operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceMetrics {
    /// Get operation metrics
    pub get_metrics: OperationMetrics,
    /// Put operation metrics
    pub put_metrics: OperationMetrics,
    /// Remove operation metrics
    pub remove_metrics: OperationMetrics,
    /// Eviction metrics
    pub eviction_metrics: OperationMetrics,
}

impl PerformanceMetrics {
    /// Create new performance metrics
    pub fn new() -> Self {
        Self {
            get_metrics: OperationMetrics::new("get"),
            put_metrics: OperationMetrics::new("put"),
            remove_metrics: OperationMetrics::new("remove"),
            eviction_metrics: OperationMetrics::new("eviction"),
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics for a specific type of cache operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationMetrics {
    /// Operation name
    pub operation_name: String,
    /// Total number of operations
    pub count: u64,
    /// Total time spent on operations
    pub total_duration: Duration,
    /// Minimum operation time
    pub min_duration: Duration,
    /// Maximum operation time
    pub max_duration: Duration,
    /// 95th percentile operation time
    pub p95_duration: Duration,
    /// 99th percentile operation time
    pub p99_duration: Duration,
}

impl OperationMetrics {
    /// Create new operation metrics
    pub fn new(operation_name: impl Into<String>) -> Self {
        Self {
            operation_name: operation_name.into(),
            count: 0,
            total_duration: Duration::ZERO,
            min_duration: Duration::MAX,
            max_duration: Duration::ZERO,
            p95_duration: Duration::ZERO,
            p99_duration: Duration::ZERO,
        }
    }

    /// Record an operation duration
    pub fn record(&mut self, duration: Duration) {
        self.count = self.count.saturating_add(1);
        self.total_duration = self.total_duration.saturating_add(duration);
        self.min_duration = self.min_duration.min(duration);
        self.max_duration = self.max_duration.max(duration);
        // Note: Percentile calculation would require keeping a history
        // of durations or using a more sophisticated data structure
    }

    /// Get average operation duration
    #[inline]
    pub fn avg_duration(&self) -> Duration {
        if self.count == 0 {
            Duration::ZERO
        } else {
            self.total_duration / self.count as u32
        }
    }

    /// Get operations per second
    #[inline]
    pub fn ops_per_second(&self, time_window: Duration) -> f64 {
        let secs = time_window.as_secs_f64();
        if secs == 0.0 {
            0.0
        } else {
            self.count as f64 / secs
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_cache_stats_new() {
        let stats = CacheStats::new();
        assert_eq!(stats.get_count, 0);
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.memory_usage_bytes, 0);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let mut stats = CacheStats::new();
        assert_eq!(stats.hit_rate(), 0.0);

        stats.get_count = 10;
        stats.hit_count = 7;
        stats.miss_count = 3;
        assert!((stats.hit_rate() - 0.7).abs() < f64::EPSILON);
        assert!((stats.miss_rate() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_stats_utilization() {
        let mut stats = CacheStats::new();
        stats.entry_count = 250;
        assert!((stats.utilization(1000) - 0.25).abs() < f64::EPSILON);

        stats.memory_usage_bytes = 512 * 1024; // 512 KB
        assert!((stats.memory_utilization(1024 * 1024) - 0.5).abs() < f64::EPSILON); // 1 MB max
    }

    #[test]
    fn test_cache_stats_merge() {
        let mut stats1 = CacheStats::new();
        stats1.get_count = 10;
        stats1.hit_count = 8;
        stats1.put_count = 5;
        stats1.entry_count = 5;
        stats1.memory_usage_bytes = 1000;

        let mut stats2 = CacheStats::new();
        stats2.get_count = 20;
        stats2.hit_count = 15;
        stats2.put_count = 10;
        stats2.entry_count = 10;
        stats2.memory_usage_bytes = 2000;

        stats1.merge(&stats2);
        assert_eq!(stats1.get_count, 30);
        assert_eq!(stats1.hit_count, 23);
        assert_eq!(stats1.put_count, 15);
        assert_eq!(stats1.entry_count, 15);
        assert_eq!(stats1.memory_usage_bytes, 3000);
    }

    #[test]
    fn test_atomic_cache_metrics() {
        let metrics = AtomicCacheMetrics::new();

        // Record some operations
        metrics.record_get(true, Duration::from_millis(10));
        metrics.record_get(false, Duration::from_millis(20));
        metrics.record_put(1024, Duration::from_millis(5));

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.get_count, 2);
        assert_eq!(snapshot.hit_count, 1);
        assert_eq!(snapshot.miss_count, 1);
        assert_eq!(snapshot.put_count, 1);
        assert_eq!(snapshot.entry_count, 1);
        assert_eq!(snapshot.memory_usage_bytes, 1024);
        assert!(snapshot.avg_get_time > Duration::ZERO);
        assert!(snapshot.avg_put_time > Duration::ZERO);
    }

    #[test]
    fn test_fast_cache_metrics() {
        let metrics = AtomicCacheMetrics::new();

        // Record some operations
        for i in 0..100 {
            metrics.record_get(i % 2 == 0, Duration::from_micros(i as u64));
        }

        let fast_metrics = metrics.fast_snapshot();
        assert_eq!(fast_metrics.get_count, 100);
        assert_eq!(fast_metrics.hit_count, 50);
        assert_eq!(fast_metrics.hit_rate(), 0.5);

        let hit_rate = metrics.fast_hit_rate();
        assert!((hit_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_batch_gets_optimization() {
        let metrics = AtomicCacheMetrics::new();

        let operations = vec![
            (true, Duration::from_micros(100)),
            (false, Duration::from_micros(200)),
            (true, Duration::from_micros(150)),
            (false, Duration::from_micros(250)),
        ];

        metrics.record_batch_gets(&operations);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.get_count, 4);
        assert_eq!(snapshot.hit_count, 2);
        assert_eq!(snapshot.miss_count, 2);
        assert!(snapshot.avg_get_time > Duration::ZERO);
    }

    #[test]
    fn test_cache_aligned_atomics() {
        // Test that cache-aligned atomics work correctly
        let atomic_u64 = CacheAlignedAtomicU64::new(42);
        let atomic_usize = CacheAlignedAtomicUsize::new(1337);

        assert_eq!(atomic_u64.load(Ordering::Relaxed), 42);
        assert_eq!(atomic_usize.load(Ordering::Relaxed), 1337);

        atomic_u64.fetch_add(8, Ordering::Relaxed);
        atomic_usize.fetch_add(663, Ordering::Relaxed);

        assert_eq!(atomic_u64.load(Ordering::Relaxed), 50);
        assert_eq!(atomic_usize.load(Ordering::Relaxed), 2000);
    }

    // ... rest of tests remain the same
}

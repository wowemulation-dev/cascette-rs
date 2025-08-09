//! Cache statistics tracking and reporting
//!
//! This module provides comprehensive statistics tracking for cache operations,
//! including hit/miss ratios, bandwidth savings, and performance metrics.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Cache statistics for tracking performance and effectiveness
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cache hits
    hits: Arc<AtomicU64>,
    /// Total number of cache misses
    misses: Arc<AtomicU64>,
    /// Total number of cache evictions
    evictions: Arc<AtomicU64>,
    /// Total bytes served from cache (bandwidth saved)
    bytes_saved: Arc<AtomicU64>,
    /// Total bytes written to cache
    bytes_written: Arc<AtomicU64>,
    /// Total bytes evicted from cache
    bytes_evicted: Arc<AtomicU64>,
    /// Number of read operations
    read_operations: Arc<AtomicU64>,
    /// Number of write operations
    write_operations: Arc<AtomicU64>,
    /// Number of delete operations
    delete_operations: Arc<AtomicU64>,
    /// Start time for calculating uptime
    start_time: Instant,
}

/// Snapshot of cache statistics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatsSnapshot {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total cache evictions
    pub evictions: u64,
    /// Total bytes saved by cache hits
    pub bytes_saved: u64,
    /// Total bytes written to cache
    pub bytes_written: u64,
    /// Total bytes evicted from cache
    pub bytes_evicted: u64,
    /// Total read operations
    pub read_operations: u64,
    /// Total write operations
    pub write_operations: u64,
    /// Total delete operations
    pub delete_operations: u64,
    /// Cache hit rate as a percentage (0.0 to 100.0)
    pub hit_rate: f64,
    /// Cache miss rate as a percentage (0.0 to 100.0)
    pub miss_rate: f64,
    /// Total cache operations (hits + misses)
    pub total_operations: u64,
    /// Cache uptime in seconds
    pub uptime_seconds: u64,
}

/// Detailed cache performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheReport {
    /// Basic statistics snapshot
    pub stats: CacheStatsSnapshot,
    /// Average bytes per hit
    pub avg_bytes_per_hit: f64,
    /// Average bytes per write
    pub avg_bytes_per_write: f64,
    /// Bandwidth savings ratio (0.0 to 1.0)
    pub bandwidth_savings_ratio: f64,
    /// Operations per second
    pub operations_per_second: f64,
    /// Bytes per second served from cache
    pub bytes_per_second_saved: f64,
    /// Cache effectiveness score (0.0 to 100.0)
    pub effectiveness_score: f64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStats {
    /// Create a new cache statistics tracker
    pub fn new() -> Self {
        Self {
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            evictions: Arc::new(AtomicU64::new(0)),
            bytes_saved: Arc::new(AtomicU64::new(0)),
            bytes_written: Arc::new(AtomicU64::new(0)),
            bytes_evicted: Arc::new(AtomicU64::new(0)),
            read_operations: Arc::new(AtomicU64::new(0)),
            write_operations: Arc::new(AtomicU64::new(0)),
            delete_operations: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    /// Record a cache hit with the number of bytes served
    pub fn record_hit(&self, bytes: u64) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.bytes_saved.fetch_add(bytes, Ordering::Relaxed);
        self.read_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.read_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache eviction with the number of bytes evicted
    pub fn record_eviction(&self, bytes: u64) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
        self.bytes_evicted.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a write operation with the number of bytes written
    pub fn record_write(&self, bytes: u64) {
        self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        self.write_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a delete operation
    pub fn record_delete(&self) {
        self.delete_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current hit count
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get current miss count
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get current eviction count
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    /// Get total bytes saved by cache hits
    pub fn bytes_saved(&self) -> u64 {
        self.bytes_saved.load(Ordering::Relaxed)
    }

    /// Get total bytes written to cache
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Relaxed)
    }

    /// Get total bytes evicted from cache
    pub fn bytes_evicted(&self) -> u64 {
        self.bytes_evicted.load(Ordering::Relaxed)
    }

    /// Calculate hit rate as a percentage (0.0 to 100.0)
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits();
        let total = hits + self.misses();

        if total == 0 {
            0.0
        } else {
            (hits as f64 / total as f64) * 100.0
        }
    }

    /// Calculate miss rate as a percentage (0.0 to 100.0)
    pub fn miss_rate(&self) -> f64 {
        let misses = self.misses();
        let total = self.hits() + misses;

        if total == 0 {
            0.0
        } else {
            (misses as f64 / total as f64) * 100.0
        }
    }

    /// Get total cache operations (hits + misses)
    pub fn total_operations(&self) -> u64 {
        self.hits() + self.misses()
    }

    /// Get cache uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.bytes_saved.store(0, Ordering::Relaxed);
        self.bytes_written.store(0, Ordering::Relaxed);
        self.bytes_evicted.store(0, Ordering::Relaxed);
        self.read_operations.store(0, Ordering::Relaxed);
        self.write_operations.store(0, Ordering::Relaxed);
        self.delete_operations.store(0, Ordering::Relaxed);
    }

    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> CacheStatsSnapshot {
        let hits = self.hits();
        let misses = self.misses();
        let total_ops = hits + misses;
        let uptime_secs = self.uptime().as_secs();

        CacheStatsSnapshot {
            hits,
            misses,
            evictions: self.evictions(),
            bytes_saved: self.bytes_saved(),
            bytes_written: self.bytes_written(),
            bytes_evicted: self.bytes_evicted(),
            read_operations: self.read_operations.load(Ordering::Relaxed),
            write_operations: self.write_operations.load(Ordering::Relaxed),
            delete_operations: self.delete_operations.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
            miss_rate: self.miss_rate(),
            total_operations: total_ops,
            uptime_seconds: uptime_secs,
        }
    }

    /// Generate a comprehensive performance report
    pub fn report(&self) -> CacheReport {
        let stats = self.snapshot();
        let uptime_secs = stats.uptime_seconds as f64;

        // Calculate averages
        let avg_bytes_per_hit = if stats.hits > 0 {
            stats.bytes_saved as f64 / stats.hits as f64
        } else {
            0.0
        };

        let avg_bytes_per_write = if stats.write_operations > 0 {
            stats.bytes_written as f64 / stats.write_operations as f64
        } else {
            0.0
        };

        // Calculate bandwidth savings ratio
        let total_bytes_served = stats.bytes_saved + (stats.misses * avg_bytes_per_hit as u64);
        let bandwidth_savings_ratio = if total_bytes_served > 0 {
            stats.bytes_saved as f64 / total_bytes_served as f64
        } else {
            0.0
        };

        // Calculate rates
        let operations_per_second = if uptime_secs > 0.0 {
            stats.total_operations as f64 / uptime_secs
        } else {
            0.0
        };

        let bytes_per_second_saved = if uptime_secs > 0.0 {
            stats.bytes_saved as f64 / uptime_secs
        } else {
            0.0
        };

        // Calculate effectiveness score (weighted combination of hit rate and bandwidth savings)
        let hit_rate_score = stats.hit_rate;
        let bandwidth_score = bandwidth_savings_ratio * 100.0;
        let effectiveness_score = (hit_rate_score * 0.7) + (bandwidth_score * 0.3);

        CacheReport {
            stats,
            avg_bytes_per_hit,
            avg_bytes_per_write,
            bandwidth_savings_ratio,
            operations_per_second,
            bytes_per_second_saved,
            effectiveness_score,
        }
    }
}

impl CacheStatsSnapshot {
    /// Format bytes as human-readable string (KB, MB, GB)
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }

    /// Format duration as human-readable string
    pub fn format_uptime(&self) -> String {
        let secs = self.uptime_seconds;
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        if days > 0 {
            format!("{days}d {hours}h {minutes}m {seconds}s")
        } else if hours > 0 {
            format!("{hours}h {minutes}m {seconds}s")
        } else if minutes > 0 {
            format!("{minutes}m {seconds}s")
        } else {
            format!("{seconds}s")
        }
    }
}

impl std::fmt::Display for CacheStatsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Cache Statistics:")?;
        writeln!(
            f,
            "  Operations: {} hits, {} misses ({:.1}% hit rate)",
            self.hits, self.misses, self.hit_rate
        )?;
        writeln!(
            f,
            "  Bandwidth: {} saved, {} written",
            Self::format_bytes(self.bytes_saved),
            Self::format_bytes(self.bytes_written)
        )?;
        writeln!(
            f,
            "  Evictions: {} entries ({} bytes)",
            self.evictions,
            Self::format_bytes(self.bytes_evicted)
        )?;
        writeln!(f, "  Uptime: {}", self.format_uptime())?;
        Ok(())
    }
}

impl std::fmt::Display for CacheReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Cache Performance Report:")?;
        writeln!(f, "{}", self.stats)?;
        writeln!(f, "Performance Metrics:")?;
        writeln!(f, "  Average bytes per hit: {:.1}", self.avg_bytes_per_hit)?;
        writeln!(
            f,
            "  Average bytes per write: {:.1}",
            self.avg_bytes_per_write
        )?;
        writeln!(
            f,
            "  Bandwidth savings: {:.1}%",
            self.bandwidth_savings_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Operations per second: {:.1}",
            self.operations_per_second
        )?;
        writeln!(
            f,
            "  Bytes per second saved: {}",
            CacheStatsSnapshot::format_bytes(self.bytes_per_second_saved as u64)
        )?;
        writeln!(
            f,
            "  Effectiveness score: {:.1}/100",
            self.effectiveness_score
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_cache_stats_creation() {
        let stats = CacheStats::new();
        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.evictions(), 0);
        assert_eq!(stats.bytes_saved(), 0);
    }

    #[test]
    fn test_record_operations() {
        let stats = CacheStats::new();

        // Record some hits
        stats.record_hit(1024);
        stats.record_hit(2048);
        assert_eq!(stats.hits(), 2);
        assert_eq!(stats.bytes_saved(), 3072);

        // Record some misses
        stats.record_miss();
        stats.record_miss();
        assert_eq!(stats.misses(), 2);

        // Record evictions
        stats.record_eviction(512);
        assert_eq!(stats.evictions(), 1);
        assert_eq!(stats.bytes_evicted(), 512);

        // Record writes
        stats.record_write(4096);
        assert_eq!(stats.bytes_written(), 4096);
    }

    #[test]
    fn test_hit_miss_rates() {
        let stats = CacheStats::new();

        // No operations - should be 0%
        assert_eq!(stats.hit_rate(), 0.0);
        assert_eq!(stats.miss_rate(), 0.0);

        // 3 hits, 1 miss = 75% hit rate, 25% miss rate
        stats.record_hit(100);
        stats.record_hit(200);
        stats.record_hit(300);
        stats.record_miss();

        assert!((stats.hit_rate() - 75.0).abs() < 0.001);
        assert!((stats.miss_rate() - 25.0).abs() < 0.001);
        assert_eq!(stats.total_operations(), 4);
    }

    #[test]
    fn test_stats_reset() {
        let stats = CacheStats::new();

        // Record some operations
        stats.record_hit(1000);
        stats.record_miss();
        stats.record_eviction(500);
        stats.record_write(2000);

        // Verify values are set
        assert_eq!(stats.hits(), 1);
        assert_eq!(stats.misses(), 1);
        assert_eq!(stats.evictions(), 1);
        assert_eq!(stats.bytes_saved(), 1000);
        assert_eq!(stats.bytes_written(), 2000);

        // Reset and verify all are zero
        stats.reset();
        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.evictions(), 0);
        assert_eq!(stats.bytes_saved(), 0);
        assert_eq!(stats.bytes_written(), 0);
    }

    #[test]
    fn test_snapshot() {
        let stats = CacheStats::new();

        // Record some operations
        stats.record_hit(500);
        stats.record_hit(1500);
        stats.record_miss();
        stats.record_write(1000);
        stats.record_eviction(200);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.hits, 2);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.evictions, 1);
        assert_eq!(snapshot.bytes_saved, 2000);
        assert_eq!(snapshot.bytes_written, 1000);
        assert_eq!(snapshot.bytes_evicted, 200);
        assert!((snapshot.hit_rate - 66.666).abs() < 0.01);
        assert_eq!(snapshot.total_operations, 3);
    }

    #[test]
    fn test_report_generation() {
        let stats = CacheStats::new();

        // Record realistic operations
        stats.record_hit(1024); // 1KB hit
        stats.record_hit(2048); // 2KB hit
        stats.record_hit(4096); // 4KB hit
        stats.record_miss(); // 1 miss
        stats.record_write(8192); // 8KB write

        let report = stats.report();

        // Check basic stats
        assert_eq!(report.stats.hits, 3);
        assert_eq!(report.stats.misses, 1);
        assert_eq!(report.stats.bytes_saved, 7168); // 1KB + 2KB + 4KB

        // Check calculated metrics
        assert!((report.avg_bytes_per_hit - 2389.33).abs() < 0.01); // 7168/3
        assert_eq!(report.avg_bytes_per_write, 8192.0);
        assert!(report.effectiveness_score > 0.0);
        assert!(report.effectiveness_score <= 100.0);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(CacheStatsSnapshot::format_bytes(0), "0 B");
        assert_eq!(CacheStatsSnapshot::format_bytes(512), "512 B");
        assert_eq!(CacheStatsSnapshot::format_bytes(1024), "1.00 KB");
        assert_eq!(CacheStatsSnapshot::format_bytes(1536), "1.50 KB");
        assert_eq!(CacheStatsSnapshot::format_bytes(1048576), "1.00 MB");
        assert_eq!(CacheStatsSnapshot::format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_concurrent_access() {
        let stats = Arc::new(CacheStats::new());
        let mut handles = vec![];

        // Spawn multiple threads to test concurrent access
        for i in 0..10 {
            let stats_clone = Arc::clone(&stats);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    stats_clone.record_hit((i * 100 + j) as u64);
                    stats_clone.record_miss();
                    stats_clone.record_write(1000);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final counts (10 threads * 100 operations each)
        assert_eq!(stats.hits(), 1000);
        assert_eq!(stats.misses(), 1000);
        assert_eq!(stats.bytes_written(), 1000000); // 10 threads * 100 writes * 1000 bytes
        assert!((stats.hit_rate() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_uptime_tracking() {
        let stats = CacheStats::new();

        // Sleep for a short time to get measurable uptime
        thread::sleep(StdDuration::from_millis(10));

        let uptime = stats.uptime();
        assert!(uptime.as_millis() >= 10);

        let snapshot = stats.snapshot();
        // uptime_seconds is u64, so it's always >= 0, just verify it exists
        assert!(snapshot.uptime_seconds < 3600); // Should be less than 1 hour for test
    }

    #[test]
    fn test_display_formatting() {
        let stats = CacheStats::new();
        stats.record_hit(1024);
        stats.record_miss();

        let snapshot = stats.snapshot();
        let display_output = format!("{snapshot}");

        assert!(display_output.contains("Cache Statistics:"));
        assert!(display_output.contains("1 hits, 1 misses"));
        assert!(display_output.contains("1.00 KB saved"));

        let report = stats.report();
        let report_output = format!("{report}");

        assert!(report_output.contains("Cache Performance Report:"));
        assert!(report_output.contains("Performance Metrics:"));
    }
}

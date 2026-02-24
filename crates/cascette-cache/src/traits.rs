//! Core cache traits for NGDP/CASC caching system
//!
//! This module defines the fundamental traits that all cache implementations
//! must implement, providing a consistent interface across different cache
//! backends and strategies.
//!
//! # Platform Support
//!
//! Trait method names and signatures are self-documenting; see per-method
//! doc comments for behavioral notes only.

#![allow(missing_docs)]
//!
//! On native platforms, cache implementations must be `Send + Sync` for
//! concurrent access across threads.
//!
//! On WASM, caches are single-threaded and use browser storage APIs that
//! are not `Send`. The trait uses `#[async_trait(?Send)]` on WASM.

use crate::{error::CacheResult, key::CacheKey, stats::CacheStats};
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Instant is not available on WASM
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

// ============================================================================
// Native platform AsyncCache trait (requires Send + Sync)
// ============================================================================

/// Core async cache trait
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait AsyncCache<K: CacheKey>: Send + Sync {
    /// Returns None if expired.
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>>;

    /// Uses the default TTL configured for the cache.
    async fn put(&self, key: K, value: Bytes) -> CacheResult<()>;

    async fn put_with_ttl(&self, key: K, value: Bytes, ttl: Duration) -> CacheResult<()>;

    /// Returns false for expired entries.
    async fn contains(&self, key: &K) -> CacheResult<bool>;

    /// Returns true if the key was present and removed.
    async fn remove(&self, key: &K) -> CacheResult<bool>;

    async fn clear(&self) -> CacheResult<()>;

    async fn stats(&self) -> CacheResult<CacheStats>;

    /// Entry count, not byte size.
    async fn size(&self) -> CacheResult<usize>;

    async fn is_empty(&self) -> CacheResult<bool> {
        Ok(self.size().await? == 0)
    }
}

// ============================================================================
// WASM platform AsyncCache trait (single-threaded, no Send required)
// ============================================================================

/// Core async cache trait for WASM (single-threaded, no Send/Sync).
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait AsyncCache<K: CacheKey> {
    /// Returns None if expired.
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>>;

    /// Uses the default TTL configured for the cache.
    async fn put(&self, key: K, value: Bytes) -> CacheResult<()>;

    async fn put_with_ttl(&self, key: K, value: Bytes, ttl: Duration) -> CacheResult<()>;

    /// Returns false for expired entries.
    async fn contains(&self, key: &K) -> CacheResult<bool>;

    /// Returns true if the key was present and removed.
    async fn remove(&self, key: &K) -> CacheResult<bool>;

    async fn clear(&self) -> CacheResult<()>;

    async fn stats(&self) -> CacheResult<CacheStats>;

    /// Entry count, not byte size.
    async fn size(&self) -> CacheResult<usize>;

    async fn is_empty(&self) -> CacheResult<bool> {
        Ok(self.size().await? == 0)
    }
}

// ============================================================================
// CacheEntry - Native only (uses std::time::Instant)
// ============================================================================

/// Cache entry metadata. Native only (uses `std::time::Instant`).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry<V> {
    pub value: V,
    pub created_at: Instant,
    /// None for no expiration
    pub expires_at: Option<Instant>,
    pub size_bytes: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl<V> CacheEntry<V> {
    pub fn new(value: V, size_bytes: usize) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            expires_at: None,
            size_bytes,
        }
    }

    pub fn with_ttl(value: V, size_bytes: usize, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            expires_at: Some(now + ttl),
            size_bytes,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires| Instant::now() >= expires)
    }

    pub fn age(&self) -> Duration {
        Instant::now() - self.created_at
    }

    /// Returns None if no expiration set; zero duration if already expired.
    pub fn time_to_live(&self) -> Option<Duration> {
        self.expires_at
            .map(|expires| expires.saturating_duration_since(Instant::now()))
    }
}

// ============================================================================
// InvalidationStrategy - Available on all platforms
// ============================================================================

/// Invalidation strategy for cache entries
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvalidationStrategy {
    Never,
    Ttl(Duration),
    Lru,
    Lfu,
    Size {
        max_entries: usize,
    },
    Memory {
        /// Bytes
        max_bytes: usize,
    },
    Combined(Vec<InvalidationStrategy>),
}

impl InvalidationStrategy {
    /// Lru and Lfu are handled by the cache implementation, not here.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn should_invalidate<V>(
        &self,
        entry: &CacheEntry<V>,
        current_cache_size: usize,
        current_cache_bytes: usize,
    ) -> bool {
        match self {
            Self::Never | Self::Lru | Self::Lfu => false, // These are handled by the cache implementation
            Self::Ttl(_) => entry.is_expired(),
            Self::Size { max_entries } => current_cache_size > *max_entries,
            Self::Memory { max_bytes } => current_cache_bytes > *max_bytes,
            Self::Combined(strategies) => strategies.iter().any(|strategy| {
                strategy.should_invalidate(entry, current_cache_size, current_cache_bytes)
            }),
        }
    }

    pub fn get_ttl(&self) -> Option<Duration> {
        match self {
            Self::Ttl(duration) => Some(*duration),
            Self::Combined(strategies) => strategies.iter().find_map(|s| s.get_ttl()),
            _ => None,
        }
    }
}

impl Default for InvalidationStrategy {
    fn default() -> Self {
        Self::Ttl(Duration::from_secs(3600)) // 1 hour default TTL
    }
}

/// Cache eviction policy
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    #[default]
    Lru,
    Lfu,
    Fifo,
    Random,
    Ttl,
}

// ============================================================================
// Advanced cache traits - Native only (require Send + Sync)
// ============================================================================

/// Pre-populate cache from the underlying data source. Native only.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CacheWarming<K: CacheKey> {
    /// Returns the number of entries loaded.
    async fn warm(&self, keys: Vec<K>) -> CacheResult<usize>;

    fn supports_warming(&self) -> bool {
        true
    }
}

/// Persist cache to disk across restarts. Native only.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CachePersistence {
    async fn save(&self) -> CacheResult<()>;
    async fn load(&self) -> CacheResult<()>;
    fn is_persistence_enabled(&self) -> bool;
}

/// Hierarchical cache with multiple layers (L1, L2, etc.). Native only.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait MultiLayerCache<K: CacheKey>: AsyncCache<K> {
    fn layer_count(&self) -> usize;
    async fn get_from_layer(&self, key: &K, layer: usize) -> CacheResult<Option<Bytes>>;
    async fn put_to_layer(&self, key: K, value: Bytes, layer: usize) -> CacheResult<()>;
    async fn promote(&self, key: &K, from_layer: usize, to_layer: usize) -> CacheResult<bool>;
    async fn layer_stats(&self, layer: usize) -> CacheResult<CacheStats>;
}

/// Cache event notifications. Native only.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CacheListener<K: CacheKey> {
    async fn on_put(&self, key: &K, size_bytes: usize);
    async fn on_get(&self, key: &K, hit: bool);
    async fn on_remove(&self, key: &K, size_bytes: usize);
    async fn on_expire(&self, key: &K, size_bytes: usize);
    async fn on_clear(&self);
}

/// Cache metrics for monitoring. Native only.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CacheMetrics {
    async fn hit_rate(&self) -> f64;

    async fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate().await
    }

    async fn avg_response_time(&self) -> Duration;

    /// 0.0 to 1.0
    async fn utilization(&self) -> f64;

    /// Evictions per second
    async fn eviction_rate(&self) -> f64;
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::task;
    // Import std sleep for timing tests

    #[test]
    fn test_cache_entry_creation() {
        let entry = CacheEntry::new("test value".to_string(), 10);
        assert_eq!(entry.value, "test value");
        assert_eq!(entry.size_bytes, 10);
        assert!(!entry.is_expired());
        assert!(entry.time_to_live().is_none());
    }

    #[test]
    fn test_cache_entry_with_ttl() {
        let ttl = Duration::from_secs(60);
        let entry = CacheEntry::with_ttl("test value".to_string(), 10, ttl);
        assert_eq!(entry.value, "test value");
        assert_eq!(entry.size_bytes, 10);
        assert!(!entry.is_expired());
        assert!(entry.time_to_live().is_some());
    }

    #[test]
    fn test_invalidation_strategy_ttl() {
        let strategy = InvalidationStrategy::Ttl(Duration::from_secs(60));
        assert_eq!(strategy.get_ttl(), Some(Duration::from_secs(60)));

        let entry = CacheEntry::new("test".to_string(), 4);
        assert!(!strategy.should_invalidate(&entry, 10, 100));
    }

    #[test]
    fn test_invalidation_strategy_size() {
        let strategy = InvalidationStrategy::Size { max_entries: 100 };
        let entry = CacheEntry::new("test".to_string(), 4);

        assert!(!strategy.should_invalidate(&entry, 50, 1000));
        assert!(strategy.should_invalidate(&entry, 150, 1000));
    }

    #[test]
    fn test_invalidation_strategy_memory() {
        let strategy = InvalidationStrategy::Memory { max_bytes: 1000 };
        let entry = CacheEntry::new("test".to_string(), 4);

        assert!(!strategy.should_invalidate(&entry, 50, 500));
        assert!(strategy.should_invalidate(&entry, 50, 1500));
    }

    #[test]
    fn test_combined_invalidation_strategy() {
        let strategy = InvalidationStrategy::Combined(vec![
            InvalidationStrategy::Size { max_entries: 100 },
            InvalidationStrategy::Memory { max_bytes: 1000 },
        ]);

        let entry = CacheEntry::new("test".to_string(), 4);

        // Should not invalidate if both conditions are fine
        assert!(!strategy.should_invalidate(&entry, 50, 500));

        // Should invalidate if size limit exceeded
        assert!(strategy.should_invalidate(&entry, 150, 500));

        // Should invalidate if memory limit exceeded
        assert!(strategy.should_invalidate(&entry, 50, 1500));
    }

    #[test]
    fn test_cache_entry_expiration_timing() {
        let ttl = Duration::from_millis(1); // Very short TTL
        let entry = CacheEntry::with_ttl("test".to_string(), 10, ttl);

        // Should not be expired initially
        assert!(!entry.is_expired());

        // Check TTL exists
        let remaining = entry.time_to_live();
        assert!(remaining.is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(2));
        assert!(entry.is_expired());

        // TTL should be zero after expiration
        let remaining_after = entry.time_to_live();
        assert!(remaining_after.is_some());
        assert_eq!(
            remaining_after.expect("Operation should succeed"),
            Duration::ZERO
        );
    }

    #[tokio::test]
    async fn test_concurrent_cache_entry_access() {
        // Test that cache entries can be safely accessed from multiple tasks
        let entry = Arc::new(CacheEntry::with_ttl(
            "shared data".to_string(),
            100,
            Duration::from_secs(3600),
        ));

        let mut tasks = Vec::new();

        for i in 0..10 {
            let entry_clone = Arc::clone(&entry);
            let task = task::spawn(async move {
                // Simulate concurrent access
                for _ in 0..100 {
                    let _is_expired = entry_clone.is_expired();
                    let _age = entry_clone.age();
                    let _ttl = entry_clone.time_to_live();
                    // Access value and size to ensure they're readable under concurrent access
                    assert!(!entry_clone.value.is_empty());
                    assert!(entry_clone.size_bytes > 0);

                    // Small delay to increase chance of race conditions
                    tokio::task::yield_now().await;
                }
                i // Return task number for verification
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task in tasks {
            results.push(task.await);
        }

        // Verify all tasks completed successfully
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok());
            assert_eq!(result.as_ref().expect("Operation should succeed"), &i);
        }

        // Entry should still be valid
        assert!(!entry.is_expired());
        assert_eq!(entry.value, "shared data");
    }

    #[test]
    fn test_invalidation_strategy_concurrent_evaluation() {
        use std::sync::Arc;
        use std::thread;

        let strategy = Arc::new(InvalidationStrategy::Combined(vec![
            InvalidationStrategy::Size { max_entries: 100 },
            InvalidationStrategy::Memory { max_bytes: 1000 },
            InvalidationStrategy::Ttl(Duration::from_secs(3600)),
        ]));

        let entry = Arc::new(CacheEntry::new("test data".to_string(), 50));

        let mut handles = Vec::new();

        // Spawn multiple threads to test concurrent strategy evaluation
        for _ in 0..10 {
            let strategy_clone = Arc::clone(&strategy);
            let entry_clone = Arc::clone(&entry);

            let handle = thread::spawn(move || {
                let mut results = Vec::new();
                for cache_size in 0..200 {
                    for memory_usage in 0..2000 {
                        let should_invalidate = strategy_clone.should_invalidate(
                            &entry_clone,
                            cache_size,
                            memory_usage,
                        );
                        results.push((cache_size, memory_usage, should_invalidate));
                    }
                }
                results
            });

            handles.push(handle);
        }

        // Collect all results
        let mut all_results = Vec::new();
        for handle in handles {
            let thread_results = handle.join().expect("Operation should succeed");
            all_results.push(thread_results);
        }

        // Verify consistency across threads
        let first_results = &all_results[0];
        for other_results in &all_results[1..] {
            assert_eq!(first_results.len(), other_results.len());
            for (first, other) in first_results.iter().zip(other_results.iter()) {
                assert_eq!(first, other, "Inconsistent results across threads");
            }
        }
    }

    #[test]
    fn test_cache_entry_age_progression() {
        let entry = CacheEntry::new("aging test".to_string(), 20);
        let initial_age = entry.age();

        // Age should be small initially
        assert!(initial_age < Duration::from_millis(100));

        // Wait and check age progression
        std::thread::sleep(Duration::from_millis(10));
        let age_after_wait = entry.age();
        assert!(age_after_wait > initial_age);
        assert!(age_after_wait >= Duration::from_millis(10));
    }

    #[test]
    fn test_invalidation_strategy_ttl_edge_cases() {
        // Test with short TTL
        let short_ttl = Duration::from_millis(1);
        let strategy = InvalidationStrategy::Ttl(short_ttl);
        let entry = CacheEntry::with_ttl("short lived".to_string(), 10, short_ttl);

        // Should not be invalidated immediately
        assert!(!strategy.should_invalidate(&entry, 10, 100));

        // Wait for expiration and should be invalidated
        std::thread::sleep(Duration::from_millis(2));
        assert!(strategy.should_invalidate(&entry, 10, 100));
    }

    #[test]
    fn test_cache_entry_without_ttl() {
        let entry = CacheEntry::new("permanent".to_string(), 15);

        // Should never expire
        assert!(!entry.is_expired());
        assert!(entry.time_to_live().is_none());

        // Even after some time
        std::thread::sleep(Duration::from_millis(10));
        assert!(!entry.is_expired());
        assert!(entry.time_to_live().is_none());
    }

    #[test]
    fn test_eviction_policy_completeness() {
        // Ensure all eviction policies can be constructed
        let policies = vec![
            EvictionPolicy::Lru,
            EvictionPolicy::Lfu,
            EvictionPolicy::Fifo,
            EvictionPolicy::Random,
            EvictionPolicy::Ttl,
        ];

        for policy in policies {
            // Test debug formatting
            let debug_str = format!("{policy:?}");
            assert!(!debug_str.is_empty());

            // Test equality
            let cloned_policy = policy.clone();
            assert_eq!(policy, cloned_policy);
        }
    }

    #[test]
    fn test_invalidation_strategy_default() {
        let strategy = InvalidationStrategy::default();
        match strategy {
            InvalidationStrategy::Ttl(duration) => {
                assert_eq!(duration, Duration::from_secs(3600));
            }
            _ => unreachable!("Default should be TTL strategy"),
        }
    }

    #[test]
    fn test_invalidation_strategy_get_ttl() {
        let ttl_duration = Duration::from_secs(1800);

        let strategies = vec![
            (InvalidationStrategy::Ttl(ttl_duration), Some(ttl_duration)),
            (InvalidationStrategy::Never, None),
            (InvalidationStrategy::Lru, None),
            (InvalidationStrategy::Lfu, None),
            (InvalidationStrategy::Size { max_entries: 100 }, None),
            (InvalidationStrategy::Memory { max_bytes: 1000 }, None),
            (
                InvalidationStrategy::Combined(vec![
                    InvalidationStrategy::Ttl(ttl_duration),
                    InvalidationStrategy::Lru,
                ]),
                Some(ttl_duration),
            ),
        ];

        for (strategy, expected_ttl) in strategies {
            assert_eq!(strategy.get_ttl(), expected_ttl);
        }
    }

    #[test]
    fn test_cache_entry_memory_usage() {
        // Test that cache entries properly track their memory usage
        let small_entry = CacheEntry::new("small".to_string(), 5);
        let large_entry = CacheEntry::new("large data".repeat(1000), 1000 * 10);

        assert_eq!(small_entry.size_bytes, 5);
        assert_eq!(large_entry.size_bytes, 10000);

        // Size should remain constant over time
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(small_entry.size_bytes, 5);
        assert_eq!(large_entry.size_bytes, 10000);
    }

    #[test]
    fn test_invalidation_strategy_size_boundary() {
        let strategy = InvalidationStrategy::Size { max_entries: 100 };
        let entry = CacheEntry::new("test".to_string(), 10);

        // Test boundary conditions
        assert!(!strategy.should_invalidate(&entry, 99, 1000)); // Just under limit
        assert!(!strategy.should_invalidate(&entry, 100, 1000)); // At limit
        assert!(strategy.should_invalidate(&entry, 101, 1000)); // Over limit
    }

    #[test]
    fn test_invalidation_strategy_memory_boundary() {
        let strategy = InvalidationStrategy::Memory { max_bytes: 1000 };
        let entry = CacheEntry::new("test".to_string(), 10);

        // Test boundary conditions
        assert!(!strategy.should_invalidate(&entry, 100, 999)); // Just under limit
        assert!(!strategy.should_invalidate(&entry, 100, 1000)); // At limit
        assert!(strategy.should_invalidate(&entry, 100, 1001)); // Over limit
    }

    #[test]
    fn test_cache_entry_instant_precision() {
        // Test that Instant precision is maintained
        let entry1 = CacheEntry::new("first".to_string(), 10);
        let entry2 = CacheEntry::new("second".to_string(), 10);

        // Even though created in quick succession, should have different timestamps
        // (or at least be comparable)
        assert!(entry1.created_at <= entry2.created_at);

        // Age should be deterministic for the same entry
        let age1 = entry1.age();
        let age2 = entry1.age();
        assert!(age2 >= age1); // Second measurement should be >= first
    }
}

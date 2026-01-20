//! Core cache traits for NGDP/CASC caching system
//!
//! This module defines the fundamental traits that all cache implementations
//! must implement, providing a consistent interface across different cache
//! backends and strategies.
//!
//! # Platform Support
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
///
/// Provides the fundamental operations for all cache implementations.
/// Implementations should be thread-safe and support concurrent access.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait AsyncCache<K: CacheKey>: Send + Sync {
    /// Get a value from the cache
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>>;

    /// Put a value into the cache
    ///
    /// The value will be stored with the default TTL configured for the cache.
    async fn put(&self, key: K, value: Bytes) -> CacheResult<()>;

    /// Put a value into the cache with a specific TTL
    ///
    /// The value will expire after the specified duration.
    async fn put_with_ttl(&self, key: K, value: Bytes, ttl: Duration) -> CacheResult<()>;

    /// Check if a key exists in the cache
    ///
    /// Returns `true` if the key exists and has not expired.
    async fn contains(&self, key: &K) -> CacheResult<bool>;

    /// Remove a key from the cache
    ///
    /// Returns `true` if the key was present and removed.
    async fn remove(&self, key: &K) -> CacheResult<bool>;

    /// Clear all entries from the cache
    async fn clear(&self) -> CacheResult<()>;

    /// Get cache statistics
    async fn stats(&self) -> CacheResult<CacheStats>;

    /// Get the current size of the cache (number of entries)
    async fn size(&self) -> CacheResult<usize>;

    /// Check if the cache is empty
    async fn is_empty(&self) -> CacheResult<bool> {
        Ok(self.size().await? == 0)
    }
}

// ============================================================================
// WASM platform AsyncCache trait (single-threaded, no Send required)
// ============================================================================

/// Core async cache trait for WASM
///
/// Provides the fundamental operations for browser-based cache implementations.
/// WASM caches are single-threaded and don't require Send/Sync bounds.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait AsyncCache<K: CacheKey> {
    /// Get a value from the cache
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>>;

    /// Put a value into the cache
    ///
    /// The value will be stored with the default TTL configured for the cache.
    async fn put(&self, key: K, value: Bytes) -> CacheResult<()>;

    /// Put a value into the cache with a specific TTL
    ///
    /// The value will expire after the specified duration.
    async fn put_with_ttl(&self, key: K, value: Bytes, ttl: Duration) -> CacheResult<()>;

    /// Check if a key exists in the cache
    ///
    /// Returns `true` if the key exists and has not expired.
    async fn contains(&self, key: &K) -> CacheResult<bool>;

    /// Remove a key from the cache
    ///
    /// Returns `true` if the key was present and removed.
    async fn remove(&self, key: &K) -> CacheResult<bool>;

    /// Clear all entries from the cache
    async fn clear(&self) -> CacheResult<()>;

    /// Get cache statistics
    async fn stats(&self) -> CacheResult<CacheStats>;

    /// Get the current size of the cache (number of entries)
    async fn size(&self) -> CacheResult<usize>;

    /// Check if the cache is empty
    async fn is_empty(&self) -> CacheResult<bool> {
        Ok(self.size().await? == 0)
    }
}

// ============================================================================
// CacheEntry - Native only (uses std::time::Instant)
// ============================================================================

/// Cache entry metadata
///
/// Contains information about when an entry was created and when it expires.
/// Only available on native platforms (uses std::time::Instant).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry<V> {
    /// The cached value
    pub value: V,
    /// When the entry was created
    pub created_at: Instant,
    /// When the entry expires (None for no expiration)
    pub expires_at: Option<Instant>,
    /// Size of the entry in bytes
    pub size_bytes: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl<V> CacheEntry<V> {
    /// Create a new cache entry
    pub fn new(value: V, size_bytes: usize) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            expires_at: None,
            size_bytes,
        }
    }

    /// Create a new cache entry with TTL
    pub fn with_ttl(value: V, size_bytes: usize, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            expires_at: Some(now + ttl),
            size_bytes,
        }
    }

    /// Check if the entry has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires| Instant::now() >= expires)
    }

    /// Get the age of the entry
    pub fn age(&self) -> Duration {
        Instant::now() - self.created_at
    }

    /// Get time remaining until expiration
    /// Returns None if the entry doesn't expire, or Some(duration) where
    /// duration may be zero if already expired
    pub fn time_to_live(&self) -> Option<Duration> {
        self.expires_at
            .map(|expires| expires.saturating_duration_since(Instant::now()))
    }
}

// ============================================================================
// InvalidationStrategy - Available on all platforms
// ============================================================================

/// Invalidation strategy for cache entries
///
/// Defines when and how cache entries should be invalidated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvalidationStrategy {
    /// No automatic invalidation
    Never,
    /// Time-based invalidation with TTL
    Ttl(Duration),
    /// Least Recently Used eviction
    Lru,
    /// Least Frequently Used eviction
    Lfu,
    /// Size-based eviction when cache grows too large
    Size {
        /// Maximum number of entries
        max_entries: usize,
    },
    /// Memory-based eviction when cache uses too much memory
    Memory {
        /// Maximum memory usage in bytes
        max_bytes: usize,
    },
    /// Combined strategy using multiple criteria
    Combined(Vec<InvalidationStrategy>),
}

impl InvalidationStrategy {
    /// Check if an entry should be invalidated based on this strategy
    /// Only available on native platforms (requires CacheEntry with Instant).
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

    /// Get the TTL for this strategy, if applicable
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
///
/// Determines which entries to remove when the cache needs space.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    #[default]
    Lru,
    /// Least Frequently Used
    Lfu,
    /// First In, First Out
    Fifo,
    /// Random eviction
    Random,
    /// Time-based (oldest first)
    Ttl,
}

// ============================================================================
// Advanced cache traits - Native only (require Send + Sync)
// ============================================================================

/// Cache warming trait
///
/// Allows caches to be pre-populated with data to improve initial performance.
/// Only available on native platforms.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CacheWarming<K: CacheKey> {
    /// Warm the cache with a set of keys
    ///
    /// The cache should attempt to load data for these keys from the
    /// underlying data source.
    async fn warm(&self, keys: Vec<K>) -> CacheResult<usize>;

    /// Check if the cache supports warming
    fn supports_warming(&self) -> bool {
        true
    }
}

/// Cache persistence trait
///
/// Allows caches to persist data to disk for durability across restarts.
/// Only available on native platforms.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CachePersistence {
    /// Save cache state to persistent storage
    async fn save(&self) -> CacheResult<()>;

    /// Load cache state from persistent storage
    async fn load(&self) -> CacheResult<()>;

    /// Check if persistence is enabled
    fn is_persistence_enabled(&self) -> bool;
}

/// Multi-layer cache trait
///
/// Supports hierarchical caching with multiple layers (L1, L2, etc.).
/// Only available on native platforms.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait MultiLayerCache<K: CacheKey>: AsyncCache<K> {
    /// Get the number of cache layers
    fn layer_count(&self) -> usize;

    /// Get a value from a specific cache layer
    async fn get_from_layer(&self, key: &K, layer: usize) -> CacheResult<Option<Bytes>>;

    /// Put a value into a specific cache layer
    async fn put_to_layer(&self, key: K, value: Bytes, layer: usize) -> CacheResult<()>;

    /// Promote a value from a lower layer to a higher layer
    async fn promote(&self, key: &K, from_layer: usize, to_layer: usize) -> CacheResult<bool>;

    /// Get statistics for a specific layer
    async fn layer_stats(&self, layer: usize) -> CacheResult<CacheStats>;
}

/// Cache listener trait
///
/// Allows external code to be notified of cache events.
/// Only available on native platforms.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CacheListener<K: CacheKey> {
    /// Called when an entry is added to the cache
    async fn on_put(&self, key: &K, size_bytes: usize);

    /// Called when an entry is retrieved from the cache
    async fn on_get(&self, key: &K, hit: bool);

    /// Called when an entry is removed from the cache
    async fn on_remove(&self, key: &K, size_bytes: usize);

    /// Called when an entry expires
    async fn on_expire(&self, key: &K, size_bytes: usize);

    /// Called when the cache is cleared
    async fn on_clear(&self);
}

/// Cache metrics trait
///
/// Provides detailed metrics for monitoring and debugging.
/// Only available on native platforms.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait CacheMetrics {
    /// Get hit rate (hits / total requests)
    async fn hit_rate(&self) -> f64;

    /// Get miss rate (misses / total requests)
    async fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate().await
    }

    /// Get average response time for cache operations
    async fn avg_response_time(&self) -> Duration;

    /// Get cache utilization (used space / total space)
    /// Returns a value between 0.0 and 1.0
    async fn utilization(&self) -> f64;

    /// Get eviction rate (evictions per second)
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

        // Age should be very small initially
        assert!(initial_age < Duration::from_millis(100));

        // Wait and check age progression
        std::thread::sleep(Duration::from_millis(10));
        let age_after_wait = entry.age();
        assert!(age_after_wait > initial_age);
        assert!(age_after_wait >= Duration::from_millis(10));
    }

    #[test]
    fn test_invalidation_strategy_ttl_edge_cases() {
        // Test with very short TTL
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

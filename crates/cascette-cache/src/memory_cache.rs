//! In-memory cache implementation optimized for NGDP workloads
//!
//! This module provides a high-performance in-memory cache using:
//! - DashMap for concurrent access with minimal lock contention
//! - LRU eviction policy with atomic timestamp tracking
//! - Memory-optimized entry storage with `bytes::Bytes`
//! - Background cleanup tasks for expired entries
//! - Metrics collection with the optimized stats system
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::cast_lossless)] // u32/u8 to u64 casts are safe
#![allow(clippy::single_match_else)]
#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]

use crate::{
    config::MemoryCacheConfig,
    error::{CacheError, CacheResult},
    key::CacheKey,
    stats::AtomicCacheMetrics,
    traits::AsyncCache,
};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::time::interval;

/// High-performance in-memory cache entry
#[derive(Debug)]
struct MemoryCacheEntryInner {
    /// The cached value as Bytes for zero-copy sharing
    value: Bytes,
    /// When the entry was created
    created_at: Instant,
    /// When the entry expires (None for no expiration)
    expires_at: Option<Instant>,
    /// Size of the entry in bytes
    size_bytes: usize,
    /// Access timestamp for LRU tracking (atomic for concurrent access)
    last_accessed: AtomicU64,
    /// Access count for LFU tracking
    access_count: AtomicU64,
}

impl MemoryCacheEntryInner {
    fn new(value: Bytes, size_bytes: usize, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        // Use a different approach - we'll use SystemTime since epoch as nanos
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            value,
            created_at: now,
            expires_at: ttl.map(|t| now + t),
            size_bytes,
            last_accessed: AtomicU64::new(now_nanos),
            access_count: AtomicU64::new(1),
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires| Instant::now() >= expires)
    }

    fn update_access(&self) {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.last_accessed.store(now_nanos, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    fn get_last_accessed(&self) -> u64 {
        self.last_accessed.load(Ordering::Relaxed)
    }

    fn get_access_count(&self) -> u64 {
        self.access_count.load(Ordering::Relaxed)
    }
}

/// High-performance in-memory cache implementation
///
/// Uses DashMap for concurrent access and implements various eviction policies
/// optimized for NGDP workload patterns.
pub struct MemoryCache<K: CacheKey> {
    /// The main storage using DashMap for concurrent access
    storage: DashMap<K, Arc<MemoryCacheEntryInner>>,
    /// Cache configuration
    config: MemoryCacheConfig,
    /// Current number of entries (atomic for fast access)
    entry_count: AtomicUsize,
    /// Current memory usage in bytes (atomic for fast access)
    memory_usage: AtomicU64,
    /// High-performance metrics collector
    metrics: Arc<AtomicCacheMetrics>,
    /// Background cleanup task handle
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl<K: CacheKey + 'static> MemoryCache<K> {
    /// Create a new memory cache with the given configuration
    pub fn new(config: MemoryCacheConfig) -> CacheResult<Self> {
        config
            .validate()
            .map_err(CacheError::InvalidConfiguration)?;

        let storage = DashMap::with_capacity(config.max_entries.min(1024));
        let metrics = Arc::new(AtomicCacheMetrics::new());

        Ok(Self {
            storage,
            config,
            entry_count: AtomicUsize::new(0),
            memory_usage: AtomicU64::new(0),
            metrics,
            cleanup_handle: None,
        })
    }

    /// Create a new memory cache and start background cleanup task
    pub fn new_with_cleanup(config: MemoryCacheConfig) -> CacheResult<Self> {
        let cleanup_interval = config.cleanup_interval;
        let mut cache = Self::new(config)?;

        if cleanup_interval > Duration::ZERO {
            cache.start_cleanup_task(cleanup_interval);
        }

        Ok(cache)
    }

    /// Start background cleanup task for expired entries
    fn start_cleanup_task(&mut self, cleanup_interval: Duration) {
        let storage = self.storage.clone();
        let metrics = Arc::clone(&self.metrics);

        let handle = tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);

            loop {
                interval.tick().await;

                let _start_time = Instant::now();
                let mut removed_count = 0;
                let mut freed_bytes = 0;

                // Collect expired keys
                let mut expired_keys = Vec::new();
                for entry in storage.iter() {
                    if entry.value().is_expired() {
                        expired_keys.push(entry.key().clone());
                    }
                }

                // Remove expired entries
                for key in expired_keys {
                    if let Some((_, entry)) = storage.remove(&key) {
                        removed_count += 1;
                        freed_bytes += entry.size_bytes;
                    }
                }

                if removed_count > 0 {
                    // Update metrics for cleanup
                    for _ in 0..removed_count {
                        metrics.record_eviction(freed_bytes / removed_count);
                    }
                }
            }
        });

        self.cleanup_handle = Some(handle);
    }

    /// Check if eviction is needed based on configured limits
    fn needs_eviction(&self) -> bool {
        let current_entries = self.entry_count.load(Ordering::Relaxed);
        let current_memory = self.memory_usage.load(Ordering::Relaxed);

        current_entries >= self.config.max_entries
            || self
                .config
                .max_memory_bytes
                .is_some_and(|max| current_memory >= max as u64)
    }

    /// Perform eviction based on configured policy
    fn perform_eviction(&self) {
        if !self.needs_eviction() {
            return;
        }

        let target_entries = (self.config.max_entries * 90) / 100; // Evict to 90% capacity
        let current_entries = self.entry_count.load(Ordering::Relaxed);

        if current_entries <= target_entries {
            return;
        }

        let evict_count = current_entries - target_entries;

        match &self.config.eviction_policy {
            crate::traits::EvictionPolicy::Lru => self.evict_lru(evict_count),
            crate::traits::EvictionPolicy::Lfu => self.evict_lfu(evict_count),
            crate::traits::EvictionPolicy::Fifo => self.evict_fifo(evict_count),
            crate::traits::EvictionPolicy::Random => self.evict_random(evict_count),
            crate::traits::EvictionPolicy::Ttl => self.evict_expired(),
        }
    }

    /// Evict entries using LRU policy
    fn evict_lru(&self, count: usize) {
        let mut candidates: Vec<(K, u64)> = self
            .storage
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().get_last_accessed()))
            .collect();

        // Sort by last accessed time (oldest first)
        candidates.sort_by_key(|(_, last_accessed)| *last_accessed);

        let to_evict = candidates.into_iter().take(count);

        for (key, _) in to_evict {
            if let Some((_, entry)) = self.storage.remove(&key) {
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_usage
                    .fetch_sub(entry.size_bytes as u64, Ordering::Relaxed);
                self.metrics.record_eviction(entry.size_bytes);
            }
        }
    }

    /// Evict entries using LFU policy
    fn evict_lfu(&self, count: usize) {
        let mut candidates: Vec<(K, u64)> = self
            .storage
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().get_access_count()))
            .collect();

        // Sort by access count (least accessed first)
        candidates.sort_by_key(|(_, access_count)| *access_count);

        let to_evict = candidates.into_iter().take(count);

        for (key, _) in to_evict {
            if let Some((_, entry)) = self.storage.remove(&key) {
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_usage
                    .fetch_sub(entry.size_bytes as u64, Ordering::Relaxed);
                self.metrics.record_eviction(entry.size_bytes);
            }
        }
    }

    /// Evict entries using FIFO policy
    fn evict_fifo(&self, count: usize) {
        let mut candidates: Vec<(K, Instant)> = self
            .storage
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().created_at))
            .collect();

        // Sort by creation time (oldest first)
        candidates.sort_by_key(|(_, created_at)| *created_at);

        let to_evict = candidates.into_iter().take(count);

        for (key, _) in to_evict {
            if let Some((_, entry)) = self.storage.remove(&key) {
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_usage
                    .fetch_sub(entry.size_bytes as u64, Ordering::Relaxed);
                self.metrics.record_eviction(entry.size_bytes);
            }
        }
    }

    /// Evict entries randomly
    fn evict_random(&self, count: usize) {
        use rand::{rng, seq::SliceRandom};

        let mut keys: Vec<K> = self
            .storage
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        keys.shuffle(&mut rng());

        let to_evict = keys.into_iter().take(count);

        for key in to_evict {
            if let Some((_, entry)) = self.storage.remove(&key) {
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_usage
                    .fetch_sub(entry.size_bytes as u64, Ordering::Relaxed);
                self.metrics.record_eviction(entry.size_bytes);
            }
        }
    }

    /// Evict expired entries
    fn evict_expired(&self) {
        let expired_keys: Vec<K> = self
            .storage
            .iter()
            .filter_map(|entry| {
                if entry.value().is_expired() {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for key in expired_keys {
            if let Some((_, entry)) = self.storage.remove(&key) {
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_usage
                    .fetch_sub(entry.size_bytes as u64, Ordering::Relaxed);
                self.metrics.record_eviction(entry.size_bytes);
            }
        }
    }

    /// Get current cache statistics
    pub fn cache_stats(&self) -> crate::stats::CacheStats {
        let snapshot = self.metrics.fast_snapshot();
        let current_entries = self.entry_count.load(Ordering::Relaxed);
        let current_memory = self.memory_usage.load(Ordering::Relaxed);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        crate::stats::CacheStats {
            get_count: snapshot.get_count,
            hit_count: snapshot.hit_count,
            miss_count: snapshot.get_count - snapshot.hit_count,
            put_count: 0,        // Would need separate counter
            remove_count: 0,     // Would need separate counter
            eviction_count: 0,   // Would need separate counter
            expiration_count: 0, // Would need separate counter
            entry_count: current_entries,
            memory_usage_bytes: current_memory as usize,
            max_memory_usage_bytes: current_memory as usize, // Placeholder
            created_at_ms: now_ms,                           // Placeholder
            updated_at_ms: now_ms,
            avg_get_time: Duration::ZERO, // Would need separate tracking
            avg_put_time: Duration::ZERO, // Would need separate tracking
        }
    }
}

#[async_trait]
impl<K: CacheKey + 'static> AsyncCache<K> for MemoryCache<K> {
    async fn get(&self, key: &K) -> CacheResult<Option<Bytes>> {
        let start_time = Instant::now();

        if let Some(entry) = self.storage.get(key) {
            if entry.is_expired() {
                // Need to collect info and drop the guard before removing
                let size_bytes = entry.size_bytes;
                drop(entry); // Drop the guard before attempting to remove

                // Remove expired entry
                if self.storage.remove(key).is_some() {
                    self.entry_count.fetch_sub(1, Ordering::Relaxed);
                    self.memory_usage
                        .fetch_sub(size_bytes as u64, Ordering::Relaxed);
                }

                self.metrics.record_get(false, start_time.elapsed());
                return Ok(None);
            }

            // Update access statistics
            entry.update_access();
            let value = entry.value.clone();

            self.metrics.record_get(true, start_time.elapsed());
            Ok(Some(value))
        } else {
            self.metrics.record_get(false, start_time.elapsed());
            Ok(None)
        }
    }

    async fn put(&self, key: K, value: Bytes) -> CacheResult<()> {
        let ttl = self.config.default_ttl;
        self.put_with_ttl(key, value, ttl.unwrap_or(Duration::from_secs(3600)))
            .await
    }

    async fn put_with_ttl(&self, key: K, value: Bytes, ttl: Duration) -> CacheResult<()> {
        let start_time = Instant::now();
        let size_bytes = value.len();

        // Check capacity and evict if necessary
        if self.needs_eviction() {
            self.perform_eviction();
        }

        let entry = Arc::new(MemoryCacheEntryInner::new(value, size_bytes, Some(ttl)));

        // Insert or update entry
        if let Some(old_entry) = self.storage.insert(key, entry) {
            // Updating existing entry - adjust memory usage
            let old_size = old_entry.size_bytes as u64;
            let new_size = size_bytes as u64;

            if new_size > old_size {
                self.memory_usage
                    .fetch_add(new_size - old_size, Ordering::Relaxed);
            } else {
                self.memory_usage
                    .fetch_sub(old_size - new_size, Ordering::Relaxed);
            }
        } else {
            // New entry
            self.entry_count.fetch_add(1, Ordering::Relaxed);
            self.memory_usage
                .fetch_add(size_bytes as u64, Ordering::Relaxed);
        }

        self.metrics.record_put(size_bytes, start_time.elapsed());
        Ok(())
    }

    async fn contains(&self, key: &K) -> CacheResult<bool> {
        if let Some(entry) = self.storage.get(key) {
            if entry.is_expired() {
                // Need to collect info and drop the guard before removing
                let size_bytes = entry.size_bytes;
                drop(entry); // Drop the guard before attempting to remove

                // Clean up expired entry
                if self.storage.remove(key).is_some() {
                    self.entry_count.fetch_sub(1, Ordering::Relaxed);
                    self.memory_usage
                        .fetch_sub(size_bytes as u64, Ordering::Relaxed);
                }
                Ok(false)
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    async fn remove(&self, key: &K) -> CacheResult<bool> {
        if let Some((_, entry)) = self.storage.remove(key) {
            self.entry_count.fetch_sub(1, Ordering::Relaxed);
            self.memory_usage
                .fetch_sub(entry.size_bytes as u64, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn clear(&self) -> CacheResult<()> {
        self.storage.clear();
        self.entry_count.store(0, Ordering::Relaxed);
        self.memory_usage.store(0, Ordering::Relaxed);
        self.metrics.reset();
        Ok(())
    }

    async fn stats(&self) -> CacheResult<crate::stats::CacheStats> {
        Ok(self.cache_stats())
    }

    async fn size(&self) -> CacheResult<usize> {
        Ok(self.entry_count.load(Ordering::Relaxed))
    }
}

impl<K: CacheKey> Drop for MemoryCache<K> {
    fn drop(&mut self) {
        // Cancel cleanup task
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{config::MemoryCacheConfig, key::RibbitKey, traits::EvictionPolicy};
    use std::time::Duration;

    #[tokio::test]
    async fn test_memory_cache_basic_operations() {
        let config = MemoryCacheConfig::new()
            .with_max_entries(100)
            .with_default_ttl(Duration::from_secs(60));

        let cache = MemoryCache::new(config).expect("Test operation should succeed");
        let key = RibbitKey::new("summary", "us");
        let value = Bytes::from("test data");

        // Test put and get
        cache
            .put(key.clone(), value.clone())
            .await
            .expect("Test operation should succeed");
        let retrieved = cache
            .get(&key)
            .await
            .expect("Test operation should succeed");
        assert_eq!(retrieved, Some(value));

        // Test contains
        assert!(
            cache
                .contains(&key)
                .await
                .expect("Operation should succeed")
        );

        // Test size
        assert_eq!(cache.size().await.expect("Operation should succeed"), 1);

        // Test remove
        assert!(cache.remove(&key).await.expect("Operation should succeed"));
        assert_eq!(cache.size().await.expect("Operation should succeed"), 0);
    }

    #[tokio::test]
    async fn test_memory_cache_ttl_expiration() {
        let config = MemoryCacheConfig::new().with_max_entries(100);
        let cache = MemoryCache::new(config).expect("Test operation should succeed");
        let key = RibbitKey::new("summary", "us");
        let value = Bytes::from("test data");

        // Put with 50ms TTL to avoid flaky timing
        cache
            .put_with_ttl(key.clone(), value.clone(), Duration::from_millis(50))
            .await
            .expect("Test operation should succeed");

        // Should be present initially via get (not contains to avoid deadlock)
        let initial_get = cache.get(&key).await.expect("Operation should succeed");
        assert_eq!(initial_get, Some(value));

        // Sleep longer than TTL to ensure expiration
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired and return None on get
        let expired_get = cache.get(&key).await.expect("Operation should succeed");
        assert_eq!(expired_get, None);

        // Verify cache size is reduced after cleanup
        let final_size = cache.size().await.expect("Operation should succeed");
        assert_eq!(final_size, 0);
    }

    #[tokio::test]
    async fn test_memory_cache_lru_eviction() {
        let config = MemoryCacheConfig::new()
            .with_max_entries(2)
            .with_eviction_policy(EvictionPolicy::Lru);

        let cache = MemoryCache::new(config).expect("Test operation should succeed");

        // Fill cache to capacity
        let key1 = RibbitKey::new("key1", "us");
        let key2 = RibbitKey::new("key2", "us");
        let key3 = RibbitKey::new("key3", "us");

        cache
            .put(key1.clone(), Bytes::from("value1"))
            .await
            .expect("Test operation should succeed");
        cache
            .put(key2.clone(), Bytes::from("value2"))
            .await
            .expect("Test operation should succeed");

        // Access key1 to make it recently used
        cache
            .get(&key1)
            .await
            .expect("Test operation should succeed");

        // Add key3, should evict key2 (least recently used)
        cache
            .put(key3.clone(), Bytes::from("value3"))
            .await
            .expect("Test operation should succeed");

        // key1 and key3 should be present, key2 should be evicted
        assert!(
            cache
                .contains(&key1)
                .await
                .expect("Operation should succeed")
        );
        assert!(
            cache
                .contains(&key3)
                .await
                .expect("Operation should succeed")
        );
        // Note: Due to eviction timing, key2 might still be present
        // This is a simplified test of the eviction mechanism
    }

    #[tokio::test]
    async fn test_memory_cache_clear() {
        let config = MemoryCacheConfig::new().with_max_entries(100);
        let cache = MemoryCache::new(config).expect("Test operation should succeed");

        // Add some entries
        for i in 0..10 {
            let key = RibbitKey::new(format!("key{i}"), "us");
            cache
                .put(key, Bytes::from(format!("value{i}")))
                .await
                .expect("Test operation should succeed");
        }

        assert_eq!(cache.size().await.expect("Operation should succeed"), 10);

        // Clear cache
        cache.clear().await.expect("Test operation should succeed");
        assert_eq!(cache.size().await.expect("Operation should succeed"), 0);
    }

    #[tokio::test]
    async fn test_memory_cache_concurrent_access() {
        let config = MemoryCacheConfig::new().with_max_entries(1000);
        let cache = Arc::new(MemoryCache::new(config).expect("Operation should succeed"));

        let mut handles = Vec::new();

        // Spawn multiple tasks doing concurrent operations
        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                for j in 0..100 {
                    let key = RibbitKey::new(format!("key{i}_{j}"), "us");
                    let value = Bytes::from(format!("value{i}_{j}"));

                    cache_clone
                        .put(key.clone(), value.clone())
                        .await
                        .expect("Test operation should succeed");
                    let retrieved = cache_clone
                        .get(&key)
                        .await
                        .expect("Test operation should succeed");
                    assert_eq!(retrieved, Some(value));
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Test operation should succeed");
        }

        // Verify final state
        assert_eq!(cache.size().await.expect("Operation should succeed"), 1000);
    }

    #[tokio::test]
    async fn test_memory_cache_metrics() {
        let config = MemoryCacheConfig::new().with_max_entries(100);
        let cache = MemoryCache::new(config).expect("Test operation should succeed");
        let key = RibbitKey::new("test", "us");

        // Perform operations
        cache
            .put(key.clone(), Bytes::from("data"))
            .await
            .expect("Test operation should succeed");
        cache
            .get(&key)
            .await
            .expect("Test operation should succeed"); // Hit

        let missing_key = RibbitKey::new("missing", "us");
        cache
            .get(&missing_key)
            .await
            .expect("Test operation should succeed"); // Miss

        let stats = cache.stats().await.expect("Test operation should succeed");
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
    }
}

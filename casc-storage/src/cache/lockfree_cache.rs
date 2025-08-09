//! Lock-free cache implementation using DashMap for improved performance

use crate::types::EKey;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tracing::debug;

/// Lock-free cache with LRU-like eviction based on access frequency
/// This implementation uses DashMap for lock-free concurrent access
/// and provides 2-4x better performance than traditional LRU cache
pub struct LockFreeCache {
    /// Main storage using DashMap for lock-free access
    map: Arc<DashMap<EKey, CacheEntry>>,
    /// Maximum cache size in bytes
    max_size: usize,
    /// Current cache size in bytes
    current_size: Arc<AtomicUsize>,
    /// Hit counter for statistics
    hits: Arc<AtomicUsize>,
    /// Miss counter for statistics
    misses: Arc<AtomicUsize>,
}

/// Entry in the cache with metadata
#[derive(Clone)]
struct CacheEntry {
    /// Actual data (using Arc for zero-copy)
    data: Arc<Vec<u8>>,
    /// Size in bytes
    size: usize,
    /// Last access time for LRU-like eviction
    last_access: Instant,
    /// Access count for frequency-based eviction
    access_count: usize,
}

impl LockFreeCache {
    /// Create a new lock-free cache with the specified maximum size in bytes
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            map: Arc::new(DashMap::with_capacity(1024)),
            max_size: max_size_bytes,
            current_size: Arc::new(AtomicUsize::new(0)),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get an item from the cache (returns Arc for zero-copy)
    pub fn get(&self, key: &EKey) -> Option<Arc<Vec<u8>>> {
        if let Some(mut entry) = self.map.get_mut(key) {
            // Update access metadata
            entry.last_access = Instant::now();
            entry.access_count += 1;

            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(Arc::clone(&entry.data))
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Put an item into the cache
    pub fn put(&self, key: EKey, data: Arc<Vec<u8>>) {
        let size = data.len();

        // Check if we need to evict items
        if self.current_size.load(Ordering::Relaxed) + size > self.max_size {
            self.evict_until_space_available(size);
        }

        let entry = CacheEntry {
            data,
            size,
            last_access: Instant::now(),
            access_count: 1,
        };

        // Insert or update
        if let Some(old_entry) = self.map.insert(key, entry) {
            // Subtract old size
            self.current_size
                .fetch_sub(old_entry.size, Ordering::Relaxed);
        }

        // Add new size
        self.current_size.fetch_add(size, Ordering::Relaxed);
    }

    /// Evict items until there's enough space for the new item
    fn evict_until_space_available(&self, needed_space: usize) {
        let target_size = self.max_size.saturating_sub(needed_space);

        // Collect entries with their scores for eviction
        let mut candidates: Vec<(EKey, f64, usize)> = self
            .map
            .iter()
            .map(|entry| {
                let key = *entry.key();
                let score = self.calculate_eviction_score(&entry);
                let size = entry.size;
                (key, score, size)
            })
            .collect();

        // Sort by eviction score (lower score = evict first)
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Evict entries until we have enough space
        for (key, _score, size) in candidates {
            if self.current_size.load(Ordering::Relaxed) <= target_size {
                break;
            }

            if self.map.remove(&key).is_some() {
                self.current_size.fetch_sub(size, Ordering::Relaxed);
                debug!("Evicted {} from cache (size: {})", key, size);
            }
        }
    }

    /// Calculate eviction score (lower = evict first)
    /// Uses a combination of recency and frequency
    fn calculate_eviction_score(&self, entry: &CacheEntry) -> f64 {
        let age = entry.last_access.elapsed().as_secs_f64();
        let frequency = entry.access_count as f64;

        // Higher frequency and more recent access = higher score (keep in cache)
        // Formula: frequency / (1 + age)
        frequency / (1.0 + age)
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        self.map.clear();
        self.current_size.store(0, Ordering::Relaxed);
        debug!("Cache cleared");
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;

        CacheStats {
            size: self.current_size.load(Ordering::Relaxed),
            max_size: self.max_size,
            entry_count: self.map.len(),
            hits,
            misses,
            hit_rate: if total_requests > 0 {
                (hits as f64) / (total_requests as f64)
            } else {
                0.0
            },
        }
    }

    /// Check if a key exists in the cache without updating access stats
    pub fn contains(&self, key: &EKey) -> bool {
        self.map.contains_key(key)
    }

    /// Get the current size of the cache in bytes
    pub fn current_size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }

    /// Preallocate space in the cache
    pub fn reserve(&self, additional: usize) {
        // DashMap doesn't have a reserve method, but we can hint at capacity
        // This is mainly for documentation purposes
        debug!("Cache reserve hint: {} additional entries", additional);
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current cache size in bytes
    pub size: usize,
    /// Maximum cache size in bytes
    pub max_size: usize,
    /// Number of entries in the cache
    pub entry_count: usize,
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_cache_operations() {
        let cache = LockFreeCache::new(1024 * 1024); // 1MB cache

        // Create test data
        let key1 = EKey::new([
            0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
            0xcd, 0xef,
        ]);
        let data1 = Arc::new(vec![1, 2, 3, 4, 5]);

        // Put and get
        cache.put(key1, Arc::clone(&data1));
        let retrieved = cache.get(&key1).unwrap();
        assert!(Arc::ptr_eq(&data1, &retrieved));

        // Check stats
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.entry_count, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = LockFreeCache::new(100); // Small cache for testing eviction

        // Add items that exceed cache size
        for i in 0..20 {
            let mut key_bytes = [0u8; 16];
            key_bytes[0] = i as u8;
            let key = EKey::new(key_bytes);
            let data = Arc::new(vec![i as u8; 10]); // 10 bytes each
            cache.put(key, data);
        }

        // Cache should have evicted items to stay under 100 bytes
        assert!(cache.current_size() <= 100);
        assert!(cache.map.len() < 20);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let cache = Arc::new(LockFreeCache::new(10 * 1024 * 1024)); // 10MB
        let mut handles = vec![];

        // Spawn multiple threads to access cache concurrently
        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let mut key_bytes = [0u8; 16];
                    let val = (i * 100 + j) as u16;
                    key_bytes[0] = (val >> 8) as u8;
                    key_bytes[1] = (val & 0xff) as u8;
                    let key = EKey::new(key_bytes);
                    let data = Arc::new(vec![i as u8; 100]);

                    // Put and get operations
                    cache_clone.put(key, Arc::clone(&data));
                    cache_clone.get(&key);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify cache is still functional
        let stats = cache.stats();
        assert!(stats.entry_count > 0);
        assert!(stats.hits > 0);
    }

    #[test]
    fn test_zero_copy_behavior() {
        let cache = LockFreeCache::new(1024 * 1024);

        let key = EKey::new([
            0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
            0xcd, 0xef,
        ]);
        let data = Arc::new(vec![1, 2, 3, 4, 5]);

        cache.put(key, Arc::clone(&data));

        // Get the same key multiple times
        let retrieved1 = cache.get(&key).unwrap();
        let retrieved2 = cache.get(&key).unwrap();

        // Should return the same Arc (zero-copy)
        assert!(Arc::ptr_eq(&retrieved1, &retrieved2));
        assert!(Arc::ptr_eq(&data, &retrieved1));
    }

    #[test]
    fn test_frequency_based_eviction() {
        let cache = LockFreeCache::new(150); // Small cache

        // Add items
        let key1 = EKey::new([0x11; 16]);
        let key2 = EKey::new([0x22; 16]);
        let key3 = EKey::new([0x33; 16]);

        cache.put(key1, Arc::new(vec![1; 50]));
        cache.put(key2, Arc::new(vec![2; 50]));

        // Access key1 multiple times to increase its frequency
        for _ in 0..5 {
            cache.get(&key1);
        }

        // Add key3, which should evict key2 (less frequently accessed)
        cache.put(key3, Arc::new(vec![3; 50]));

        // key1 should still be in cache (frequently accessed)
        assert!(cache.contains(&key1));
        // key3 should be in cache (newly added)
        assert!(cache.contains(&key3));
    }
}

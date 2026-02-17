//! LRU cache with generation checkpoints.
//!
//! CASC maintains an LRU cache in shared memory with:
//! - Generation-based checkpoints for eviction decisions
//! - 20-char hex `.lru` filenames
//! - Bounded memory usage with configurable eviction policy
//!
//! This replaces the unbounded `DashMap` caches in the old code.

pub mod lru_file;

use std::collections::VecDeque;

/// LRU cache manager.
pub struct LruManager {
    /// Ordered entries, most recently used at the back.
    entries: VecDeque<LruEntry>,
    /// Current generation number.
    generation: u64,
    /// Maximum number of entries.
    max_size: usize,
}

/// Single LRU cache entry.
#[derive(Debug, Clone)]
pub struct LruEntry {
    /// Content key.
    pub key: [u8; 16],
    /// Generation when this entry was last accessed.
    pub generation: u64,
    /// Last access timestamp (seconds since epoch).
    pub last_access: u64,
}

impl LruManager {
    /// Create a new LRU manager with the given capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_size),
            generation: 0,
            max_size,
        }
    }

    /// Record an access for a key.
    pub fn touch(&mut self, key: [u8; 16]) {
        // Remove existing entry if present
        self.entries.retain(|e| e.key != key);

        // Evict if at capacity
        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
        }

        // Add at the back (most recently used)
        self.entries.push_back(LruEntry {
            key,
            generation: self.generation,
            last_access: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    }

    /// Advance the generation counter.
    pub fn checkpoint(&mut self) {
        self.generation += 1;
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the current generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_basic() {
        let mut lru = LruManager::new(3);
        assert!(lru.is_empty());

        lru.touch([1u8; 16]);
        lru.touch([2u8; 16]);
        lru.touch([3u8; 16]);
        assert_eq!(lru.len(), 3);

        // Adding a 4th should evict the first
        lru.touch([4u8; 16]);
        assert_eq!(lru.len(), 3);
    }

    #[test]
    fn test_lru_touch_existing() {
        let mut lru = LruManager::new(3);
        lru.touch([1u8; 16]);
        lru.touch([2u8; 16]);
        lru.touch([1u8; 16]); // Touch again, should move to back
        assert_eq!(lru.len(), 2); // No duplicate
    }

    #[test]
    fn test_checkpoint() {
        let mut lru = LruManager::new(10);
        assert_eq!(lru.generation(), 0);
        lru.checkpoint();
        assert_eq!(lru.generation(), 1);
    }
}

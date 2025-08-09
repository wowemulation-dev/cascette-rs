//! Combined index for efficient lookups across multiple bucket indices

use crate::types::{ArchiveLocation, EKey};
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{debug, trace};

/// A combined index that maintains both per-bucket indices and a global lookup table
pub struct CombinedIndex {
    /// Per-bucket indices using BTreeMap for O(log n) lookups
    bucket_indices: DashMap<u8, BTreeMap<EKey, ArchiveLocation>>,
    /// Global index for direct O(1) bucket lookup
    global_index: DashMap<EKey, u8>,
    /// Optional bloom filter for existence checks (future enhancement)
    #[allow(dead_code)]
    bloom_filter: Option<Arc<BloomFilter>>,
}

impl CombinedIndex {
    /// Create a new combined index
    pub fn new() -> Self {
        Self {
            bucket_indices: DashMap::new(),
            global_index: DashMap::new(),
            bloom_filter: None,
        }
    }

    /// Add an entry to the index
    pub fn insert(&self, ekey: EKey, location: ArchiveLocation) {
        let bucket = ekey.bucket_index();

        // Update bucket index
        self.bucket_indices
            .entry(bucket)
            .or_default()
            .insert(ekey, location);

        // Update global index
        self.global_index.insert(ekey, bucket);

        trace!("Inserted {} into bucket {:02x}", ekey, bucket);
    }

    /// Perform an optimized lookup using bucket hash
    pub fn lookup(&self, ekey: &EKey) -> Option<ArchiveLocation> {
        // First check if we know which bucket this key is in
        if let Some(bucket_ref) = self.global_index.get(ekey) {
            let bucket = *bucket_ref;

            // Direct lookup in the specific bucket - O(log n)
            if let Some(bucket_index) = self.bucket_indices.get(&bucket) {
                return bucket_index.get(ekey).copied();
            }
        }

        // Fallback: compute bucket and try direct lookup
        let computed_bucket = ekey.bucket_index();
        if let Some(bucket_index) = self.bucket_indices.get(&computed_bucket) {
            if let Some(location) = bucket_index.get(ekey) {
                // Update global index for next time
                self.global_index.insert(*ekey, computed_bucket);
                return Some(*location);
            }
        }

        // Last resort: search all buckets (should rarely happen)
        debug!("Falling back to full search for {}", ekey);
        for bucket_ref in self.bucket_indices.iter() {
            if let Some(location) = bucket_ref.value().get(ekey) {
                // Update global index for next time
                self.global_index.insert(*ekey, *bucket_ref.key());
                return Some(*location);
            }
        }

        None
    }

    /// Batch lookup for multiple keys - optimized for bulk operations
    pub fn lookup_batch(&self, ekeys: &[EKey]) -> Vec<Option<ArchiveLocation>> {
        ekeys.iter().map(|ekey| self.lookup(ekey)).collect()
    }

    /// Get all entries from a specific bucket
    pub fn get_bucket(&self, bucket: u8) -> Option<Vec<(EKey, ArchiveLocation)>> {
        self.bucket_indices
            .get(&bucket)
            .map(|index| index.iter().map(|(k, v)| (*k, *v)).collect())
    }

    /// Get total number of entries across all buckets
    pub fn total_entries(&self) -> usize {
        self.bucket_indices
            .iter()
            .map(|bucket| bucket.value().len())
            .sum()
    }

    /// Get number of buckets
    pub fn bucket_count(&self) -> usize {
        self.bucket_indices.len()
    }

    /// Clear all indices
    pub fn clear(&self) {
        self.bucket_indices.clear();
        self.global_index.clear();
    }

    /// Rebuild global index from bucket indices (for migration)
    pub fn rebuild_global_index(&self) {
        self.global_index.clear();

        for bucket_ref in self.bucket_indices.iter() {
            let bucket = *bucket_ref.key();
            for ekey in bucket_ref.value().keys() {
                self.global_index.insert(*ekey, bucket);
            }
        }

        debug!(
            "Rebuilt global index with {} entries",
            self.global_index.len()
        );
    }

    /// Get statistics about the index
    pub fn stats(&self) -> IndexStats {
        let mut min_bucket_size = usize::MAX;
        let mut max_bucket_size = 0;
        let mut total_entries = 0;

        for bucket_ref in self.bucket_indices.iter() {
            let size = bucket_ref.value().len();
            min_bucket_size = min_bucket_size.min(size);
            max_bucket_size = max_bucket_size.max(size);
            total_entries += size;
        }

        IndexStats {
            total_entries,
            bucket_count: self.bucket_indices.len(),
            min_bucket_size: if min_bucket_size == usize::MAX {
                0
            } else {
                min_bucket_size
            },
            max_bucket_size,
            avg_bucket_size: if self.bucket_indices.is_empty() {
                0.0
            } else {
                total_entries as f64 / self.bucket_indices.len() as f64
            },
            global_index_size: self.global_index.len(),
        }
    }
}

impl Default for CombinedIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the combined index
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_entries: usize,
    pub bucket_count: usize,
    pub min_bucket_size: usize,
    pub max_bucket_size: usize,
    pub avg_bucket_size: f64,
    pub global_index_size: usize,
}

/// Simple bloom filter for existence checks (stub for future implementation)
struct BloomFilter {
    // Future: implement a proper bloom filter for fast negative lookups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combined_index_lookup() {
        let index = CombinedIndex::new();

        // Create test entries
        let key1 = EKey::new([0x10; 16]);
        let key2 = EKey::new([0x20; 16]);
        let key3 = EKey::new([0x30; 16]);

        let loc1 = ArchiveLocation {
            archive_id: 1,
            offset: 100,
            size: 50,
        };

        let loc2 = ArchiveLocation {
            archive_id: 2,
            offset: 200,
            size: 60,
        };

        // Insert entries
        index.insert(key1, loc1);
        index.insert(key2, loc2);

        // Test lookups
        assert_eq!(index.lookup(&key1), Some(loc1));
        assert_eq!(index.lookup(&key2), Some(loc2));
        assert_eq!(index.lookup(&key3), None);

        // Test stats
        let stats = index.stats();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.global_index_size, 2);
    }

    #[test]
    fn test_batch_lookup() {
        let index = CombinedIndex::new();

        // Insert test data
        for i in 0..10u8 {
            let mut key_bytes = [0u8; 16];
            key_bytes[0] = i;
            let key = EKey::new(key_bytes);
            let loc = ArchiveLocation {
                archive_id: i as u16,
                offset: (i as u64) * 100,
                size: 50,
            };
            index.insert(key, loc);
        }

        // Batch lookup
        let keys: Vec<_> = (0..5u8)
            .map(|i| {
                let mut key_bytes = [0u8; 16];
                key_bytes[0] = i;
                EKey::new(key_bytes)
            })
            .collect();

        let results = index.lookup_batch(&keys);
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.is_some()));
    }
}

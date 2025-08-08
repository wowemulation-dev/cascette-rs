//! Sorted index implementation for efficient binary search lookups

use crate::types::{ArchiveLocation, EKey};
use std::collections::BTreeMap;

/// A sorted index that provides O(log n) lookups using binary search
#[derive(Debug, Clone)]
pub struct SortedIndex {
    /// Entries stored in a BTreeMap for automatic sorting and binary search
    entries: BTreeMap<EKey, ArchiveLocation>,
    /// Cached sorted keys for range queries
    sorted_keys: Option<Vec<EKey>>,
}

impl SortedIndex {
    /// Create a new empty sorted index
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            sorted_keys: None,
        }
    }

    /// Create a sorted index with pre-allocated capacity
    pub fn with_capacity(_capacity: usize) -> Self {
        // BTreeMap doesn't support with_capacity, but we keep the API consistent
        Self::new()
    }

    /// Insert an entry into the index
    pub fn insert(&mut self, key: EKey, location: ArchiveLocation) {
        self.entries.insert(key, location);
        // Invalidate sorted keys cache
        self.sorted_keys = None;
    }

    /// Perform a binary search lookup - O(log n)
    pub fn lookup(&self, key: &EKey) -> Option<&ArchiveLocation> {
        self.entries.get(key)
    }

    /// Get a range of entries between two keys
    pub fn range(
        &self,
        start: &EKey,
        end: &EKey,
    ) -> impl Iterator<Item = (&EKey, &ArchiveLocation)> {
        self.entries.range(start..=end)
    }

    /// Find the first entry with key >= target
    pub fn lower_bound(&self, key: &EKey) -> Option<(&EKey, &ArchiveLocation)> {
        self.entries.range(key..).next()
    }

    /// Find the last entry with key <= target
    pub fn upper_bound(&self, key: &EKey) -> Option<(&EKey, &ArchiveLocation)> {
        self.entries.range(..=key).next_back()
    }

    /// Get all entries
    pub fn entries(&self) -> impl Iterator<Item = (&EKey, &ArchiveLocation)> {
        self.entries.iter()
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.sorted_keys = None;
    }

    /// Get sorted keys (cached)
    pub fn sorted_keys(&mut self) -> &[EKey] {
        if self.sorted_keys.is_none() {
            self.sorted_keys = Some(self.entries.keys().copied().collect());
        }
        self.sorted_keys.as_ref().unwrap()
    }

    /// Perform a bulk insert from an iterator
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (EKey, ArchiveLocation)>,
    {
        self.entries.extend(iter);
        self.sorted_keys = None;
    }

    /// Create from a HashMap for migration
    pub fn from_hashmap(map: std::collections::HashMap<EKey, ArchiveLocation>) -> Self {
        let mut index = Self::new();
        index.extend(map);
        index
    }

    /// Convert to BTreeMap for compatibility
    pub fn into_btree_map(self) -> BTreeMap<EKey, ArchiveLocation> {
        self.entries
    }
}

impl Default for SortedIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_index_operations() {
        let mut index = SortedIndex::new();

        // Create test keys
        let key1 = EKey::new([1; 16]);
        let key2 = EKey::new([2; 16]);
        let key3 = EKey::new([3; 16]);

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

        let loc3 = ArchiveLocation {
            archive_id: 3,
            offset: 300,
            size: 70,
        };

        // Insert entries
        index.insert(key2, loc2);
        index.insert(key1, loc1);
        index.insert(key3, loc3);

        // Test lookup
        assert_eq!(index.lookup(&key1), Some(&loc1));
        assert_eq!(index.lookup(&key2), Some(&loc2));
        assert_eq!(index.lookup(&key3), Some(&loc3));

        // Test non-existent key
        let key4 = EKey::new([4; 16]);
        assert_eq!(index.lookup(&key4), None);

        // Test size
        assert_eq!(index.len(), 3);

        // Test range query
        let range: Vec<_> = index.range(&key1, &key2).collect();
        assert_eq!(range.len(), 2);

        // Test lower bound
        let lower = index.lower_bound(&key2);
        assert!(lower.is_some());
        assert_eq!(lower.unwrap().0, &key2);
    }

    #[test]
    fn test_sorted_order() {
        let mut index = SortedIndex::new();

        // Insert in random order
        for i in [5u8, 2, 8, 1, 9, 3, 7, 4, 6] {
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

        // Verify entries are sorted
        let keys: Vec<_> = index.entries().map(|(k, _)| *k).collect();
        for i in 1..keys.len() {
            assert!(keys[i - 1] < keys[i], "Keys should be in sorted order");
        }
    }
}

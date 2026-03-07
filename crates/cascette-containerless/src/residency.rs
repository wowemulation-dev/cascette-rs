//! In-memory residency tracking.
//!
//! Tracks which encoding keys are locally available (resident) on disk.
//! This avoids filesystem stat calls for every lookup by maintaining
//! a `HashSet` in memory that is populated on open and updated on
//! write/remove operations.

use std::collections::HashSet;
use std::sync::RwLock;

/// Tracks which files are resident (locally available) by encoding key.
pub struct ResidencyTracker {
    keys: RwLock<HashSet<[u8; 16]>>,
}

impl ResidencyTracker {
    /// Create an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(HashSet::new()),
        }
    }

    /// Create a tracker pre-populated with known resident keys.
    #[must_use]
    pub fn with_keys(keys: HashSet<[u8; 16]>) -> Self {
        Self {
            keys: RwLock::new(keys),
        }
    }

    /// Mark a key as resident.
    pub fn mark_resident(&self, ekey: &[u8; 16]) {
        if let Ok(mut set) = self.keys.write() {
            set.insert(*ekey);
        }
    }

    /// Mark a key as non-resident.
    pub fn mark_absent(&self, ekey: &[u8; 16]) {
        if let Ok(mut set) = self.keys.write() {
            set.remove(ekey);
        }
    }

    /// Check whether a key is resident.
    #[must_use]
    pub fn is_resident(&self, ekey: &[u8; 16]) -> bool {
        self.keys.read().is_ok_and(|set| set.contains(ekey))
    }

    /// Number of resident keys.
    #[must_use]
    pub fn count(&self) -> usize {
        self.keys.read().map_or(0, |set| set.len())
    }

    /// Get all resident keys.
    #[must_use]
    pub fn all_keys(&self) -> Vec<[u8; 16]> {
        self.keys
            .read()
            .map_or_else(|_| Vec::new(), |set| set.iter().copied().collect())
    }
}

impl Default for ResidencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tracker() {
        let tracker = ResidencyTracker::new();
        assert_eq!(tracker.count(), 0);
        assert!(!tracker.is_resident(&[0x01; 16]));
    }

    #[test]
    fn test_mark_and_check() {
        let tracker = ResidencyTracker::new();
        let key = [0xAA; 16];

        tracker.mark_resident(&key);
        assert!(tracker.is_resident(&key));
        assert_eq!(tracker.count(), 1);
    }

    #[test]
    fn test_mark_absent() {
        let tracker = ResidencyTracker::new();
        let key = [0xBB; 16];

        tracker.mark_resident(&key);
        assert!(tracker.is_resident(&key));

        tracker.mark_absent(&key);
        assert!(!tracker.is_resident(&key));
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn test_with_keys() {
        let mut keys = HashSet::new();
        keys.insert([0x01; 16]);
        keys.insert([0x02; 16]);

        let tracker = ResidencyTracker::with_keys(keys);
        assert_eq!(tracker.count(), 2);
        assert!(tracker.is_resident(&[0x01; 16]));
        assert!(tracker.is_resident(&[0x02; 16]));
        assert!(!tracker.is_resident(&[0x03; 16]));
    }

    #[test]
    fn test_all_keys() {
        let tracker = ResidencyTracker::new();
        tracker.mark_resident(&[0x01; 16]);
        tracker.mark_resident(&[0x02; 16]);

        let keys = tracker.all_keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&[0x01; 16]));
        assert!(keys.contains(&[0x02; 16]));
    }

    #[test]
    fn test_duplicate_mark() {
        let tracker = ResidencyTracker::new();
        let key = [0xCC; 16];

        tracker.mark_resident(&key);
        tracker.mark_resident(&key);
        assert_eq!(tracker.count(), 1);
    }
}

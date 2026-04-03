//! E-header cache for loose files.
//!
//! E-headers contain BLTE encoding metadata (frame table) for each
//! loose file. Caching them avoids re-reading the first bytes of every
//! file to determine encoding parameters.

use std::collections::HashMap;
use std::sync::RwLock;

/// Cached encoding header for a loose file.
#[derive(Debug, Clone)]
pub struct EHeader {
    /// Encoding key this header belongs to.
    pub ekey: [u8; 16],
    /// Total BLTE-encoded size.
    pub encoded_size: u64,
    /// Number of BLTE frames.
    pub frame_count: u32,
}

/// In-memory cache of e-headers keyed by encoding key.
pub struct EHeaderCache {
    entries: RwLock<HashMap<[u8; 16], EHeader>>,
}

impl EHeaderCache {
    /// Create an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Insert or update a cached header.
    pub fn insert(&self, header: EHeader) {
        if let Ok(mut map) = self.entries.write() {
            map.insert(header.ekey, header);
        }
    }

    /// Look up a cached header.
    #[must_use]
    pub fn get(&self, ekey: &[u8; 16]) -> Option<EHeader> {
        self.entries
            .read()
            .ok()
            .and_then(|map| map.get(ekey).cloned())
    }

    /// Remove a cached header.
    pub fn remove(&self, ekey: &[u8; 16]) {
        if let Ok(mut map) = self.entries.write() {
            map.remove(ekey);
        }
    }

    /// Number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.read().map_or(0, |map| map.len())
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        if let Ok(mut map) = self.entries.write() {
            map.clear();
        }
    }
}

impl Default for EHeaderCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample_header(ekey_byte: u8) -> EHeader {
        EHeader {
            ekey: [ekey_byte; 16],
            encoded_size: 4096,
            frame_count: 3,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let cache = EHeaderCache::new();
        let header = sample_header(0xAA);
        cache.insert(header);

        let loaded = cache.get(&[0xAA; 16]).unwrap();
        assert_eq!(loaded.encoded_size, 4096);
        assert_eq!(loaded.frame_count, 3);
    }

    #[test]
    fn test_get_missing() {
        let cache = EHeaderCache::new();
        assert!(cache.get(&[0xFF; 16]).is_none());
    }

    #[test]
    fn test_remove() {
        let cache = EHeaderCache::new();
        cache.insert(sample_header(0xBB));
        assert_eq!(cache.len(), 1);

        cache.remove(&[0xBB; 16]);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_clear() {
        let cache = EHeaderCache::new();
        cache.insert(sample_header(0x01));
        cache.insert(sample_header(0x02));
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }
}

//! Preservation set — collects encoding keys that must not be removed.
//!
//! The preservation set determines which content is "live" and must be
//! protected from garbage collection. Keys in this set will not be removed.
//!
//! Two construction paths:
//! - [`PreservationSet::build`] — index-only (fallback when manifests are unavailable)
//! - [`PreservationSet::build_from_manifests`] — manifest-aware, collects keys from
//!   install manifest, download manifest, and encoding table

use std::collections::HashSet;
use std::time::Instant;

use cascette_client_storage::Installation;
use cascette_installation::BuildManifests;

use crate::error::MaintenanceResult;
use crate::report::PreservationReport;

/// Set of encoding keys that must be preserved.
///
/// Keys are stored as 9-byte truncated encoding keys, matching the format
/// used in `.idx` index files.
pub struct PreservationSet {
    keys: HashSet<[u8; 9]>,
}

impl PreservationSet {
    /// Build a preservation set from the current installation state.
    ///
    /// Scans all index entries and collects their keys. This is a conservative
    /// approach that preserves everything currently indexed.
    pub async fn build(
        installation: &Installation,
    ) -> MaintenanceResult<(Self, PreservationReport)> {
        let start = Instant::now();

        let entries = installation.get_all_index_entries().await;
        let source_count = entries.len();

        let mut keys = HashSet::with_capacity(source_count);
        for entry in &entries {
            keys.insert(entry.key);
        }

        let report = PreservationReport {
            key_count: keys.len(),
            source_index_entries: source_count,
            manifest_keys: 0,
            duration: start.elapsed(),
        };

        Ok((Self { keys }, report))
    }

    /// Build a preservation set from both index entries and build manifests.
    ///
    /// Collects encoding keys from three manifest sources in addition to the
    /// index scan:
    /// 1. Install manifest — content keys looked up in the encoding table
    /// 2. Download manifest — encoding keys directly available
    /// 3. Encoding table — all encoding keys from CKey pages
    ///
    /// Keys from manifests that are not already in the index set are counted
    /// separately in `PreservationReport::manifest_keys`.
    pub async fn build_from_manifests(
        installation: &Installation,
        manifests: &BuildManifests,
    ) -> MaintenanceResult<(Self, PreservationReport)> {
        let start = Instant::now();

        // Start with index entries (same as build())
        let entries = installation.get_all_index_entries().await;
        let source_count = entries.len();

        let mut keys = HashSet::with_capacity(source_count * 2);
        for entry in &entries {
            keys.insert(entry.key);
        }
        let index_key_count = keys.len();

        // Install manifest: look up content keys in encoding table
        for install_entry in &manifests.install.entries {
            if let Some(ekey) = manifests.encoding.find_encoding(&install_entry.content_key) {
                keys.insert(ekey.first_9());
            }
        }

        // Download manifest: encoding keys are directly available
        for download_entry in &manifests.download.entries {
            keys.insert(download_entry.encoding_key.first_9());
        }

        // Encoding table: iterate all CKey page entries for their encoding keys
        for page in &manifests.encoding.ckey_pages {
            for ckey_entry in &page.entries {
                for ekey in &ckey_entry.encoding_keys {
                    keys.insert(ekey.first_9());
                }
            }
        }

        let manifest_keys = keys.len().saturating_sub(index_key_count);

        let report = PreservationReport {
            key_count: keys.len(),
            source_index_entries: source_count,
            manifest_keys,
            duration: start.elapsed(),
        };

        Ok((Self { keys }, report))
    }

    /// Check whether a key is in the preservation set.
    pub fn contains(&self, key: &[u8; 9]) -> bool {
        self.keys.contains(key)
    }

    /// Number of preserved keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the preservation set is empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Iterate over all preserved keys.
    pub fn iter(&self) -> impl Iterator<Item = &[u8; 9]> {
        self.keys.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cascette_crypto::EncodingKey;

    #[test]
    fn empty_set_operations() {
        let set = PreservationSet {
            keys: HashSet::new(),
        };
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(!set.contains(&[0u8; 9]));
        assert_eq!(set.iter().count(), 0);
    }

    #[test]
    fn contains_inserted_key() {
        let key = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut keys = HashSet::new();
        keys.insert(key);
        let set = PreservationSet { keys };

        assert!(set.contains(&key));
        assert!(!set.contains(&[0u8; 9]));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn deduplication() {
        let key = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut keys = HashSet::new();
        keys.insert(key);
        keys.insert(key); // duplicate
        let set = PreservationSet { keys };

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn encoding_key_truncation_to_9_bytes() {
        let ekey = EncodingKey::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let truncated = ekey.first_9();
        assert_eq!(truncated, [1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let mut keys = HashSet::new();
        keys.insert(truncated);
        let set = PreservationSet { keys };
        assert!(set.contains(&truncated));
    }
}

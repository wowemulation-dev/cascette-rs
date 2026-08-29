//! Patch data resolution
//!
//! Resolves patch encoding keys to their location within CDN patch archives.
//! Uses archive index data to map keys to (archive_index, offset, size) tuples.

use std::collections::HashMap;

use cascette_formats::patch_archive::PatchLocation;

/// Resolves patch encoding keys to archive locations
///
/// Populated from CDN config `patch-archives` and their corresponding
/// archive index files.
#[derive(Debug)]
pub struct PatchResolver {
    /// Map from patch EKey to its archive location
    locations: HashMap<[u8; 16], PatchLocation>,
    /// List of patch archive hashes from CDN config
    archive_hashes: Vec<String>,
}

impl PatchResolver {
    /// Create a new resolver from archive index entries
    ///
    /// # Arguments
    ///
    /// * `archive_hashes` - Patch archive hashes from CDN config `patch-archives`
    /// * `entries` - Tuples of (ekey, archive_index, offset, encoded_size) from archive indexes
    pub fn new(archive_hashes: Vec<String>, entries: Vec<([u8; 16], u16, u64, u32)>) -> Self {
        let mut locations = HashMap::with_capacity(entries.len());
        for (ekey, archive_index, offset, encoded_size) in entries {
            locations.insert(
                ekey,
                PatchLocation {
                    archive_index,
                    offset,
                    encoded_size,
                },
            );
        }
        Self {
            locations,
            archive_hashes,
        }
    }

    /// Look up the location of a patch by its encoding key
    pub fn locate_patch(&self, patch_ekey: &[u8; 16]) -> Option<&PatchLocation> {
        self.locations.get(patch_ekey)
    }

    /// Construct the CDN path for a patch data blob by its hex hash
    ///
    /// Format: `patch/{hash[0:2]}/{hash[2:4]}/{hash}`
    pub fn patch_cdn_path(hash: &str) -> String {
        if hash.len() < 4 {
            return format!("patch/{hash}");
        }
        format!("patch/{}/{}/{}", &hash[..2], &hash[2..4], hash)
    }

    /// Get the CDN path for a patch archive by its index
    pub fn archive_cdn_path(&self, archive_index: u16) -> Option<String> {
        self.archive_hashes
            .get(archive_index as usize)
            .map(|hash| PatchLocation::cdn_archive_path(hash))
    }

    /// Get the raw hex hash for a patch archive by its index.
    pub fn archive_hash(&self, archive_index: u16) -> Option<&str> {
        self.archive_hashes
            .get(archive_index as usize)
            .map(String::as_str)
    }

    /// Number of patch archives
    pub fn archive_count(&self) -> usize {
        self.archive_hashes.len()
    }

    /// Number of indexed patch entries
    pub fn entry_count(&self) -> usize {
        self.locations.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_cdn_path() {
        let hash = "abcdef1234567890abcdef1234567890";
        assert_eq!(
            PatchResolver::patch_cdn_path(hash),
            "patch/ab/cd/abcdef1234567890abcdef1234567890"
        );
    }

    #[test]
    fn test_resolve_patch() {
        let ekey = [0xAA; 16];
        let resolver = PatchResolver::new(
            vec!["deadbeef12345678deadbeef12345678".to_string()],
            vec![(ekey, 0, 1024, 512)],
        );

        let loc = resolver.locate_patch(&ekey).expect("should find patch");
        assert_eq!(loc.archive_index, 0);
        assert_eq!(loc.offset, 1024);
        assert_eq!(loc.encoded_size, 512);

        assert!(resolver.locate_patch(&[0xBB; 16]).is_none());
    }

    #[test]
    fn test_archive_cdn_path() {
        let resolver = PatchResolver::new(
            vec![
                "aabbccdd11223344aabbccdd11223344".to_string(),
                "eeff0011aabbccdd5566778899aabbcc".to_string(),
            ],
            vec![],
        );

        assert_eq!(
            resolver.archive_cdn_path(0).unwrap(),
            "aa/bb/aabbccdd11223344aabbccdd11223344"
        );
        assert_eq!(
            resolver.archive_cdn_path(1).unwrap(),
            "ee/ff/eeff0011aabbccdd5566778899aabbcc"
        );
        assert!(resolver.archive_cdn_path(2).is_none());
    }

    #[test]
    fn test_resolver_counts() {
        let resolver = PatchResolver::new(
            vec!["hash1".to_string(), "hash2".to_string()],
            vec![([0x01; 16], 0, 0, 100), ([0x02; 16], 1, 512, 200)],
        );

        assert_eq!(resolver.archive_count(), 2);
        assert_eq!(resolver.entry_count(), 2);
    }
}

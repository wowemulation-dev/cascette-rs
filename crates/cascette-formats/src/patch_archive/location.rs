//! Patch data location types
//!
//! Types for resolving where patch data resides within CDN patch archives.
//! Used by the patch resolver to map patch encoding keys to archive offsets.

/// Location of a patch blob within a CDN patch archive
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchLocation {
    /// Index into the CDN config `patch-archives` list
    pub archive_index: u16,
    /// Byte offset within the archive data file
    pub offset: u64,
    /// Encoded size of the patch blob
    pub encoded_size: u32,
}

impl PatchLocation {
    /// Construct a CDN path for a patch archive by its hash
    ///
    /// CDN path format: `{hash[0:2]}/{hash[2:4]}/{hash}`
    pub fn cdn_archive_path(archive_hash: &str) -> String {
        if archive_hash.len() < 4 {
            return archive_hash.to_string();
        }
        format!(
            "{}/{}/{}",
            &archive_hash[..2],
            &archive_hash[2..4],
            archive_hash
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_cdn_archive_path() {
        let hash = "abcdef1234567890abcdef1234567890";
        assert_eq!(
            PatchLocation::cdn_archive_path(hash),
            "ab/cd/abcdef1234567890abcdef1234567890"
        );
    }

    #[test]
    fn test_cdn_archive_path_short_hash() {
        assert_eq!(PatchLocation::cdn_archive_path("ab"), "ab");
    }

    #[test]
    fn test_patch_location_fields() {
        let loc = PatchLocation {
            archive_index: 3,
            offset: 12345,
            encoded_size: 678,
        };
        assert_eq!(loc.archive_index, 3);
        assert_eq!(loc.offset, 12345);
        assert_eq!(loc.encoded_size, 678);
    }
}

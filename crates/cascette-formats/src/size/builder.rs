//! Builder for constructing Size manifests

use crate::install::TagType;
use crate::size::SizeTag;
use crate::size::entry::SizeEntry;
use crate::size::error::{Result, SizeError};
use crate::size::header::SizeHeader;
use crate::size::manifest::SizeManifest;

/// Builder for constructing `SizeManifest` instances
///
/// The builder collects entries and configuration, then computes
/// the derived header fields (total_size, entry_count) at build time.
pub struct SizeManifestBuilder {
    version: u8,
    ekey_size: u8,
    tag_count: u16,
    esize_bytes: u8,
    tags: Vec<SizeTag>,
    entries: Vec<SizeEntry>,
}

impl SizeManifestBuilder {
    /// Create a new builder with default settings
    ///
    /// Defaults: version 2, ekey_size 9, tag_count 0, esize_bytes 4
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: 2,
            ekey_size: 9,
            tag_count: 0,
            esize_bytes: 4,
            tags: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Set the format version (1 or 2)
    #[must_use]
    pub fn version(mut self, version: u8) -> Self {
        self.version = version;
        self
    }

    /// Set the encoding key size in bytes (1-16, typically 9)
    #[must_use]
    pub fn ekey_size(mut self, size: u8) -> Self {
        self.ekey_size = size;
        self
    }

    /// Set the tag count
    ///
    /// This sets the expected tag count in the header. Tags must be added
    /// to the manifest after building, or use `add_tag()`.
    #[must_use]
    pub fn tag_count(mut self, count: u16) -> Self {
        self.tag_count = count;
        self
    }

    /// Set the esize byte width (V1 only, ignored for V2)
    #[must_use]
    pub fn esize_bytes(mut self, width: u8) -> Self {
        self.esize_bytes = width;
        self
    }

    /// Add a tag with the given name and type
    ///
    /// The tag's bit mask is sized to the current entry count at build time.
    #[must_use]
    pub fn add_tag(mut self, name: String, tag_type: TagType) -> Self {
        self.tags.push(SizeTag::new(name, tag_type, 0));
        self
    }

    /// Mark a file as associated with a tag
    ///
    /// # Panics
    ///
    /// Panics if `tag_index` is out of bounds.
    #[must_use]
    pub fn tag_file(mut self, tag_index: usize, file_index: usize) -> Self {
        // Ensure bit_mask is large enough
        let needed = (file_index + 1).div_ceil(8);
        if self.tags[tag_index].bit_mask.len() < needed {
            self.tags[tag_index].bit_mask.resize(needed, 0);
        }
        self.tags[tag_index].add_file(file_index);
        self
    }

    /// Add an entry with the given key and estimated size
    #[must_use]
    pub fn add_entry(mut self, key: Vec<u8>, esize: u64) -> Self {
        self.entries.push(SizeEntry::new(key, esize));
        self
    }

    /// Build the final `SizeManifest`
    ///
    /// Computes total_size from the sum of entry esizes and entry_count
    /// from the number of added entries. If tags were added via `add_tag()`,
    /// tag_count is set automatically and bit masks are resized.
    pub fn build(mut self) -> Result<SizeManifest> {
        if self.version == 0 || self.version > 2 {
            return Err(SizeError::UnsupportedVersion(self.version));
        }

        let ekey_size = self.ekey_size;
        if ekey_size == 0 || ekey_size > 16 {
            return Err(SizeError::InvalidEKeySize(ekey_size));
        }

        // If tags were added via add_tag(), update tag_count
        if !self.tags.is_empty() {
            self.tag_count = self.tags.len() as u16;
        }

        let entry_count = self.entries.len() as u32;
        let total_size: u64 = self.entries.iter().map(|e| e.esize).sum();

        // Resize tag bit masks to match entry count
        let bit_mask_size = (self.entries.len()).div_ceil(8);
        for tag in &mut self.tags {
            tag.bit_mask.resize(bit_mask_size, 0);
        }

        let header = match self.version {
            1 => {
                if self.esize_bytes == 0 || self.esize_bytes > 8 {
                    return Err(SizeError::InvalidEsizeWidth(self.esize_bytes));
                }
                SizeHeader::new_v1(
                    ekey_size,
                    entry_count,
                    self.tag_count,
                    total_size,
                    self.esize_bytes,
                )
            }
            2 => SizeHeader::new_v2(ekey_size, entry_count, self.tag_count, total_size),
            _ => unreachable!(),
        };

        let manifest = SizeManifest {
            header,
            tags: self.tags,
            entries: self.entries,
        };

        // Validate the constructed manifest
        manifest.validate()?;

        Ok(manifest)
    }
}

impl Default for SizeManifestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let manifest = SizeManifestBuilder::new()
            .add_entry(vec![0xAA; 9], 100)
            .build()
            .expect("Should build with defaults");

        assert_eq!(manifest.header.version(), 2);
        assert_eq!(manifest.header.ekey_size(), 9);
        assert_eq!(manifest.header.tag_count(), 0);
        assert_eq!(manifest.header.esize_bytes(), 4);
        assert_eq!(manifest.header.entry_count(), 1);
        assert_eq!(manifest.header.total_size(), 100);
        assert_eq!(manifest.tags.len(), 0);
    }

    #[test]
    fn test_builder_v1() {
        let manifest = SizeManifestBuilder::new()
            .version(1)
            .esize_bytes(2)
            .ekey_size(9)
            .add_entry(vec![0x11; 9], 50)
            .add_entry(vec![0x22; 9], 75)
            .build()
            .expect("Should build V1 manifest");

        assert_eq!(manifest.header.version(), 1);
        assert_eq!(manifest.header.esize_bytes(), 2);
        assert_eq!(manifest.header.entry_count(), 2);
        assert_eq!(manifest.header.total_size(), 125);
    }

    #[test]
    fn test_builder_v2() {
        let manifest = SizeManifestBuilder::new()
            .version(2)
            .ekey_size(9)
            .add_entry(vec![0xCC; 9], 1000)
            .build()
            .expect("Should build V2 manifest");

        assert_eq!(manifest.header.version(), 2);
        assert_eq!(manifest.header.ekey_size(), 9);
        assert_eq!(manifest.header.esize_bytes(), 4);
        assert_eq!(manifest.header.total_size(), 1000);
    }

    #[test]
    fn test_builder_empty_manifest() {
        let manifest = SizeManifestBuilder::new()
            .build()
            .expect("Should build empty manifest");

        assert_eq!(manifest.entries.len(), 0);
        assert_eq!(manifest.tags.len(), 0);
        assert_eq!(manifest.header.total_size(), 0);
    }

    #[test]
    fn test_builder_rejects_version_0() {
        let result = SizeManifestBuilder::new().version(0).build();
        assert!(matches!(result, Err(SizeError::UnsupportedVersion(0))));
    }

    #[test]
    fn test_builder_rejects_version_3() {
        let result = SizeManifestBuilder::new().version(3).build();
        assert!(matches!(result, Err(SizeError::UnsupportedVersion(3))));
    }

    #[test]
    fn test_builder_rejects_zero_ekey_size() {
        let result = SizeManifestBuilder::new().ekey_size(0).build();
        assert!(matches!(result, Err(SizeError::InvalidEKeySize(0))));
    }

    #[test]
    fn test_builder_rejects_ekey_size_17() {
        let result = SizeManifestBuilder::new().ekey_size(17).build();
        assert!(matches!(result, Err(SizeError::InvalidEKeySize(17))));
    }

    #[test]
    fn test_builder_rejects_invalid_esize_bytes_v1() {
        let result = SizeManifestBuilder::new().version(1).esize_bytes(0).build();
        assert!(matches!(result, Err(SizeError::InvalidEsizeWidth(0))));

        let result = SizeManifestBuilder::new().version(1).esize_bytes(9).build();
        assert!(matches!(result, Err(SizeError::InvalidEsizeWidth(9))));
    }

    #[test]
    fn test_builder_with_tags() {
        let manifest = SizeManifestBuilder::new()
            .version(2)
            .ekey_size(9)
            .add_entry(vec![0xAA; 9], 100)
            .add_entry(vec![0xBB; 9], 200)
            .add_tag("Windows".to_string(), TagType::Platform)
            .tag_file(0, 0)
            .tag_file(0, 1)
            .add_tag("x86_64".to_string(), TagType::Architecture)
            .tag_file(1, 0)
            .build()
            .expect("Should build manifest with tags");

        assert_eq!(manifest.header.tag_count(), 2);
        assert_eq!(manifest.tags.len(), 2);
        assert_eq!(manifest.tags[0].name, "Windows");
        assert!(manifest.tags[0].has_file(0));
        assert!(manifest.tags[0].has_file(1));
        assert_eq!(manifest.tags[1].name, "x86_64");
        assert!(manifest.tags[1].has_file(0));
        assert!(!manifest.tags[1].has_file(1));
    }

    #[test]
    fn test_builder_tag_count_auto_set() {
        let manifest = SizeManifestBuilder::new()
            .add_tag("Test".to_string(), TagType::Platform)
            .build()
            .expect("Should build");

        assert_eq!(manifest.header.tag_count(), 1);
    }
}

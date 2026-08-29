//! Builder for constructing Size manifests

use crate::install::{InstallTag, TagType};
use crate::size::entry::SizeEntry;
use crate::size::error::{Result, SizeError};
use crate::size::header::SizeHeader;
use crate::size::manifest::SizeManifest;

/// Builder for `SizeManifest`.
///
/// Collects entries and tags, computes `total_size` and counts at build time.
pub struct SizeManifestBuilder {
    version: u8,
    ekey_size: u8,
    tags: Vec<InstallTag>,
    entries: Vec<SizeEntry>,
}

impl SizeManifestBuilder {
    /// Create a new builder with default settings.
    ///
    /// Defaults: version 1, ekey_size 9.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: 1,
            ekey_size: 9,
            tags: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Set the format version.
    #[must_use]
    pub fn version(mut self, version: u8) -> Self {
        self.version = version;
        self
    }

    /// Set the EKey size in bytes (e.g. 9).
    #[must_use]
    pub fn ekey_size(mut self, size: u8) -> Self {
        self.ekey_size = size;
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn add_tag(mut self, name: String, tag_type: TagType) -> Self {
        self.tags.push(InstallTag::new(name, tag_type, 0));
        self
    }

    /// Mark a file as associated with a tag.
    ///
    /// # Panics
    ///
    /// Panics if `tag_index` is out of bounds.
    #[must_use]
    pub fn tag_file(mut self, tag_index: usize, file_index: usize) -> Self {
        let needed = (file_index + 1).div_ceil(8);
        if self.tags[tag_index].bit_mask.len() < needed {
            self.tags[tag_index].bit_mask.resize(needed, 0);
        }
        self.tags[tag_index].add_file(file_index);
        self
    }

    /// Add a file entry.
    #[must_use]
    pub fn add_entry(mut self, key: Vec<u8>, esize: u32) -> Self {
        self.entries.push(SizeEntry::new(key, esize));
        self
    }

    /// Build the final `SizeManifest`.
    pub fn build(mut self) -> Result<SizeManifest> {
        if self.version == 0 {
            return Err(SizeError::UnsupportedVersion(self.version));
        }
        if self.ekey_size == 0 || self.ekey_size > 16 {
            return Err(SizeError::InvalidEKeySize(self.ekey_size));
        }

        let num_files = self.entries.len() as u32;
        let num_tags = self.tags.len() as u16;
        let total_size: u64 = self.entries.iter().map(|e| u64::from(e.esize)).sum();

        // Resize tag bit masks to match entry count
        let bit_mask_size = self.entries.len().div_ceil(8);
        for tag in &mut self.tags {
            tag.bit_mask.resize(bit_mask_size, 0);
        }

        let header = SizeHeader::new(
            self.version,
            self.ekey_size,
            num_files,
            num_tags,
            total_size,
        );

        let manifest = SizeManifest {
            header,
            tags: self.tags,
            entries: self.entries,
        };

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
            .add_entry(vec![0xAAu8; 9], 100)
            .build()
            .expect("Should build with defaults");

        assert_eq!(manifest.header.version, 1);
        assert_eq!(manifest.header.ekey_size, 9);
        assert_eq!(manifest.header.num_tags, 0);
        assert_eq!(manifest.header.num_files, 1);
        assert_eq!(manifest.header.total_size, 100);
        assert_eq!(manifest.tags.len(), 0);
    }

    #[test]
    fn test_builder_two_entries() {
        let manifest = SizeManifestBuilder::new()
            .add_entry(vec![0x11u8; 9], 500)
            .add_entry(vec![0x22u8; 9], 300)
            .build()
            .expect("Should build");

        assert_eq!(manifest.header.num_files, 2);
        assert_eq!(manifest.header.total_size, 800);
    }

    #[test]
    fn test_builder_empty() {
        let manifest = SizeManifestBuilder::new()
            .build()
            .expect("Should build empty manifest");

        assert_eq!(manifest.entries.len(), 0);
        assert_eq!(manifest.header.total_size, 0);
    }

    #[test]
    fn test_builder_rejects_version_0() {
        let result = SizeManifestBuilder::new().version(0).build();
        assert!(matches!(result, Err(SizeError::UnsupportedVersion(0))));
    }

    #[test]
    fn test_builder_rejects_zero_ekey_size() {
        let result = SizeManifestBuilder::new().ekey_size(0).build();
        assert!(matches!(result, Err(SizeError::InvalidEKeySize(0))));
    }

    #[test]
    fn test_builder_rejects_oversized_ekey() {
        let result = SizeManifestBuilder::new().ekey_size(17).build();
        assert!(matches!(result, Err(SizeError::InvalidEKeySize(17))));
    }

    #[test]
    fn test_builder_with_tags() {
        let manifest = SizeManifestBuilder::new()
            .add_entry(vec![0xAAu8; 9], 100)
            .add_entry(vec![0xBBu8; 9], 200)
            .add_tag("Windows".to_string(), TagType::Platform)
            .tag_file(0, 0)
            .tag_file(0, 1)
            .build()
            .expect("Should build manifest with tags");

        assert_eq!(manifest.header.num_tags, 1);
        assert_eq!(manifest.tags.len(), 1);
        assert_eq!(manifest.tags[0].name, "Windows");
        assert!(manifest.tags[0].has_file(0));
        assert!(manifest.tags[0].has_file(1));
    }

    #[test]
    fn test_builder_round_trip() {
        let manifest = SizeManifestBuilder::new()
            .add_entry(vec![0xAAu8; 9], 500)
            .add_entry(vec![0xBBu8; 9], 600)
            .build()
            .expect("Should build");

        let data = manifest.build().expect("Should serialize");
        let parsed = SizeManifest::parse(&data).expect("Should parse");
        assert_eq!(manifest, parsed);
    }
}

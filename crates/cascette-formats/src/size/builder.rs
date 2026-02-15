//! Builder for constructing Size manifests

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
    flags: u8,
    key_size_bits: u16,
    esize_bytes: u8,
    entries: Vec<SizeEntry>,
}

impl SizeManifestBuilder {
    /// Create a new builder with default settings
    ///
    /// Defaults: version 2, flags 0, key_size_bits 128, esize_bytes 4
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: 2,
            flags: 0,
            key_size_bits: 128,
            esize_bytes: 4,
            entries: Vec::new(),
        }
    }

    /// Set the format version (1 or 2)
    #[must_use]
    pub fn version(mut self, version: u8) -> Self {
        self.version = version;
        self
    }

    /// Set the flags byte
    #[must_use]
    pub fn flags(mut self, flags: u8) -> Self {
        self.flags = flags;
        self
    }

    /// Set the key size in bits
    #[must_use]
    pub fn key_size_bits(mut self, bits: u16) -> Self {
        self.key_size_bits = bits;
        self
    }

    /// Set the esize byte width (V1 only, ignored for V2)
    #[must_use]
    pub fn esize_bytes(mut self, width: u8) -> Self {
        self.esize_bytes = width;
        self
    }

    /// Add an entry with the given key, key_hash, and estimated size
    #[must_use]
    pub fn add_entry(mut self, key: Vec<u8>, key_hash: u16, esize: u64) -> Self {
        self.entries.push(SizeEntry::new(key, key_hash, esize));
        self
    }

    /// Build the final `SizeManifest`
    ///
    /// Computes total_size from the sum of entry esizes and entry_count
    /// from the number of added entries.
    pub fn build(self) -> Result<SizeManifest> {
        if self.version == 0 || self.version > 2 {
            return Err(SizeError::UnsupportedVersion(self.version));
        }

        if self.key_size_bits == 0 {
            return Err(SizeError::InvalidKeySize);
        }

        let entry_count = self.entries.len() as u32;
        let total_size: u64 = self.entries.iter().map(|e| e.esize).sum();

        let header = match self.version {
            1 => {
                if self.esize_bytes == 0 || self.esize_bytes > 8 {
                    return Err(SizeError::InvalidEsizeWidth(self.esize_bytes));
                }
                SizeHeader::new_v1(
                    self.flags,
                    entry_count,
                    self.key_size_bits,
                    total_size,
                    self.esize_bytes,
                )
            }
            2 => SizeHeader::new_v2(self.flags, entry_count, self.key_size_bits, total_size),
            _ => unreachable!(),
        };

        let manifest = SizeManifest {
            header,
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
            .add_entry(vec![0xAA; 16], 0x1234, 100)
            .build()
            .expect("Should build with defaults");

        assert_eq!(manifest.header.version(), 2);
        assert_eq!(manifest.header.flags(), 0);
        assert_eq!(manifest.header.key_size_bits(), 128);
        assert_eq!(manifest.header.esize_bytes(), 4);
        assert_eq!(manifest.header.entry_count(), 1);
        assert_eq!(manifest.header.total_size(), 100);
    }

    #[test]
    fn test_builder_v1() {
        let manifest = SizeManifestBuilder::new()
            .version(1)
            .esize_bytes(2)
            .key_size_bits(128)
            .add_entry(vec![0x11; 16], 0x0001, 50)
            .add_entry(vec![0x22; 16], 0x0002, 75)
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
            .flags(0x05)
            .add_entry(vec![0xCC; 16], 0xABCD, 1000)
            .build()
            .expect("Should build V2 manifest");

        assert_eq!(manifest.header.version(), 2);
        assert_eq!(manifest.header.flags(), 0x05);
        assert_eq!(manifest.header.esize_bytes(), 4);
        assert_eq!(manifest.header.total_size(), 1000);
    }

    #[test]
    fn test_builder_empty_manifest() {
        let manifest = SizeManifestBuilder::new()
            .build()
            .expect("Should build empty manifest");

        assert_eq!(manifest.entries.len(), 0);
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
    fn test_builder_rejects_zero_key_size() {
        let result = SizeManifestBuilder::new().key_size_bits(0).build();
        assert!(matches!(result, Err(SizeError::InvalidKeySize)));
    }

    #[test]
    fn test_builder_rejects_invalid_esize_bytes_v1() {
        let result = SizeManifestBuilder::new().version(1).esize_bytes(0).build();
        assert!(matches!(result, Err(SizeError::InvalidEsizeWidth(0))));

        let result = SizeManifestBuilder::new().version(1).esize_bytes(9).build();
        assert!(matches!(result, Err(SizeError::InvalidEsizeWidth(9))));
    }

    #[test]
    fn test_builder_validates_key_hash() {
        let result = SizeManifestBuilder::new()
            .add_entry(vec![0x00; 16], 0x0000, 100) // invalid sentinel
            .build();
        assert!(result.is_err());
    }
}

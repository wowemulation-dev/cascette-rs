//! Patch Archive header structure

use crate::patch_archive::error::{PatchArchiveError, PatchArchiveResult};
use binrw::{BinRead, BinWrite};

/// Standard block size bits for 64KB blocks
pub const STANDARD_BLOCK_SIZE_BITS: u8 = 16;
/// Standard key size for MD5 hashes
pub const STANDARD_KEY_SIZE: u8 = 16;

/// Patch Archive header (10 bytes, big-endian)
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)] // Big-endian header
#[bw(big)]
pub struct PatchArchiveHeader {
    /// Magic bytes: "PA"
    #[br(assert(magic == *b"PA", "Invalid PA magic"))]
    pub magic: [u8; 2],

    /// Format version (1 or 2)
    pub version: u8,

    /// Size of file content keys in bytes (16 for MD5)
    pub file_key_size: u8,

    /// Size of old content keys in bytes (16 for MD5)
    pub old_key_size: u8,

    /// Size of patch encoding keys in bytes (16 for MD5)
    pub patch_key_size: u8,

    /// Block size as power of 2 (16 = 64KB blocks)
    pub block_size_bits: u8,

    /// Number of patch entries in this archive
    pub block_count: u16,

    /// Format flags (typically 0)
    pub flags: u8,
}

impl PatchArchiveHeader {
    /// Create new header with standard values
    pub fn new(block_count: u16) -> Self {
        Self {
            magic: *b"PA",
            version: 2,
            file_key_size: STANDARD_KEY_SIZE,
            old_key_size: STANDARD_KEY_SIZE,
            patch_key_size: STANDARD_KEY_SIZE,
            block_size_bits: STANDARD_BLOCK_SIZE_BITS,
            block_count,
            flags: 0,
        }
    }

    /// Get block size in bytes
    pub fn block_size(&self) -> usize {
        1 << self.block_size_bits
    }

    /// Validate header fields
    pub fn validate(&self) -> PatchArchiveResult<()> {
        if &self.magic != b"PA" {
            return Err(PatchArchiveError::InvalidMagic(self.magic));
        }

        if self.version == 0 || self.version > 2 {
            return Err(PatchArchiveError::UnsupportedVersion(self.version));
        }

        if self.file_key_size != 16 || self.old_key_size != 16 || self.patch_key_size != 16 {
            return Err(PatchArchiveError::InvalidKeySize {
                file: self.file_key_size,
                old: self.old_key_size,
                patch: self.patch_key_size,
            });
        }

        if self.block_size_bits < 12 || self.block_size_bits > 24 {
            return Err(PatchArchiveError::InvalidBlockSize(self.block_size_bits));
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    #[test]
    fn test_header_creation() {
        let header = PatchArchiveHeader::new(42);

        assert_eq!(header.magic, *b"PA");
        assert_eq!(header.version, 2);
        assert_eq!(header.file_key_size, 16);
        assert_eq!(header.old_key_size, 16);
        assert_eq!(header.patch_key_size, 16);
        assert_eq!(header.block_size_bits, 16);
        assert_eq!(header.block_count, 42);
        assert_eq!(header.flags, 0);
        assert_eq!(header.block_size(), 65536);
        assert!(header.validate().is_ok());
    }

    #[test]
    fn test_header_round_trip() {
        let header = PatchArchiveHeader::new(123);

        // Serialize
        let mut writer = Vec::new();
        header
            .write_options(&mut Cursor::new(&mut writer), binrw::Endian::Big, ())
            .expect("Operation should succeed");

        // Check expected size
        assert_eq!(writer.len(), 10);

        // Deserialize
        let parsed =
            PatchArchiveHeader::read_options(&mut Cursor::new(&writer), binrw::Endian::Big, ())
                .expect("Operation should succeed");

        assert_eq!(parsed, header);
    }

    #[test]
    fn test_header_validation() {
        let mut header = PatchArchiveHeader::new(1);

        // Test version 0 is invalid
        header.version = 0;
        assert!(header.validate().is_err());

        // Test version 1 is valid
        header.version = 1;
        assert!(header.validate().is_ok());

        // Test version 3 is invalid
        header.version = 3;
        assert!(header.validate().is_err());

        // Test invalid key size
        header.version = 2;
        header.file_key_size = 8;
        assert!(header.validate().is_err());

        // Test invalid block size
        header.file_key_size = 16;
        header.block_size_bits = 11; // Too small (min is 12)
        assert!(header.validate().is_err());

        header.block_size_bits = 25; // Too large (max is 24)
        assert!(header.validate().is_err());

        // Test boundary values are accepted
        header.block_size_bits = 12;
        assert!(header.validate().is_ok());
        header.block_size_bits = 24;
        assert!(header.validate().is_ok());
    }
}

//! Install manifest header parsing and building

use crate::install::error::{InstallError, Result};
use binrw::{BinRead, BinWrite};

/// Install manifest header
///
/// The header contains metadata about the manifest structure:
/// - Magic signature "IN" (2 bytes)
/// - Version number (1 byte, currently 1)
/// - Content key length (1 byte, typically 16 for MD5)
/// - Tag count (2 bytes, big-endian)
/// - Entry count (4 bytes, big-endian)
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)] // All multi-byte fields are big-endian
#[bw(big)]
pub struct InstallHeader {
    /// Magic signature, always "IN"
    #[br(assert(magic == *b"IN", "Invalid install magic: expected 'IN', got {:?}", magic))]
    pub magic: [u8; 2],

    /// Version number, currently 1
    pub version: u8,

    /// Content key length in bytes, typically 16 for MD5
    pub ckey_length: u8,

    /// Number of tags in the manifest
    pub tag_count: u16,

    /// Number of file entries in the manifest
    pub entry_count: u32,
}

impl InstallHeader {
    /// Create a new install header with specified counts
    pub fn new(tag_count: u16, entry_count: u32) -> Self {
        Self {
            magic: *b"IN",
            version: 1,
            ckey_length: 16,
            tag_count,
            entry_count,
        }
    }

    /// Calculate the size of bit masks for tags
    ///
    /// Each tag has a bit mask with one bit per file entry.
    /// The bit mask is stored as bytes, so we need (`entry_count` + 7) / 8 bytes.
    pub fn bit_mask_size(&self) -> usize {
        (self.entry_count as usize).div_ceil(8)
    }

    /// Validate the header fields
    pub fn validate(&self) -> Result<()> {
        if self.magic != *b"IN" {
            return Err(InstallError::InvalidMagic(self.magic));
        }

        if self.version != 1 {
            return Err(InstallError::UnsupportedVersion(self.version));
        }

        if self.ckey_length != 16 {
            return Err(InstallError::InvalidCKeyLength(self.ckey_length));
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
    fn test_header_new() {
        let header = InstallHeader::new(2, 5);
        assert_eq!(header.magic, *b"IN");
        assert_eq!(header.version, 1);
        assert_eq!(header.ckey_length, 16);
        assert_eq!(header.tag_count, 2);
        assert_eq!(header.entry_count, 5);
    }

    #[test]
    fn test_bit_mask_size() {
        // Test various entry counts
        assert_eq!(InstallHeader::new(0, 0).bit_mask_size(), 0);
        assert_eq!(InstallHeader::new(0, 1).bit_mask_size(), 1);
        assert_eq!(InstallHeader::new(0, 7).bit_mask_size(), 1);
        assert_eq!(InstallHeader::new(0, 8).bit_mask_size(), 1);
        assert_eq!(InstallHeader::new(0, 9).bit_mask_size(), 2);
        assert_eq!(InstallHeader::new(0, 16).bit_mask_size(), 2);
        assert_eq!(InstallHeader::new(0, 17).bit_mask_size(), 3);
    }

    #[test]
    fn test_header_validation() {
        let valid_header = InstallHeader::new(2, 5);
        assert!(valid_header.validate().is_ok());

        // Test invalid magic
        let mut invalid_magic = valid_header.clone();
        invalid_magic.magic = *b"XX";
        assert!(matches!(
            invalid_magic.validate(),
            Err(InstallError::InvalidMagic(_))
        ));

        // Test invalid version
        let mut invalid_version = valid_header.clone();
        invalid_version.version = 2;
        assert!(matches!(
            invalid_version.validate(),
            Err(InstallError::UnsupportedVersion(2))
        ));

        // Test invalid ckey length
        let mut invalid_ckey = valid_header;
        invalid_ckey.ckey_length = 20;
        assert!(matches!(
            invalid_ckey.validate(),
            Err(InstallError::InvalidCKeyLength(20))
        ));
    }

    #[test]
    fn test_header_parsing() {
        let data = [
            b'I', b'N', // Magic (2 bytes)
            1,    // Version (1 byte)
            16,   // CKey length (1 byte)
            0, 2, // Tag count (2 bytes, big-endian)
            0, 0, 0, 5, // Entry count (4 bytes, big-endian)
        ];

        let header =
            InstallHeader::read(&mut Cursor::new(&data)).expect("Operation should succeed");
        assert_eq!(header.magic, *b"IN");
        assert_eq!(header.version, 1);
        assert_eq!(header.ckey_length, 16);
        assert_eq!(header.tag_count, 2);
        assert_eq!(header.entry_count, 5);
        assert_eq!(header.bit_mask_size(), 1); // (5 + 7) / 8 = 1
    }

    #[test]
    fn test_header_round_trip() {
        let original = InstallHeader::new(10, 100);

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write(&mut cursor)
            .expect("Operation should succeed");

        // Deserialize
        let parsed =
            InstallHeader::read(&mut Cursor::new(&buffer)).expect("Operation should succeed");

        assert_eq!(original, parsed);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_header_big_endian() {
        let header = InstallHeader {
            magic: *b"IN",
            version: 1,
            ckey_length: 16,
            tag_count: 0x1234,        // Big value to test endianness
            entry_count: 0x1234_5678, // Big value to test endianness
        };

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header.write(&mut cursor).expect("Operation should succeed");

        // Check that multi-byte fields are stored in big-endian
        assert_eq!(buffer[0], b'I');
        assert_eq!(buffer[1], b'N');
        assert_eq!(buffer[2], 1);
        assert_eq!(buffer[3], 16);
        // tag_count 0x1234 should be stored as [0x12, 0x34]
        assert_eq!(buffer[4], 0x12);
        assert_eq!(buffer[5], 0x34);
        // entry_count 0x12345678 should be stored as [0x12, 0x34, 0x56, 0x78]
        assert_eq!(buffer[6], 0x12);
        assert_eq!(buffer[7], 0x34);
        assert_eq!(buffer[8], 0x56);
        assert_eq!(buffer[9], 0x78);

        // Verify round-trip
        let parsed =
            InstallHeader::read(&mut Cursor::new(&buffer)).expect("Operation should succeed");
        assert_eq!(header, parsed);
    }

    #[test]
    fn test_header_assertion_error() {
        // Test that invalid magic triggers assertion error
        let data = [
            b'X', b'X', // Invalid magic
            1,    // Version
            16,   // CKey length
            0, 2, // Tag count
            0, 0, 0, 5, // Entry count
        ];

        let result = InstallHeader::read(&mut Cursor::new(&data));
        assert!(result.is_err());
        // The assertion error contains the expected magic validation message
        let error_msg = format!("{:?}", result.expect_err("Test operation should fail"));
        assert!(error_msg.contains("Invalid install magic") || error_msg.contains("assertion"));
    }
}

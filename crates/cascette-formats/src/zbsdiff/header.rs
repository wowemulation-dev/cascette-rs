//! ZBSDIFF1 header structure and validation
//!
//! The ZBSDIFF1 header is a 32-byte big-endian structure that contains:
//! - 8-byte signature "ZBSDIFF1"
//! - 8-byte control block size
//! - 8-byte diff block size
//! - 8-byte output file size

use crate::zbsdiff::error::ZbsdiffError;
use binrw::{BinRead, BinWrite};

/// ZBSDIFF1 signature: "ZBSDIFF1" as little-endian u64
pub const ZBSDIFF1_SIGNATURE: u64 = u64::from_le_bytes(*b"ZBSDIFF1");

/// ZBSDIFF1 header structure (32 bytes, little-endian)
///
/// All integer fields are little-endian, matching the original bsdiff format
/// and verified against Agent.exe `tact::BsPatch::ParseHeader` at 0x6fbd1c.
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(little)]
#[bw(little)]
pub struct ZbsdiffHeader {
    /// File signature, must be ZBSDIFF1_SIGNATURE
    #[br(assert(signature == ZBSDIFF1_SIGNATURE, "Invalid ZBSDIFF1 signature: expected {:#x}, got {:#x}", ZBSDIFF1_SIGNATURE, signature))]
    pub signature: u64,

    /// Size of compressed control block in bytes
    pub control_size: i64,

    /// Size of compressed diff block in bytes
    pub diff_size: i64,

    /// Size of final output file in bytes
    pub output_size: i64,
}

impl ZbsdiffHeader {
    /// Create a new ZBSDIFF1 header with the given sizes
    pub fn new(control_size: i64, diff_size: i64, output_size: i64) -> Result<Self, ZbsdiffError> {
        let header = Self {
            signature: ZBSDIFF1_SIGNATURE,
            control_size,
            diff_size,
            output_size,
        };

        header.validate()?;
        Ok(header)
    }

    /// Validate header fields for correctness and reasonable limits
    pub fn validate(&self) -> Result<(), ZbsdiffError> {
        // Reasonable size limits to prevent DoS attacks (1GB)
        const MAX_SIZE: i64 = 1_000_000_000;

        // Check signature
        if self.signature != ZBSDIFF1_SIGNATURE {
            return Err(ZbsdiffError::InvalidSignature {
                expected: ZBSDIFF1_SIGNATURE,
                actual: self.signature,
            });
        }

        // Check for negative sizes
        if self.control_size < 0 {
            return Err(ZbsdiffError::InvalidSize {
                field: "control_size",
                value: self.control_size,
            });
        }

        if self.diff_size < 0 {
            return Err(ZbsdiffError::InvalidSize {
                field: "diff_size",
                value: self.diff_size,
            });
        }

        if self.output_size < 0 {
            return Err(ZbsdiffError::InvalidSize {
                field: "output_size",
                value: self.output_size,
            });
        }

        if self.control_size > MAX_SIZE {
            return Err(ZbsdiffError::SizeTooLarge(self.control_size));
        }

        if self.diff_size > MAX_SIZE {
            return Err(ZbsdiffError::SizeTooLarge(self.diff_size));
        }

        if self.output_size > MAX_SIZE {
            return Err(ZbsdiffError::SizeTooLarge(self.output_size));
        }

        // Check for overflow when adding sizes
        if let Some(total_compressed) = self.control_size.checked_add(self.diff_size) {
            if total_compressed > MAX_SIZE {
                return Err(ZbsdiffError::SizeTooLarge(total_compressed));
            }
        } else {
            return Err(ZbsdiffError::SizeTooLarge(i64::MAX));
        }

        Ok(())
    }

    /// Calculate the minimum total patch file size (header + compressed blocks)
    pub fn minimum_patch_size(&self) -> usize {
        32 + (self.control_size as usize) + (self.diff_size as usize)
    }

    /// Get the total size of all compressed data blocks
    pub fn compressed_data_size(&self) -> i64 {
        self.control_size + self.diff_size
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use binrw::{BinRead, BinWrite};
    use std::io::Cursor;

    #[test]
    fn test_zbsdiff_header_new() {
        let header = ZbsdiffHeader::new(100, 200, 1000).expect("Operation should succeed");

        assert_eq!(header.signature, ZBSDIFF1_SIGNATURE);
        assert_eq!(header.control_size, 100);
        assert_eq!(header.diff_size, 200);
        assert_eq!(header.output_size, 1000);
    }

    #[test]
    fn test_zbsdiff_header_validation() {
        // Valid header
        let valid = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: 100,
            diff_size: 200,
            output_size: 1000,
        };
        assert!(valid.validate().is_ok());

        // Invalid signature
        let invalid_sig = ZbsdiffHeader {
            signature: 0x1234_5678_90AB_CDEF,
            control_size: 100,
            diff_size: 200,
            output_size: 1000,
        };
        assert!(matches!(
            invalid_sig.validate(),
            Err(ZbsdiffError::InvalidSignature { .. })
        ));

        // Negative control size
        let negative_control = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: -1,
            diff_size: 200,
            output_size: 1000,
        };
        assert!(matches!(
            negative_control.validate(),
            Err(ZbsdiffError::InvalidSize {
                field: "control_size",
                ..
            })
        ));

        // Too large size
        let too_large = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: 2_000_000_000,
            diff_size: 200,
            output_size: 1000,
        };
        assert!(matches!(
            too_large.validate(),
            Err(ZbsdiffError::SizeTooLarge(_))
        ));
    }

    #[test]
    fn test_zbsdiff_header_serialization() {
        let header = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: 100,
            diff_size: 200,
            output_size: 1000,
        };

        // Test serialization
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write_options(&mut cursor, binrw::Endian::Little, ())
            .expect("Operation should succeed");

        assert_eq!(buffer.len(), 32); // Header is always 32 bytes
        assert_eq!(&buffer[0..8], &ZBSDIFF1_SIGNATURE.to_le_bytes());
        assert_eq!(&buffer[8..16], &100i64.to_le_bytes());
        assert_eq!(&buffer[16..24], &200i64.to_le_bytes());
        assert_eq!(&buffer[24..32], &1000i64.to_le_bytes());
    }

    #[test]
    fn test_zbsdiff_header_deserialization() {
        // Create test header bytes in little-endian
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&ZBSDIFF1_SIGNATURE.to_le_bytes());
        buffer.extend_from_slice(&100i64.to_le_bytes());
        buffer.extend_from_slice(&200i64.to_le_bytes());
        buffer.extend_from_slice(&1000i64.to_le_bytes());

        let mut cursor = Cursor::new(&buffer);
        let header = ZbsdiffHeader::read_options(&mut cursor, binrw::Endian::Little, ())
            .expect("Operation should succeed");

        assert_eq!(header.signature, ZBSDIFF1_SIGNATURE);
        assert_eq!(header.control_size, 100);
        assert_eq!(header.diff_size, 200);
        assert_eq!(header.output_size, 1000);
    }

    #[test]
    fn test_zbsdiff_header_round_trip() {
        let original = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: 123,
            diff_size: 456,
            output_size: 789,
        };

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_options(&mut cursor, binrw::Endian::Little, ())
            .expect("Operation should succeed");

        // Deserialize
        let mut cursor = Cursor::new(&buffer);
        let deserialized = ZbsdiffHeader::read_options(&mut cursor, binrw::Endian::Little, ())
            .expect("Operation should succeed");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_zbsdiff_header_signature_constant() {
        // Verify the signature constant matches "ZBSDIFF1" in little-endian
        let expected_bytes = b"ZBSDIFF1";
        let signature_bytes = ZBSDIFF1_SIGNATURE.to_le_bytes();

        assert_eq!(expected_bytes, &signature_bytes);
    }

    #[test]
    fn test_zbsdiff_header_utility_methods() {
        let header = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: 100,
            diff_size: 200,
            output_size: 1000,
        };

        assert_eq!(header.minimum_patch_size(), 32 + 100 + 200);
        assert_eq!(header.compressed_data_size(), 300);
    }
}

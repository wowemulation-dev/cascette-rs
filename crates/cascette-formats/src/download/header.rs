//! Download manifest headers with version-aware parsing

use crate::download::error::{DownloadError, Result};
use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Read, Seek, Write};

/// Base header structure common to all download manifest versions
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct DownloadHeaderBase {
    /// Magic bytes "DL" identifying download manifest
    #[br(assert(magic == *b"DL", "Invalid download magic: expected 'DL', got {:?}", magic))]
    pub magic: [u8; 2],
    /// Format version (1, 2, or 3)
    pub version: u8,
    /// Encoding key length in bytes (always 16 for MD5)
    pub ekey_length: u8,
    /// Boolean flag indicating if entries have checksums (0=false, 1=true)
    pub has_checksum: u8,
    /// Number of file entries in manifest
    pub entry_count: u32,
    /// Number of tags in manifest
    pub tag_count: u16,
}

/// Version 2 header with flag support
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct DownloadHeaderV2 {
    /// Magic bytes "DL" identifying download manifest
    #[br(assert(magic == *b"DL", "Invalid download magic: expected 'DL', got {:?}", magic))]
    pub magic: [u8; 2],
    /// Format version (1, 2, or 3)
    pub version: u8,
    /// Encoding key length in bytes (always 16 for MD5)
    pub ekey_length: u8,
    /// Boolean flag indicating if entries have checksums (0=false, 1=true)
    pub has_checksum: u8,
    /// Number of file entries in manifest
    pub entry_count: u32,
    /// Number of tags in manifest
    pub tag_count: u16,
    /// Size of flag field per entry (0 = no flags)
    pub flag_size: u8,
}

/// Version 3 header with flag and base priority support
#[derive(Debug, Clone, PartialEq, Eq, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct DownloadHeaderV3 {
    /// Magic bytes "DL" identifying download manifest
    #[br(assert(magic == *b"DL", "Invalid download magic: expected 'DL', got {:?}", magic))]
    pub magic: [u8; 2],
    /// Format version (1, 2, or 3)
    pub version: u8,
    /// Encoding key length in bytes (always 16 for MD5)
    pub ekey_length: u8,
    /// Boolean flag indicating if entries have checksums (0=false, 1=true)
    pub has_checksum: u8,
    /// Number of file entries in manifest
    pub entry_count: u32,
    /// Number of tags in manifest
    pub tag_count: u16,
    /// Size of flag field per entry (0 = no flags)
    pub flag_size: u8,
    /// Base priority adjustment (signed byte)
    pub base_priority: i8,
    /// Reserved bytes (must be zero)
    pub reserved: [u8; 3],
}

/// Version-aware download header enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadHeader {
    /// Version 1 header
    V1(DownloadHeaderBase),
    /// Version 2 header
    V2(DownloadHeaderV2),
    /// Version 3 header
    V3(DownloadHeaderV3),
}

impl DownloadHeader {
    /// Get the format version
    pub fn version(&self) -> u8 {
        match self {
            Self::V1(h) => h.version,
            Self::V2(h) => h.version,
            Self::V3(h) => h.version,
        }
    }

    /// Get the number of file entries
    pub fn entry_count(&self) -> u32 {
        match self {
            Self::V1(h) => h.entry_count,
            Self::V2(h) => h.entry_count,
            Self::V3(h) => h.entry_count,
        }
    }

    /// Get the number of tags
    pub fn tag_count(&self) -> u16 {
        match self {
            Self::V1(h) => h.tag_count,
            Self::V2(h) => h.tag_count,
            Self::V3(h) => h.tag_count,
        }
    }

    /// Check if entries have checksums
    pub fn has_checksum(&self) -> bool {
        match self {
            Self::V1(h) => h.has_checksum != 0,
            Self::V2(h) => h.has_checksum != 0,
            Self::V3(h) => h.has_checksum != 0,
        }
    }

    /// Get the flag size (0 for V1)
    pub fn flag_size(&self) -> u8 {
        match self {
            Self::V1(_) => 0,
            Self::V2(h) => h.flag_size,
            Self::V3(h) => h.flag_size,
        }
    }

    /// Get the base priority adjustment (0 for V1/V2)
    pub fn base_priority(&self) -> i8 {
        match self {
            Self::V1(_) | Self::V2(_) => 0,
            Self::V3(h) => h.base_priority,
        }
    }

    /// Get the encoding key length
    pub fn ekey_length(&self) -> u8 {
        match self {
            Self::V1(h) => h.ekey_length,
            Self::V2(h) => h.ekey_length,
            Self::V3(h) => h.ekey_length,
        }
    }

    /// Calculate bit mask size for tags
    pub fn bit_mask_size(&self) -> usize {
        (self.entry_count() as usize).div_ceil(8)
    }

    /// Calculate total header size in bytes
    pub fn header_size(&self) -> usize {
        match self {
            Self::V1(_) => 11, // magic(2) + version(1) + ekey_length(1) + has_checksum(1) + entry_count(4) + tag_count(2)
            Self::V2(_) => 12, // V1 + flag_size(1)
            Self::V3(_) => 16, // V2 + base_priority(1) + reserved(3)
        }
    }

    /// Validate header consistency
    pub fn validate(&self) -> Result<()> {
        // Check magic
        let magic = match self {
            Self::V1(h) => h.magic,
            Self::V2(h) => h.magic,
            Self::V3(h) => h.magic,
        };
        if magic != *b"DL" {
            return Err(DownloadError::InvalidMagic(magic));
        }

        // Check version
        let version = self.version();
        if !(1..=3).contains(&version) {
            return Err(DownloadError::UnsupportedVersion(version));
        }

        // Check encoding key length
        let ekey_length = self.ekey_length();
        if ekey_length != 16 {
            return Err(DownloadError::InvalidEncodingKeyLength(ekey_length));
        }

        Ok(())
    }

    /// Create a new V1 header
    pub fn new_v1(entry_count: u32, tag_count: u16, has_checksum: bool) -> Self {
        Self::V1(DownloadHeaderBase {
            magic: *b"DL",
            version: 1,
            ekey_length: 16,
            has_checksum: u8::from(has_checksum),
            entry_count,
            tag_count,
        })
    }

    /// Create a new V2 header
    pub fn new_v2(entry_count: u32, tag_count: u16, has_checksum: bool, flag_size: u8) -> Self {
        Self::V2(DownloadHeaderV2 {
            magic: *b"DL",
            version: 2,
            ekey_length: 16,
            has_checksum: u8::from(has_checksum),
            entry_count,
            tag_count,
            flag_size,
        })
    }

    /// Create a new V3 header
    pub fn new_v3(
        entry_count: u32,
        tag_count: u16,
        has_checksum: bool,
        flag_size: u8,
        base_priority: i8,
    ) -> Self {
        Self::V3(DownloadHeaderV3 {
            magic: *b"DL",
            version: 3,
            ekey_length: 16,
            has_checksum: u8::from(has_checksum),
            entry_count,
            tag_count,
            flag_size,
            base_priority,
            reserved: [0, 0, 0],
        })
    }
}

impl BinRead for DownloadHeader {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read base header first to determine version
        let base = DownloadHeaderBase::read_options(reader, endian, ())?;

        match base.version {
            1 => Ok(Self::V1(base)),
            2 => {
                // Read additional V2 field
                let flag_size = u8::read_options(reader, endian, ())?;
                Ok(Self::V2(DownloadHeaderV2 {
                    magic: base.magic,
                    version: base.version,
                    ekey_length: base.ekey_length,
                    has_checksum: base.has_checksum,
                    entry_count: base.entry_count,
                    tag_count: base.tag_count,
                    flag_size,
                }))
            }
            3 => {
                // Read V2 field first
                let flag_size = u8::read_options(reader, endian, ())?;
                // Then V3 fields
                let base_priority = i8::read_options(reader, endian, ())?;
                let mut reserved = [0u8; 3];
                reader.read_exact(&mut reserved)?;

                Ok(Self::V3(DownloadHeaderV3 {
                    magic: base.magic,
                    version: base.version,
                    ekey_length: base.ekey_length,
                    has_checksum: base.has_checksum,
                    entry_count: base.entry_count,
                    tag_count: base.tag_count,
                    flag_size,
                    base_priority,
                    reserved,
                }))
            }
            v => Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(DownloadError::UnsupportedVersion(v)),
            }),
        }
    }
}

impl BinWrite for DownloadHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        match self {
            Self::V1(h) => h.write_options(writer, endian, ()),
            Self::V2(h) => h.write_options(writer, endian, ()),
            Self::V3(h) => h.write_options(writer, endian, ()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    #[test]
    fn test_header_v1_creation() {
        let header = DownloadHeader::new_v1(100, 5, true);

        assert_eq!(header.version(), 1);
        assert_eq!(header.entry_count(), 100);
        assert_eq!(header.tag_count(), 5);
        assert!(header.has_checksum());
        assert_eq!(header.flag_size(), 0);
        assert_eq!(header.base_priority(), 0);
        assert_eq!(header.header_size(), 11);
    }

    #[test]
    fn test_header_v2_creation() {
        let header = DownloadHeader::new_v2(200, 10, false, 2);

        assert_eq!(header.version(), 2);
        assert_eq!(header.entry_count(), 200);
        assert_eq!(header.tag_count(), 10);
        assert!(!header.has_checksum());
        assert_eq!(header.flag_size(), 2);
        assert_eq!(header.base_priority(), 0);
        assert_eq!(header.header_size(), 12);
    }

    #[test]
    fn test_header_v3_creation() {
        let header = DownloadHeader::new_v3(300, 15, true, 1, -5);

        assert_eq!(header.version(), 3);
        assert_eq!(header.entry_count(), 300);
        assert_eq!(header.tag_count(), 15);
        assert!(header.has_checksum());
        assert_eq!(header.flag_size(), 1);
        assert_eq!(header.base_priority(), -5);
        assert_eq!(header.header_size(), 16);
    }

    #[test]
    fn test_header_validation() {
        // Valid headers
        let v1 = DownloadHeader::new_v1(10, 2, false);
        assert!(v1.validate().is_ok());

        let v2 = DownloadHeader::new_v2(20, 4, true, 1);
        assert!(v2.validate().is_ok());

        let v3 = DownloadHeader::new_v3(30, 6, false, 2, -3);
        assert!(v3.validate().is_ok());

        // Invalid magic
        let invalid_header = DownloadHeader::V1(DownloadHeaderBase {
            magic: *b"XX",
            version: 1,
            ekey_length: 16,
            has_checksum: 0,
            entry_count: 10,
            tag_count: 2,
        });
        assert!(matches!(
            invalid_header.validate(),
            Err(DownloadError::InvalidMagic(_))
        ));
    }

    #[test]
    fn test_bit_mask_size_calculation() {
        let header = DownloadHeader::new_v1(17, 1, false);
        assert_eq!(header.bit_mask_size(), 3); // (17 + 7) / 8 = 3

        let header = DownloadHeader::new_v2(8, 1, false, 0);
        assert_eq!(header.bit_mask_size(), 1); // (8 + 7) / 8 = 1

        let header = DownloadHeader::new_v3(1000, 1, false, 0, 0);
        assert_eq!(header.bit_mask_size(), 125); // (1000 + 7) / 8 = 125
    }

    #[test]
    fn test_header_parsing_v1() {
        let data = [
            b'D', b'L', // Magic
            1,    // Version
            16,   // EKey length
            1,    // Has checksum
            0, 0, 0, 10, // Entry count (big-endian)
            0, 5, // Tag count (big-endian)
        ];

        let header = DownloadHeader::read_options(&mut Cursor::new(&data), binrw::Endian::Big, ())
            .expect("Operation should succeed");

        assert_eq!(header.version(), 1);
        assert_eq!(header.entry_count(), 10);
        assert_eq!(header.tag_count(), 5);
        assert!(header.has_checksum());
        assert_eq!(header.flag_size(), 0);
        assert_eq!(header.base_priority(), 0);
    }

    #[test]
    fn test_header_parsing_v2() {
        let data = [
            b'D', b'L', // Magic
            2,    // Version
            16,   // EKey length
            0,    // No checksum
            0, 0, 0, 20, // Entry count (big-endian)
            0, 10, // Tag count (big-endian)
            2,  // Flag size
        ];

        let header = DownloadHeader::read_options(&mut Cursor::new(&data), binrw::Endian::Big, ())
            .expect("Operation should succeed");

        assert_eq!(header.version(), 2);
        assert_eq!(header.entry_count(), 20);
        assert_eq!(header.tag_count(), 10);
        assert!(!header.has_checksum());
        assert_eq!(header.flag_size(), 2);
        assert_eq!(header.base_priority(), 0);
    }

    #[test]
    fn test_header_parsing_v3() {
        let data = [
            b'D',
            b'L', // Magic
            3,    // Version
            16,   // EKey length
            1,    // Has checksum
            0,
            0,
            0,
            30, // Entry count (big-endian)
            0,
            15,            // Tag count (big-endian)
            1,             // Flag size
            (-1_i8) as u8, // Base priority (-1)
            0,
            0,
            0, // Reserved
        ];

        let header = DownloadHeader::read_options(&mut Cursor::new(&data), binrw::Endian::Big, ())
            .expect("Operation should succeed");

        assert_eq!(header.version(), 3);
        assert_eq!(header.entry_count(), 30);
        assert_eq!(header.tag_count(), 15);
        assert!(header.has_checksum());
        assert_eq!(header.flag_size(), 1);
        assert_eq!(header.base_priority(), -1);
    }

    #[test]
    fn test_header_round_trip() {
        let headers = vec![
            DownloadHeader::new_v1(100, 5, true),
            DownloadHeader::new_v2(200, 10, false, 2),
            DownloadHeader::new_v3(300, 15, true, 1, -5),
        ];

        for original in headers {
            // Serialize
            let mut buffer = Vec::new();
            let mut cursor = Cursor::new(&mut buffer);
            original
                .write_options(&mut cursor, binrw::Endian::Big, ())
                .expect("Operation should succeed");

            // Deserialize
            let parsed =
                DownloadHeader::read_options(&mut Cursor::new(&buffer), binrw::Endian::Big, ())
                    .expect("Operation should succeed");

            assert_eq!(original, parsed);
        }
    }

    #[test]
    fn test_unsupported_version() {
        let data = [
            b'D', b'L', // Magic
            99,   // Invalid version
            16,   // EKey length
            0,    // No checksum
            0, 0, 0, 10, // Entry count
            0, 5, // Tag count
        ];

        let result = DownloadHeader::read_options(&mut Cursor::new(&data), binrw::Endian::Big, ());

        assert!(result.is_err());
        match result.expect_err("Test operation should fail") {
            binrw::Error::Custom { err, .. } => {
                assert!(matches!(
                    err.downcast_ref::<DownloadError>()
                        .expect("Operation should succeed"),
                    DownloadError::UnsupportedVersion(99)
                ));
            }
            _ => unreachable!("Expected custom error"),
        }
    }

    #[test]
    fn test_nonzero_reserved_field_accepted() {
        let data = [
            b'D', b'L', // Magic
            3,    // Version
            16,   // EKey length
            0,    // No checksum
            0, 0, 0, 10, // Entry count
            0, 5, // Tag count
            0, // Flag size
            0, // Base priority
            1, 2, 3, // Non-zero reserved field (accepted per Agent behavior)
        ];

        let header = DownloadHeader::read_options(&mut Cursor::new(&data), binrw::Endian::Big, ())
            .expect("Non-zero reserved bytes should be accepted");

        assert_eq!(header.version(), 3);
        assert_eq!(header.entry_count(), 10);
        // validate() also should not reject non-zero reserved
        assert!(header.validate().is_ok());
    }
}

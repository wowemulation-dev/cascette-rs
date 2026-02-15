//! Versioned header for the Size manifest format
//!
//! The Size manifest header has a 10-byte base that is common across versions,
//! followed by version-specific extensions:
//!
//! - V1: u64 total_size + u8 esize_bytes (19 bytes total)
//! - V2: 5-byte (40-bit) total_size, esize fixed at 4 (15 bytes total)

use crate::size::error::{Result, SizeError};
use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Read, Seek, Write};

/// Version 1 size manifest header (19 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeHeaderV1 {
    /// Magic bytes "DS"
    pub magic: [u8; 2],
    /// Format version (1)
    pub version: u8,
    /// Flags byte
    pub flags: u8,
    /// Number of entries
    pub entry_count: u32,
    /// Key size in bits
    pub key_size_bits: u16,
    /// Total estimated size across all entries
    pub total_size: u64,
    /// Byte width of esize per entry (1-8)
    pub esize_bytes: u8,
}

/// Version 2 size manifest header (15 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeHeaderV2 {
    /// Magic bytes "DS"
    pub magic: [u8; 2],
    /// Format version (2)
    pub version: u8,
    /// Flags byte
    pub flags: u8,
    /// Number of entries
    pub entry_count: u32,
    /// Key size in bits
    pub key_size_bits: u16,
    /// Total estimated size as 40-bit value (max ~1TB)
    pub total_size: u64,
}

/// Version-aware size manifest header
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeHeader {
    /// Version 1 header (19 bytes)
    V1(SizeHeaderV1),
    /// Version 2 header (15 bytes)
    V2(SizeHeaderV2),
}

impl SizeHeader {
    /// Create a new V1 header
    pub fn new_v1(
        flags: u8,
        entry_count: u32,
        key_size_bits: u16,
        total_size: u64,
        esize_bytes: u8,
    ) -> Self {
        Self::V1(SizeHeaderV1 {
            magic: *b"DS",
            version: 1,
            flags,
            entry_count,
            key_size_bits,
            total_size,
            esize_bytes,
        })
    }

    /// Create a new V2 header
    pub fn new_v2(flags: u8, entry_count: u32, key_size_bits: u16, total_size: u64) -> Self {
        Self::V2(SizeHeaderV2 {
            magic: *b"DS",
            version: 2,
            flags,
            entry_count,
            key_size_bits,
            total_size,
        })
    }

    /// Get the format version
    pub fn version(&self) -> u8 {
        match self {
            Self::V1(h) => h.version,
            Self::V2(h) => h.version,
        }
    }

    /// Get the number of entries
    pub fn entry_count(&self) -> u32 {
        match self {
            Self::V1(h) => h.entry_count,
            Self::V2(h) => h.entry_count,
        }
    }

    /// Get the key size in bits
    pub fn key_size_bits(&self) -> u16 {
        match self {
            Self::V1(h) => h.key_size_bits,
            Self::V2(h) => h.key_size_bits,
        }
    }

    /// Get the key size in bytes (ceiling division of bits / 8)
    pub fn key_size_bytes(&self) -> usize {
        (self.key_size_bits() as usize).div_ceil(8)
    }

    /// Get the byte width of the esize field per entry
    ///
    /// V1: configurable via `esize_bytes` header field (1-8)
    /// V2: fixed at 4
    pub fn esize_bytes(&self) -> u8 {
        match self {
            Self::V1(h) => h.esize_bytes,
            Self::V2(_) => 4,
        }
    }

    /// Get the total estimated size across all entries
    pub fn total_size(&self) -> u64 {
        match self {
            Self::V1(h) => h.total_size,
            Self::V2(h) => h.total_size,
        }
    }

    /// Get the flags byte
    pub fn flags(&self) -> u8 {
        match self {
            Self::V1(h) => h.flags,
            Self::V2(h) => h.flags,
        }
    }

    /// Get the header size in bytes
    pub fn header_size(&self) -> usize {
        match self {
            Self::V1(_) => 19,
            Self::V2(_) => 15,
        }
    }

    /// Validate header fields
    pub fn validate(&self) -> Result<()> {
        let magic = match self {
            Self::V1(h) => h.magic,
            Self::V2(h) => h.magic,
        };
        if magic != *b"DS" {
            return Err(SizeError::InvalidMagic(magic));
        }

        let version = self.version();
        if version == 0 || version > 2 {
            return Err(SizeError::UnsupportedVersion(version));
        }

        if self.key_size_bits() == 0 {
            return Err(SizeError::InvalidKeySize);
        }

        if let Self::V1(h) = self
            && (h.esize_bytes == 0 || h.esize_bytes > 8)
        {
            return Err(SizeError::InvalidEsizeWidth(h.esize_bytes));
        }

        Ok(())
    }
}

impl BinRead for SizeHeader {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read 10-byte base header
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;

        let mut buf1 = [0u8; 1];
        reader.read_exact(&mut buf1)?;
        let version = buf1[0];

        reader.read_exact(&mut buf1)?;
        let flags = buf1[0];

        let mut buf4 = [0u8; 4];
        reader.read_exact(&mut buf4)?;
        let entry_count = u32::from_be_bytes(buf4);

        let mut buf2 = [0u8; 2];
        reader.read_exact(&mut buf2)?;
        let key_size_bits = u16::from_be_bytes(buf2);

        match version {
            1 => {
                // V1: read u64 total_size + u8 esize_bytes
                let mut buf8 = [0u8; 8];
                reader.read_exact(&mut buf8)?;
                let total_size = u64::from_be_bytes(buf8);

                reader.read_exact(&mut buf1)?;
                let esize_bytes = buf1[0];

                Ok(Self::V1(SizeHeaderV1 {
                    magic,
                    version,
                    flags,
                    entry_count,
                    key_size_bits,
                    total_size,
                    esize_bytes,
                }))
            }
            2 => {
                // V2: read 5-byte (40-bit) total_size
                let mut buf5 = [0u8; 5];
                reader.read_exact(&mut buf5)?;
                let total_size = (u64::from(buf5[0]) << 32)
                    | (u64::from(buf5[1]) << 24)
                    | (u64::from(buf5[2]) << 16)
                    | (u64::from(buf5[3]) << 8)
                    | u64::from(buf5[4]);

                Ok(Self::V2(SizeHeaderV2 {
                    magic,
                    version,
                    flags,
                    entry_count,
                    key_size_bits,
                    total_size,
                }))
            }
            v => Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(SizeError::UnsupportedVersion(v)),
            }),
        }
    }
}

impl BinWrite for SizeHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        match self {
            Self::V1(h) => {
                writer.write_all(&h.magic)?;
                writer.write_all(&[h.version])?;
                writer.write_all(&[h.flags])?;
                writer.write_all(&h.entry_count.to_be_bytes())?;
                writer.write_all(&h.key_size_bits.to_be_bytes())?;
                writer.write_all(&h.total_size.to_be_bytes())?;
                writer.write_all(&[h.esize_bytes])?;
            }
            Self::V2(h) => {
                writer.write_all(&h.magic)?;
                writer.write_all(&[h.version])?;
                writer.write_all(&[h.flags])?;
                writer.write_all(&h.entry_count.to_be_bytes())?;
                writer.write_all(&h.key_size_bits.to_be_bytes())?;
                // Write 40-bit total_size as 5 bytes BE
                let bytes = [
                    (h.total_size >> 32) as u8,
                    (h.total_size >> 24) as u8,
                    (h.total_size >> 16) as u8,
                    (h.total_size >> 8) as u8,
                    h.total_size as u8,
                ];
                writer.write_all(&bytes)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    #[test]
    fn test_header_v1_creation() {
        let header = SizeHeader::new_v1(0, 100, 128, 50000, 4);

        assert_eq!(header.version(), 1);
        assert_eq!(header.entry_count(), 100);
        assert_eq!(header.key_size_bits(), 128);
        assert_eq!(header.key_size_bytes(), 16);
        assert_eq!(header.esize_bytes(), 4);
        assert_eq!(header.total_size(), 50000);
        assert_eq!(header.flags(), 0);
        assert_eq!(header.header_size(), 19);
    }

    #[test]
    fn test_header_v2_creation() {
        let header = SizeHeader::new_v2(0, 200, 128, 100000);

        assert_eq!(header.version(), 2);
        assert_eq!(header.entry_count(), 200);
        assert_eq!(header.key_size_bits(), 128);
        assert_eq!(header.key_size_bytes(), 16);
        assert_eq!(header.esize_bytes(), 4); // V2 fixed at 4
        assert_eq!(header.total_size(), 100000);
        assert_eq!(header.flags(), 0);
        assert_eq!(header.header_size(), 15);
    }

    #[test]
    fn test_key_size_bytes_rounding() {
        // 128 bits -> 16 bytes (exact)
        let header = SizeHeader::new_v1(0, 0, 128, 0, 4);
        assert_eq!(header.key_size_bytes(), 16);

        // 129 bits -> 17 bytes (rounded up)
        let header = SizeHeader::new_v1(0, 0, 129, 0, 4);
        assert_eq!(header.key_size_bytes(), 17);

        // 1 bit -> 1 byte
        let header = SizeHeader::new_v1(0, 0, 1, 0, 4);
        assert_eq!(header.key_size_bytes(), 1);
    }

    #[test]
    fn test_header_v1_parse_from_bytes() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DS"); // magic
        data.push(1); // version
        data.push(0); // flags
        data.extend_from_slice(&10u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&128u16.to_be_bytes()); // key_size_bits
        data.extend_from_slice(&5000u64.to_be_bytes()); // total_size
        data.push(4); // esize_bytes

        let mut cursor = Cursor::new(&data);
        let header = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Should parse V1 header");

        assert_eq!(header.version(), 1);
        assert_eq!(header.entry_count(), 10);
        assert_eq!(header.key_size_bits(), 128);
        assert_eq!(header.total_size(), 5000);
        assert_eq!(header.esize_bytes(), 4);
    }

    #[test]
    fn test_header_v2_parse_from_bytes() {
        let total: u64 = 0x12_3456_7890;
        let mut data = Vec::new();
        data.extend_from_slice(b"DS"); // magic
        data.push(2); // version
        data.push(0); // flags
        data.extend_from_slice(&10u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&128u16.to_be_bytes()); // key_size_bits
        // 40-bit total_size
        data.push((total >> 32) as u8);
        data.push((total >> 24) as u8);
        data.push((total >> 16) as u8);
        data.push((total >> 8) as u8);
        data.push(total as u8);

        let mut cursor = Cursor::new(&data);
        let header = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Should parse V2 header");

        assert_eq!(header.version(), 2);
        assert_eq!(header.entry_count(), 10);
        assert_eq!(header.key_size_bits(), 128);
        assert_eq!(header.total_size(), total);
        assert_eq!(header.esize_bytes(), 4);
    }

    #[test]
    fn test_header_v1_round_trip() {
        let header = SizeHeader::new_v1(0x05, 42, 128, 999_999, 2);
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Should write V1 header");

        assert_eq!(buf.len(), 19);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Should parse V1 header");

        assert_eq!(header, parsed);
    }

    #[test]
    fn test_header_v2_round_trip() {
        let header = SizeHeader::new_v2(0x03, 1000, 128, 0xAB_CDEF_0123);
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Should write V2 header");

        assert_eq!(buf.len(), 15);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Should parse V2 header");

        assert_eq!(header, parsed);
    }

    #[test]
    fn test_reject_bad_magic() {
        let header = SizeHeader::V1(SizeHeaderV1 {
            magic: *b"XX",
            version: 1,
            flags: 0,
            entry_count: 0,
            key_size_bits: 128,
            total_size: 0,
            esize_bytes: 4,
        });
        assert!(header.validate().is_err());
    }

    #[test]
    fn test_reject_version_0() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DS");
        data.push(0); // version 0
        data.extend_from_slice(&[0u8; 16]); // padding

        let mut cursor = Cursor::new(&data);
        let result = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ());
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_version_3() {
        let mut data = Vec::new();
        data.extend_from_slice(b"DS");
        data.push(3); // version 3
        data.extend_from_slice(&[0u8; 16]); // padding

        let mut cursor = Cursor::new(&data);
        let result = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ());
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_v1_esize_bytes_0() {
        let header = SizeHeader::V1(SizeHeaderV1 {
            magic: *b"DS",
            version: 1,
            flags: 0,
            entry_count: 0,
            key_size_bits: 128,
            total_size: 0,
            esize_bytes: 0,
        });
        assert!(matches!(
            header.validate(),
            Err(SizeError::InvalidEsizeWidth(0))
        ));
    }

    #[test]
    fn test_reject_v1_esize_bytes_9() {
        let header = SizeHeader::V1(SizeHeaderV1 {
            magic: *b"DS",
            version: 1,
            flags: 0,
            entry_count: 0,
            key_size_bits: 128,
            total_size: 0,
            esize_bytes: 9,
        });
        assert!(matches!(
            header.validate(),
            Err(SizeError::InvalidEsizeWidth(9))
        ));
    }

    #[test]
    fn test_reject_key_size_bits_zero() {
        let header = SizeHeader::new_v1(0, 0, 0, 0, 4);
        assert!(matches!(header.validate(), Err(SizeError::InvalidKeySize)));
    }

    #[test]
    fn test_header_size_values() {
        let v1 = SizeHeader::new_v1(0, 0, 128, 0, 4);
        assert_eq!(v1.header_size(), 19);

        let v2 = SizeHeader::new_v2(0, 0, 128, 0);
        assert_eq!(v2.header_size(), 15);
    }
}

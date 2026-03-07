//! Header for the Size manifest (`DS`) format
//!
//! Source: <https://wowdev.wiki/TACT#Size_manifest>
//!
//! Binary layout (15 bytes):
//!
//! | Offset | Size | Field     | Description                              |
//! |--------|------|-----------|------------------------------------------|
//! | 0-1    | 2    | signature | `"DS"` (0x44, 0x53)                      |
//! | 2      | 1    | version   | Format version (1)                       |
//! | 3      | 1    | ekey_size | EKey length in **bytes** (typically 9)   |
//! | 4-7    | 4    | num_files | Number of file entries (BE u32)          |
//! | 8-9    | 2    | num_tags  | Number of tags between header and files  |
//! | 10-14  | 5    | total_size| Sum of all file esizes (40-bit BE)       |

use crate::size::error::{Result, SizeError};
use std::io::{Read, Seek, Write};

/// Size manifest header (15 bytes, all multi-byte integers big-endian)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeHeader {
    /// Format version. Only version 1 is known in the wild.
    pub version: u8,
    /// EKey length per entry in bytes. Typically 9.
    pub ekey_size: u8,
    /// Number of file entries.
    pub num_files: u32,
    /// Number of tags between header and file entries.
    pub num_tags: u16,
    /// Sum of all per-file esize values (40-bit, max ~1 TB).
    pub total_size: u64,
}

impl SizeHeader {
    /// Construct a header.
    pub fn new(version: u8, ekey_size: u8, num_files: u32, num_tags: u16, total_size: u64) -> Self {
        Self {
            version,
            ekey_size,
            num_files,
            num_tags,
            total_size,
        }
    }

    /// Serialised byte size of the header.
    pub const SIZE: usize = 15;

    /// Validate header fields.
    pub fn validate(&self) -> Result<()> {
        if self.version == 0 {
            return Err(SizeError::UnsupportedVersion(self.version));
        }
        if self.ekey_size == 0 || self.ekey_size > 16 {
            return Err(SizeError::InvalidEKeySize(self.ekey_size));
        }
        Ok(())
    }

    /// Read from a byte slice starting at offset 0.
    ///
    /// The caller must verify that `data[0..2] == b"DS"` before calling.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(SizeError::TruncatedData {
                expected: Self::SIZE,
                actual: data.len(),
            });
        }
        if &data[0..2] != b"DS" {
            return Err(SizeError::InvalidMagic([data[0], data[1]]));
        }
        let version = data[2];
        let ekey_size = data[3];
        let num_files = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let num_tags = u16::from_be_bytes([data[8], data[9]]);
        let total_size = (u64::from(data[10]) << 32)
            | (u64::from(data[11]) << 24)
            | (u64::from(data[12]) << 16)
            | (u64::from(data[13]) << 8)
            | u64::from(data[14]);

        let hdr = Self {
            version,
            ekey_size,
            num_files,
            num_tags,
            total_size,
        };
        hdr.validate()?;
        Ok(hdr)
    }

    /// Write to a `Write` sink.
    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(b"DS")?;
        writer.write_all(&[self.version])?;
        writer.write_all(&[self.ekey_size])?;
        writer.write_all(&self.num_files.to_be_bytes())?;
        writer.write_all(&self.num_tags.to_be_bytes())?;
        // 40-bit total_size as 5 bytes BE
        writer.write_all(&[
            (self.total_size >> 32) as u8,
            (self.total_size >> 24) as u8,
            (self.total_size >> 16) as u8,
            (self.total_size >> 8) as u8,
            self.total_size as u8,
        ])?;
        Ok(())
    }
}

// binrw shims — the manifest uses custom IO to avoid pulling binrw into hot paths,
// but the rest of the codebase expects these traits for generic serialisation.
impl binrw::BinRead for SizeHeader {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let mut buf = [0u8; Self::SIZE];
        reader.read_exact(&mut buf)?;
        Self::parse(&buf).map_err(|e| binrw::Error::Custom {
            pos: 0,
            err: Box::new(e),
        })
    }
}

impl binrw::BinWrite for SizeHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        self.write(writer).map_err(|e| binrw::Error::Custom {
            pos: 0,
            err: Box::new(e),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_header_bytes(
        version: u8,
        ekey_size: u8,
        num_files: u32,
        num_tags: u16,
        total_size: u64,
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(SizeHeader::SIZE);
        data.extend_from_slice(b"DS");
        data.push(version);
        data.push(ekey_size);
        data.extend_from_slice(&num_files.to_be_bytes());
        data.extend_from_slice(&num_tags.to_be_bytes());
        data.push((total_size >> 32) as u8);
        data.push((total_size >> 24) as u8);
        data.push((total_size >> 16) as u8);
        data.push((total_size >> 8) as u8);
        data.push(total_size as u8);
        data
    }

    #[test]
    fn test_parse_header() {
        let data = make_header_bytes(1, 9, 204319, 23, 7_029_657_207);
        let hdr = SizeHeader::parse(&data).expect("Should parse header");
        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.ekey_size, 9);
        assert_eq!(hdr.num_files, 204319);
        assert_eq!(hdr.num_tags, 23);
        assert_eq!(hdr.total_size, 7_029_657_207);
    }

    #[test]
    fn test_round_trip() {
        let hdr = SizeHeader::new(1, 9, 1000, 5, 0x0001_2345_6789);
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        hdr.write(&mut cursor).expect("Should write");
        assert_eq!(buf.len(), SizeHeader::SIZE);
        let parsed = SizeHeader::parse(&buf).expect("Should parse");
        assert_eq!(hdr, parsed);
    }

    #[test]
    fn test_reject_bad_magic() {
        let mut data = make_header_bytes(1, 9, 0, 0, 0);
        data[0] = b'X';
        assert!(matches!(
            SizeHeader::parse(&data),
            Err(SizeError::InvalidMagic(_))
        ));
    }

    #[test]
    fn test_reject_version_zero() {
        let data = make_header_bytes(0, 9, 0, 0, 0);
        assert!(matches!(
            SizeHeader::parse(&data),
            Err(SizeError::UnsupportedVersion(0))
        ));
    }

    #[test]
    fn test_reject_ekey_size_zero() {
        let data = make_header_bytes(1, 0, 0, 0, 0);
        assert!(matches!(
            SizeHeader::parse(&data),
            Err(SizeError::InvalidEKeySize(0))
        ));
    }

    #[test]
    fn test_reject_ekey_size_too_large() {
        let data = make_header_bytes(1, 17, 0, 0, 0);
        assert!(matches!(
            SizeHeader::parse(&data),
            Err(SizeError::InvalidEKeySize(17))
        ));
    }

    #[test]
    fn test_total_size_40bit_max() {
        let max40 = 0xFF_FFFF_FFFF_u64;
        let data = make_header_bytes(1, 9, 0, 0, max40);
        let hdr = SizeHeader::parse(&data).expect("Should parse");
        assert_eq!(hdr.total_size, max40);
    }
}

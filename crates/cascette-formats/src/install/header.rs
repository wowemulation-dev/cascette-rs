//! Install manifest header parsing and building

use crate::install::error::{InstallError, Result};
use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Read, Seek, Write};

/// Install manifest header
///
/// The base header (V1) is 10 bytes:
/// - Magic signature "IN" (2 bytes)
/// - Version number (1 byte, 1 or 2)
/// - Content key length (1 byte, typically 16 for MD5)
/// - Tag count (2 bytes, big-endian)
/// - Entry count (4 bytes, big-endian)
///
/// V2 adds 6 bytes after the base header:
/// - Content key size override (1 byte)
/// - Additional entry count (4 bytes, big-endian)
/// - Unknown byte (1 byte)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallHeader {
    /// Magic signature, always "IN"
    pub magic: [u8; 2],

    /// Version number (1 or 2)
    pub version: u8,

    /// Content key length in bytes, typically 16 for MD5
    pub ckey_length: u8,

    /// Number of tags in the manifest
    pub tag_count: u16,

    /// Number of file entries in the manifest
    pub entry_count: u32,

    /// V2: content key size override (None for V1)
    pub content_key_size: Option<u8>,

    /// V2: additional entry count (None for V1)
    pub entry_count_v2: Option<u32>,

    /// V2: unknown byte (None for V1)
    pub v2_unknown: Option<u8>,
}

impl InstallHeader {
    /// Create a new V1 install header with specified counts
    pub fn new(tag_count: u16, entry_count: u32) -> Self {
        Self {
            magic: *b"IN",
            version: 1,
            ckey_length: 16,
            tag_count,
            entry_count,
            content_key_size: None,
            entry_count_v2: None,
            v2_unknown: None,
        }
    }

    /// Create a new V2 install header
    pub fn new_v2(
        tag_count: u16,
        entry_count: u32,
        content_key_size: u8,
        entry_count_v2: u32,
    ) -> Self {
        Self {
            magic: *b"IN",
            version: 2,
            ckey_length: 16,
            tag_count,
            entry_count,
            content_key_size: Some(content_key_size),
            entry_count_v2: Some(entry_count_v2),
            v2_unknown: Some(0),
        }
    }

    /// Get the effective content key size for entries.
    ///
    /// V2 has an explicit field; V1 uses `ckey_length + 4` per Agent.exe.
    pub fn content_key_size(&self) -> u8 {
        self.content_key_size
            .unwrap_or_else(|| self.ckey_length.saturating_add(4))
    }

    /// Header size in bytes on disk
    pub fn header_size(&self) -> usize {
        if self.version >= 2 { 16 } else { 10 }
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

        if self.version == 0 || self.version > 2 {
            return Err(InstallError::UnsupportedVersion(self.version));
        }

        if self.ckey_length != 16 {
            return Err(InstallError::InvalidCKeyLength(self.ckey_length));
        }

        // V2 must have extended fields
        if self.version >= 2 && self.content_key_size.is_none() {
            return Err(InstallError::UnsupportedVersion(self.version));
        }

        Ok(())
    }
}

impl BinRead for InstallHeader {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read base header (10 bytes), always big-endian
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;

        if magic != *b"IN" {
            return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid install magic: expected 'IN', got {magic:?}"),
                )),
            });
        }

        let version = u8::read_options(reader, binrw::Endian::Big, ())?;
        let ckey_length = u8::read_options(reader, binrw::Endian::Big, ())?;
        let tag_count = u16::read_options(reader, binrw::Endian::Big, ())?;
        let entry_count = u32::read_options(reader, binrw::Endian::Big, ())?;

        // V2: read 6 additional bytes
        let (content_key_size, entry_count_v2, v2_unknown) = if version >= 2 {
            let cks = u8::read_options(reader, binrw::Endian::Big, ())?;
            let ec2 = u32::read_options(reader, binrw::Endian::Big, ())?;
            let unk = u8::read_options(reader, binrw::Endian::Big, ())?;
            (Some(cks), Some(ec2), Some(unk))
        } else {
            (None, None, None)
        };

        Ok(Self {
            magic,
            version,
            ckey_length,
            tag_count,
            entry_count,
            content_key_size,
            entry_count_v2,
            v2_unknown,
        })
    }
}

impl BinWrite for InstallHeader {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write base header (10 bytes), always big-endian
        writer.write_all(&self.magic)?;
        self.version.write_options(writer, binrw::Endian::Big, ())?;
        self.ckey_length
            .write_options(writer, binrw::Endian::Big, ())?;
        self.tag_count
            .write_options(writer, binrw::Endian::Big, ())?;
        self.entry_count
            .write_options(writer, binrw::Endian::Big, ())?;

        // V2: write 6 additional bytes
        if self.version >= 2 {
            let cks = self.content_key_size.unwrap_or(self.ckey_length);
            let ec2 = self.entry_count_v2.unwrap_or(0);
            let unk = self.v2_unknown.unwrap_or(0);
            cks.write_options(writer, binrw::Endian::Big, ())?;
            ec2.write_options(writer, binrw::Endian::Big, ())?;
            unk.write_options(writer, binrw::Endian::Big, ())?;
        }

        Ok(())
    }
}

impl binrw::meta::ReadEndian for InstallHeader {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Big);
}

impl binrw::meta::WriteEndian for InstallHeader {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Big);
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
        assert!(header.content_key_size.is_none());
    }

    #[test]
    fn test_header_new_v2() {
        let header = InstallHeader::new_v2(3, 10, 20, 50);
        assert_eq!(header.version, 2);
        assert_eq!(header.content_key_size, Some(20));
        assert_eq!(header.entry_count_v2, Some(50));
        assert_eq!(header.v2_unknown, Some(0));
        assert_eq!(header.content_key_size(), 20);
    }

    #[test]
    fn test_v1_content_key_size() {
        let header = InstallHeader::new(2, 5);
        // V1: content_key_size = ckey_length + 4 = 16 + 4 = 20
        assert_eq!(header.content_key_size(), 20);
    }

    #[test]
    fn test_header_size() {
        assert_eq!(InstallHeader::new(0, 0).header_size(), 10);
        assert_eq!(InstallHeader::new_v2(0, 0, 16, 0).header_size(), 16);
    }

    #[test]
    fn test_bit_mask_size() {
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

        // Test version 2 is valid (with extended fields)
        let v2_header = InstallHeader::new_v2(2, 5, 16, 0);
        assert!(v2_header.validate().is_ok());

        // Test version 0 is invalid
        let mut invalid_version = valid_header.clone();
        invalid_version.version = 0;
        assert!(matches!(
            invalid_version.validate(),
            Err(InstallError::UnsupportedVersion(0))
        ));

        // Test version 3 is invalid
        let mut invalid_version = valid_header.clone();
        invalid_version.version = 3;
        assert!(matches!(
            invalid_version.validate(),
            Err(InstallError::UnsupportedVersion(3))
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
    fn test_v1_header_parsing() {
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
        assert!(header.content_key_size.is_none());
        assert_eq!(header.bit_mask_size(), 1);
    }

    #[test]
    fn test_v2_header_parsing() {
        let data = [
            b'I', b'N', // Magic (2 bytes)
            2,    // Version (1 byte)
            16,   // CKey length (1 byte)
            0, 2, // Tag count (2 bytes, big-endian)
            0, 0, 0, 5,  // Entry count (4 bytes, big-endian)
            20, // Content key size (1 byte)
            0, 0, 0, 10, // Entry count V2 (4 bytes, big-endian)
            0,  // Unknown (1 byte)
        ];

        let header =
            InstallHeader::read(&mut Cursor::new(&data)).expect("Operation should succeed");
        assert_eq!(header.version, 2);
        assert_eq!(header.tag_count, 2);
        assert_eq!(header.entry_count, 5);
        assert_eq!(header.content_key_size, Some(20));
        assert_eq!(header.entry_count_v2, Some(10));
        assert_eq!(header.v2_unknown, Some(0));
    }

    #[test]
    fn test_v1_header_round_trip() {
        let original = InstallHeader::new(10, 100);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write(&mut cursor)
            .expect("Operation should succeed");
        assert_eq!(buffer.len(), 10);

        let parsed =
            InstallHeader::read(&mut Cursor::new(&buffer)).expect("Operation should succeed");
        assert_eq!(original, parsed);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_v2_header_round_trip() {
        let original = InstallHeader::new_v2(10, 100, 20, 50);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write(&mut cursor)
            .expect("Operation should succeed");
        assert_eq!(buffer.len(), 16);

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
            tag_count: 0x1234,
            entry_count: 0x1234_5678,
            content_key_size: None,
            entry_count_v2: None,
            v2_unknown: None,
        };

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header.write(&mut cursor).expect("Operation should succeed");

        assert_eq!(buffer[0], b'I');
        assert_eq!(buffer[1], b'N');
        assert_eq!(buffer[2], 1);
        assert_eq!(buffer[3], 16);
        assert_eq!(buffer[4], 0x12);
        assert_eq!(buffer[5], 0x34);
        assert_eq!(buffer[6], 0x12);
        assert_eq!(buffer[7], 0x34);
        assert_eq!(buffer[8], 0x56);
        assert_eq!(buffer[9], 0x78);

        let parsed =
            InstallHeader::read(&mut Cursor::new(&buffer)).expect("Operation should succeed");
        assert_eq!(header, parsed);
    }

    #[test]
    fn test_header_assertion_error() {
        let data = [
            b'X', b'X', // Invalid magic
            1,    // Version
            16,   // CKey length
            0, 2, // Tag count
            0, 0, 0, 5, // Entry count
        ];

        let result = InstallHeader::read(&mut Cursor::new(&data));
        assert!(result.is_err());
    }
}

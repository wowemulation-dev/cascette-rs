//! Root file header structures for different versions

use crate::root::{error::Result, version::RootVersion};
use std::io::{Read, Seek, Write};

/// Root file header (V2, V3, V4 only - V1 has no header)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootHeader {
    /// Version 2 header with `MFST` or `TSFM` magic
    V2 {
        /// Header magic (determines field endianness)
        magic: RootMagic,
        /// File counts and structure info
        info: RootHeaderInfo,
    },
    /// Version 3/4 header with extended structure
    V3V4 {
        /// Header magic (determines field endianness)
        magic: RootMagic,
        /// Header size in bytes
        header_size: u32,
        /// Version number (3 or 4)
        version: u32,
        /// File counts and structure info
        info: RootHeaderInfo,
        /// Padding to reach `header_size`
        padding: u32,
    },
}

/// Common header information across versions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootHeaderInfo {
    /// Total number of files across all blocks
    pub total_files: u32,
    /// Number of files with name hashes
    pub named_files: u32,
}

impl RootHeaderInfo {
    /// Read header info with specified endianness
    pub fn read<R: Read>(reader: &mut R, is_little_endian: bool) -> Result<Self> {
        let mut buf = [0u8; 4];

        reader.read_exact(&mut buf)?;
        let total_files = if is_little_endian {
            u32::from_le_bytes(buf)
        } else {
            u32::from_be_bytes(buf)
        };

        reader.read_exact(&mut buf)?;
        let named_files = if is_little_endian {
            u32::from_le_bytes(buf)
        } else {
            u32::from_be_bytes(buf)
        };

        Ok(Self {
            total_files,
            named_files,
        })
    }

    /// Write header info with big-endian (MFST format)
    pub fn write_be<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.total_files.to_be_bytes())?;
        writer.write_all(&self.named_files.to_be_bytes())?;
        Ok(())
    }

    /// Write header info with little-endian (TSFM format)
    pub fn write_le<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.total_files.to_le_bytes())?;
        writer.write_all(&self.named_files.to_le_bytes())?;
        Ok(())
    }
}

/// Magic signatures for root file headers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootMagic {
    /// Standard `MFST` magic (big-endian header fields)
    Mfst,
    /// Alternative `TSFM` magic (little-endian header fields)
    Tsfm,
}

impl RootMagic {
    /// Magic bytes for `MFST`
    pub const MFST_BYTES: [u8; 4] = *b"MFST";
    /// Magic bytes for `TSFM`
    pub const TSFM_BYTES: [u8; 4] = *b"TSFM";

    /// Convert to bytes
    pub const fn to_bytes(self) -> [u8; 4] {
        match self {
            Self::Mfst => Self::MFST_BYTES,
            Self::Tsfm => Self::TSFM_BYTES,
        }
    }

    /// Parse from bytes
    pub const fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        match bytes {
            Self::MFST_BYTES => Some(Self::Mfst),
            Self::TSFM_BYTES => Some(Self::Tsfm),
            _ => None,
        }
    }

    /// Whether this magic indicates little-endian header fields
    pub const fn is_little_endian(self) -> bool {
        matches!(self, Self::Tsfm)
    }
}

impl RootHeader {
    /// Create V2 header (defaults to MFST/big-endian)
    pub fn new_v2(total_files: u32, named_files: u32) -> Self {
        Self::V2 {
            magic: RootMagic::Mfst,
            info: RootHeaderInfo {
                total_files,
                named_files,
            },
        }
    }

    /// Create V3/V4 header (defaults to MFST/big-endian)
    pub fn new_v3v4(version: u32, total_files: u32, named_files: u32) -> Self {
        Self::V3V4 {
            magic: RootMagic::Mfst,
            header_size: 20, // Standard size
            version,
            info: RootHeaderInfo {
                total_files,
                named_files,
            },
            padding: 0,
        }
    }

    /// Get the header magic
    pub const fn magic(&self) -> RootMagic {
        match self {
            Self::V2 { magic, .. } | Self::V3V4 { magic, .. } => *magic,
        }
    }

    /// Get total files count
    pub const fn total_files(&self) -> u32 {
        match self {
            Self::V2 { info, .. } | Self::V3V4 { info, .. } => info.total_files,
        }
    }

    /// Get named files count
    pub const fn named_files(&self) -> u32 {
        match self {
            Self::V2 { info, .. } | Self::V3V4 { info, .. } => info.named_files,
        }
    }

    /// Get version number
    pub const fn version(&self) -> RootVersion {
        match self {
            Self::V2 { .. } | Self::V3V4 { version: 2, .. } => RootVersion::V2,
            Self::V3V4 { version: 3, .. } => RootVersion::V3,
            Self::V3V4 { version, .. } => {
                // Default to V4 for versions >= 4
                if *version >= 4 {
                    RootVersion::V4
                } else {
                    RootVersion::V3
                }
            }
        }
    }

    /// Calculate header size in bytes
    pub const fn size(&self) -> usize {
        match self {
            Self::V2 { .. } => 12, // magic(4) + info(8)
            Self::V3V4 { header_size, .. } => *header_size as usize,
        }
    }

    /// Read header from reader (auto-detects classic vs extended header format)
    pub fn read<R: Read + Seek>(reader: &mut R, version: RootVersion) -> Result<Self> {
        if matches!(version, RootVersion::V1) {
            unreachable!("V1 has no header");
        }

        // Read magic to determine endianness
        let mut magic_bytes = [0u8; 4];
        reader.read_exact(&mut magic_bytes)?;

        // TSFM magic means little-endian, MFST means big-endian
        let magic = if magic_bytes == RootMagic::TSFM_BYTES {
            RootMagic::Tsfm
        } else {
            RootMagic::Mfst
        };
        let is_little_endian = magic.is_little_endian();

        // Read first two u32 values to detect header format
        let mut buf = [0u8; 4];

        reader.read_exact(&mut buf)?;
        let value1 = if is_little_endian {
            u32::from_le_bytes(buf)
        } else {
            u32::from_be_bytes(buf)
        };

        reader.read_exact(&mut buf)?;
        let value2 = if is_little_endian {
            u32::from_le_bytes(buf)
        } else {
            u32::from_be_bytes(buf)
        };

        // Detect extended vs classic header:
        // Extended: value1=header_size (16-64), value2=version (1-10), value2 < value1
        // Classic V2: value1=total_files (large), value2=named_files
        let is_extended_header = (16..100).contains(&value1) && value2 < 10 && value2 < value1;

        if is_extended_header {
            // Extended header: value1=header_size, value2=version
            let header_size = value1;
            let version_field = value2;

            // Read remaining: total_files, named_files, padding
            let info = RootHeaderInfo::read(reader, is_little_endian)?;

            reader.read_exact(&mut buf)?;
            let padding = if is_little_endian {
                u32::from_le_bytes(buf)
            } else {
                u32::from_be_bytes(buf)
            };

            Ok(Self::V3V4 {
                magic,
                header_size,
                version: version_field,
                info,
                padding,
            })
        } else {
            // Classic V2 header: value1=total_files, value2=named_files
            Ok(Self::V2 {
                magic,
                info: RootHeaderInfo {
                    total_files: value1,
                    named_files: value2,
                },
            })
        }
    }

    /// Write header to writer, preserving the original magic and endianness
    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> Result<()> {
        match self {
            Self::V2 { magic, info } => {
                writer.write_all(&magic.to_bytes())?;
                if magic.is_little_endian() {
                    info.write_le(writer)?;
                } else {
                    info.write_be(writer)?;
                }
            }
            Self::V3V4 {
                magic,
                header_size,
                version,
                info,
                padding,
            } => {
                writer.write_all(&magic.to_bytes())?;
                if magic.is_little_endian() {
                    writer.write_all(&header_size.to_le_bytes())?;
                    writer.write_all(&version.to_le_bytes())?;
                    info.write_le(writer)?;
                    writer.write_all(&padding.to_le_bytes())?;
                } else {
                    writer.write_all(&header_size.to_be_bytes())?;
                    writer.write_all(&version.to_be_bytes())?;
                    info.write_be(writer)?;
                    writer.write_all(&padding.to_be_bytes())?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_root_magic() {
        assert_eq!(RootMagic::Mfst.to_bytes(), *b"MFST");
        assert_eq!(RootMagic::Tsfm.to_bytes(), *b"TSFM");

        assert_eq!(RootMagic::from_bytes(*b"MFST"), Some(RootMagic::Mfst));
        assert_eq!(RootMagic::from_bytes(*b"TSFM"), Some(RootMagic::Tsfm));
        assert_eq!(RootMagic::from_bytes(*b"XXXX"), None);

        assert!(!RootMagic::Mfst.is_little_endian());
        assert!(RootMagic::Tsfm.is_little_endian());
    }

    #[test]
    fn test_header_info_round_trip_be() {
        let original = RootHeaderInfo {
            total_files: 123_456,
            named_files: 78_901,
        };

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_be(&mut cursor)
            .expect("Test operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeaderInfo::read(&mut cursor, false).expect("Test operation should succeed");

        assert_eq!(original, restored);
    }

    #[test]
    fn test_header_info_round_trip_le() {
        let original = RootHeaderInfo {
            total_files: 123_456,
            named_files: 78_901,
        };

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_le(&mut cursor)
            .expect("Test operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeaderInfo::read(&mut cursor, true).expect("Test operation should succeed");

        assert_eq!(original, restored);
    }

    #[test]
    fn test_v2_header_round_trip() {
        let header = RootHeader::new_v2(123_456, 78_901);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write(&mut cursor)
            .expect("Test operation should succeed");

        // Should be: magic(4) + total_files(4) + named_files(4) = 12 bytes
        assert_eq!(buffer.len(), 12);
        assert_eq!(&buffer[0..4], b"MFST");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeader::read(&mut cursor, RootVersion::V2).expect("Test operation should succeed");

        assert_eq!(header, restored);
        assert_eq!(restored.total_files(), 123_456);
        assert_eq!(restored.named_files(), 78_901);
        assert_eq!(restored.version(), RootVersion::V2);
        assert_eq!(restored.magic(), RootMagic::Mfst);
    }

    #[test]
    fn test_v2_tsfm_round_trip() {
        let header = RootHeader::V2 {
            magic: RootMagic::Tsfm,
            info: RootHeaderInfo {
                total_files: 123_456,
                named_files: 78_901,
            },
        };

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write(&mut cursor)
            .expect("Test operation should succeed");

        assert_eq!(&buffer[0..4], b"TSFM");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeader::read(&mut cursor, RootVersion::V2).expect("Test operation should succeed");

        assert_eq!(header, restored);
        assert_eq!(restored.magic(), RootMagic::Tsfm);
        assert_eq!(restored.total_files(), 123_456);
        assert_eq!(restored.named_files(), 78_901);
    }

    #[test]
    fn test_v3_header_round_trip() {
        let header = RootHeader::new_v3v4(3, 123_456, 78_901);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write(&mut cursor)
            .expect("Test operation should succeed");

        // Should be: magic(4) + header_size(4) + version(4) + info(8) + padding(4) = 24 bytes
        assert_eq!(buffer.len(), 24);
        assert_eq!(&buffer[0..4], b"MFST");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeader::read(&mut cursor, RootVersion::V3).expect("Test operation should succeed");

        assert_eq!(header, restored);
        assert_eq!(restored.total_files(), 123_456);
        assert_eq!(restored.named_files(), 78_901);
        assert_eq!(restored.version(), RootVersion::V3);
    }

    #[test]
    fn test_v3v4_tsfm_round_trip() {
        let header = RootHeader::V3V4 {
            magic: RootMagic::Tsfm,
            header_size: 20,
            version: 3,
            info: RootHeaderInfo {
                total_files: 123_456,
                named_files: 78_901,
            },
            padding: 0,
        };

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write(&mut cursor)
            .expect("Test operation should succeed");

        assert_eq!(&buffer[0..4], b"TSFM");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeader::read(&mut cursor, RootVersion::V3).expect("Test operation should succeed");

        assert_eq!(header, restored);
        assert_eq!(restored.magic(), RootMagic::Tsfm);
        assert_eq!(restored.total_files(), 123_456);
        assert_eq!(restored.named_files(), 78_901);
    }

    #[test]
    fn test_v4_header_round_trip() {
        let header = RootHeader::new_v3v4(4, 123_456, 78_901);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        header
            .write(&mut cursor)
            .expect("Test operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored =
            RootHeader::read(&mut cursor, RootVersion::V4).expect("Test operation should succeed");

        assert_eq!(header, restored);
        assert_eq!(restored.version(), RootVersion::V4);
    }

    #[test]
    fn test_header_size() {
        let v2_header = RootHeader::new_v2(1000, 500);
        assert_eq!(v2_header.size(), 12);

        let v3_header = RootHeader::new_v3v4(3, 1000, 500);
        assert_eq!(v3_header.size(), 20); // header_size field value
    }
}

//! Install manifest tag system with bit mask operations

use crate::install::error::InstallError;
use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Read, Seek, Write};

/// Tag types used to categorize files in install manifests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum TagType {
    /// Platform tag (Windows, Mac, etc.)
    Platform = 0x0001,
    /// Architecture tag (x86, `x86_64`, etc.)
    Architecture = 0x0002,
    /// Locale tag (enUS, deDE, etc.)
    Locale = 0x0003,
    /// Category tag (base, optional, etc.)
    Category = 0x0004,
    /// Unknown tag type (commonly seen in manifests)
    Unknown = 0x0005,
    /// Component tag (game, launcher, etc.)
    Component = 0x0010,
    /// Version tag (live, ptr, beta, etc.)
    Version = 0x0020,
    /// Optimization tag (retail, debug, etc.)
    Optimization = 0x0040,
    /// Region tag (US, EU, KR, etc.)
    Region = 0x0080,
    /// Device tag (desktop, mobile, etc.)
    Device = 0x0100,
    /// Mode tag (online, offline, etc.)
    Mode = 0x0200,
    /// Branch tag (main, experimental, etc.)
    Branch = 0x0400,
    /// Content tag (cinematics, audio, etc.)
    Content = 0x0800,
    /// Feature tag (graphics, physics, etc.)
    Feature = 0x1000,
    /// Expansion tag (base, expansion1, etc.)
    Expansion = 0x2000,
    /// Alternate tag (alternate content version)
    Alternate = 0x4000,
    /// Option tag (optional features)
    Option = 0x8000,
}

impl TagType {
    /// Convert from raw u16 value
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::Platform),
            0x0002 => Some(Self::Architecture),
            0x0003 => Some(Self::Locale),
            0x0004 => Some(Self::Category),
            0x0005 => Some(Self::Unknown),
            0x0010 => Some(Self::Component),
            0x0020 => Some(Self::Version),
            0x0040 => Some(Self::Optimization),
            0x0080 => Some(Self::Region),
            0x0100 => Some(Self::Device),
            0x0200 => Some(Self::Mode),
            0x0400 => Some(Self::Branch),
            0x0800 => Some(Self::Content),
            0x1000 => Some(Self::Feature),
            0x2000 => Some(Self::Expansion),
            0x4000 => Some(Self::Alternate),
            0x8000 => Some(Self::Option),
            _ => None,
        }
    }
}

/// Install tag with name, type, and file associations via bit mask
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTag {
    /// Tag name (e.g., "Windows", "`x86_64`", "enUS")
    pub name: String,
    /// Tag type categorizing the tag's purpose
    pub tag_type: TagType,
    /// Bit mask indicating which files are associated with this tag
    /// Each bit corresponds to a file index (bit 0 = file 0, etc.)
    /// Uses little-endian bit ordering within bytes
    pub bit_mask: Vec<u8>,
}

impl InstallTag {
    /// Create a new install tag
    pub fn new(name: String, tag_type: TagType, entry_count: usize) -> Self {
        let bit_mask_size = entry_count.div_ceil(8);
        Self {
            name,
            tag_type,
            bit_mask: vec![0u8; bit_mask_size],
        }
    }

    /// Check if a file at the given index is associated with this tag
    pub fn has_file(&self, file_index: usize) -> bool {
        let byte_index = file_index / 8;
        let bit_offset = file_index % 8;

        if byte_index >= self.bit_mask.len() {
            return false;
        }

        // Little-endian bit ordering within bytes: bit 0 = LSB
        (self.bit_mask[byte_index] & (1 << bit_offset)) != 0
    }

    /// Associate a file at the given index with this tag
    pub fn add_file(&mut self, file_index: usize) {
        let byte_index = file_index / 8;
        let bit_offset = file_index % 8;

        // Extend bit mask if needed
        if byte_index >= self.bit_mask.len() {
            self.bit_mask.resize(byte_index + 1, 0);
        }

        // Set bit using little-endian ordering
        self.bit_mask[byte_index] |= 1 << bit_offset;
    }

    /// Remove file association at the given index
    pub fn remove_file(&mut self, file_index: usize) {
        let byte_index = file_index / 8;
        let bit_offset = file_index % 8;

        if byte_index >= self.bit_mask.len() {
            return;
        }

        // Clear bit using little-endian ordering
        self.bit_mask[byte_index] &= !(1 << bit_offset);
    }

    /// Count the total number of files associated with this tag
    pub fn file_count(&self) -> usize {
        self.bit_mask
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum()
    }

    /// Get all file indices associated with this tag
    pub fn get_files(&self, max_entry_count: usize) -> Vec<usize> {
        let mut files = Vec::new();

        for file_index in 0..max_entry_count {
            if self.has_file(file_index) {
                files.push(file_index);
            }
        }

        files
    }

    /// Compute intersection of bit masks with another tag
    pub fn intersect(&self, other: &Self) -> Vec<u8> {
        let min_len = self.bit_mask.len().min(other.bit_mask.len());
        let mut result = vec![0u8; min_len];

        for (i, result_byte) in result.iter_mut().enumerate().take(min_len) {
            *result_byte = self.bit_mask[i] & other.bit_mask[i];
        }

        result
    }

    /// Compute union of bit masks with another tag
    pub fn union(&self, other: &Self) -> Vec<u8> {
        let max_len = self.bit_mask.len().max(other.bit_mask.len());
        let mut result = vec![0u8; max_len];

        for (i, result_byte) in result.iter_mut().enumerate().take(max_len) {
            let a = self.bit_mask.get(i).copied().unwrap_or(0);
            let b = other.bit_mask.get(i).copied().unwrap_or(0);
            *result_byte = a | b;
        }

        result
    }

    /// Check if this tag is a platform tag
    pub fn is_platform_tag(&self) -> bool {
        matches!(self.tag_type, TagType::Platform)
    }

    /// Check if this tag is required for a specific platform
    pub fn is_required_for_platform(&self, platform: &str) -> bool {
        self.tag_type == TagType::Platform && self.name == platform
    }
}

impl BinRead for InstallTag {
    type Args<'a> = u32; // entry_count for bit mask size

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        entry_count: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read null-terminated tag name
        let mut name_bytes = Vec::new();
        loop {
            let byte = u8::read_options(reader, endian, ())?;
            if byte == 0 {
                break;
            }
            name_bytes.push(byte);
        }
        let name = String::from_utf8(name_bytes).map_err(|e| binrw::Error::Custom {
            pos: reader.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        // Read tag type (big-endian)
        let tag_type_value = u16::read_options(reader, binrw::Endian::Big, ())?;
        let tag_type = TagType::from_u16(tag_type_value).ok_or_else(|| binrw::Error::Custom {
            pos: reader.stream_position().unwrap_or(0),
            err: Box::new(InstallError::InvalidTagType(tag_type_value)),
        })?;

        // Read bit mask
        let bit_mask_size = (entry_count as usize).div_ceil(8);
        let mut bit_mask = vec![0u8; bit_mask_size];
        reader.read_exact(&mut bit_mask)?;

        Ok(Self {
            name,
            tag_type,
            bit_mask,
        })
    }
}

impl BinWrite for InstallTag {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write null-terminated tag name
        writer.write_all(self.name.as_bytes())?;
        writer.write_all(&[0])?;

        // Write tag type (big-endian)
        (self.tag_type as u16).write_options(writer, binrw::Endian::Big, ())?;

        // Write bit mask
        writer.write_all(&self.bit_mask)?;

        Ok(())
    }
}

// Manual WriteEndian implementation required by binrw
impl binrw::meta::WriteEndian for InstallTag {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Big);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    #[test]
    fn test_tag_type_conversion() {
        let tag_types = [
            (TagType::Platform, 0x0001),
            (TagType::Architecture, 0x0002),
            (TagType::Locale, 0x0003),
            (TagType::Category, 0x0004),
            (TagType::Unknown, 0x0005),
            (TagType::Component, 0x0010),
            (TagType::Version, 0x0020),
            (TagType::Optimization, 0x0040),
            (TagType::Region, 0x0080),
            (TagType::Device, 0x0100),
            (TagType::Mode, 0x0200),
            (TagType::Branch, 0x0400),
            (TagType::Content, 0x0800),
            (TagType::Feature, 0x1000),
            (TagType::Expansion, 0x2000),
            (TagType::Alternate, 0x4000),
            (TagType::Option, 0x8000),
        ];

        for (tag_type, expected_value) in tag_types {
            assert_eq!(tag_type as u16, expected_value);
            assert_eq!(TagType::from_u16(expected_value), Some(tag_type));
        }

        // Test invalid tag type
        assert_eq!(TagType::from_u16(0x9999), None);
    }

    #[test]
    fn test_bit_mask_operations() {
        let mut tag = InstallTag::new("Windows".to_string(), TagType::Platform, 16);

        // Initially no files should be associated
        assert_eq!(tag.file_count(), 0);
        assert!(!tag.has_file(0));

        // Add files 0, 1, and 9
        tag.add_file(0);
        tag.add_file(1);
        tag.add_file(9);

        // Check bit patterns (little-endian bit ordering)
        assert_eq!(tag.bit_mask[0], 0b0000_0011); // Files 0 and 1
        assert_eq!(tag.bit_mask[1], 0b0000_0010); // File 9 (bit 1 of byte 1)

        // Verify file associations
        assert!(tag.has_file(0));
        assert!(tag.has_file(1));
        assert!(!tag.has_file(2));
        assert!(tag.has_file(9));
        assert!(!tag.has_file(10));

        assert_eq!(tag.file_count(), 3);

        // Test get_files
        let files = tag.get_files(16);
        assert_eq!(files, vec![0, 1, 9]);

        // Test removing file
        tag.remove_file(1);
        assert!(!tag.has_file(1));
        assert_eq!(tag.file_count(), 2);
        assert_eq!(tag.bit_mask[0], 0b0000_0001); // Only file 0
    }

    #[test]
    fn test_bit_mask_auto_resize() {
        let mut tag = InstallTag::new("Test".to_string(), TagType::Category, 8);
        assert_eq!(tag.bit_mask.len(), 1); // 8 bits = 1 byte

        // Add file beyond initial capacity
        tag.add_file(15);
        assert_eq!(tag.bit_mask.len(), 2); // Should auto-resize to 2 bytes
        assert!(tag.has_file(15));

        // Add file even further out
        tag.add_file(31);
        assert_eq!(tag.bit_mask.len(), 4); // Should auto-resize to 4 bytes
        assert!(tag.has_file(31));
    }

    #[test]
    fn test_tag_intersection() {
        let mut tag1 = InstallTag::new("Windows".to_string(), TagType::Platform, 16);
        let mut tag2 = InstallTag::new("x86_64".to_string(), TagType::Architecture, 16);

        tag1.add_file(0);
        tag1.add_file(1);
        tag1.add_file(2);

        tag2.add_file(1);
        tag2.add_file(2);
        tag2.add_file(3);

        let intersection = tag1.intersect(&tag2);
        // Files 1 and 2 are in both tags
        assert_eq!(intersection[0], 0b0000_0110); // Bits 1 and 2 set

        let union = tag1.union(&tag2);
        // Files 0, 1, 2, 3 are in either tag
        assert_eq!(union[0], 0b0000_1111); // Bits 0, 1, 2, 3 set
    }

    #[test]
    fn test_tag_parsing() {
        // "Windows\0" + type (0x0001) + bit mask (1 byte)
        let data = [
            b'W',
            b'i',
            b'n',
            b'd',
            b'o',
            b'w',
            b's',
            0, // Tag name
            0,
            1,           // Tag type (big-endian)
            0b1010_1010, // Bit mask
        ];

        let tag = InstallTag::read_options(
            &mut Cursor::new(&data),
            binrw::Endian::Big,
            8, // entry_count
        )
        .expect("Operation should succeed");

        assert_eq!(tag.name, "Windows");
        assert_eq!(tag.tag_type, TagType::Platform);
        assert_eq!(tag.bit_mask, vec![0b1010_1010]);

        // Check file associations (little-endian bit ordering)
        assert!(!tag.has_file(0)); // Bit 0 not set
        assert!(tag.has_file(1)); // Bit 1 set
        assert!(!tag.has_file(2)); // Bit 2 not set
        assert!(tag.has_file(3)); // Bit 3 set
        assert!(!tag.has_file(4)); // Bit 4 not set
        assert!(tag.has_file(5)); // Bit 5 set
        assert!(!tag.has_file(6)); // Bit 6 not set
        assert!(tag.has_file(7)); // Bit 7 set

        assert_eq!(tag.file_count(), 4);
    }

    #[test]
    fn test_tag_round_trip() {
        let mut tag = InstallTag::new("x86_64".to_string(), TagType::Architecture, 16);
        tag.add_file(0);
        tag.add_file(5);
        tag.add_file(10);

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        tag.write(&mut cursor).expect("Operation should succeed");

        // Deserialize
        let parsed = InstallTag::read_options(
            &mut Cursor::new(&buffer),
            binrw::Endian::Big,
            16, // entry_count
        )
        .expect("Operation should succeed");

        assert_eq!(tag, parsed);
    }

    #[test]
    fn test_tag_utility_methods() {
        let platform_tag = InstallTag::new("Windows".to_string(), TagType::Platform, 8);
        let category_tag = InstallTag::new("Base".to_string(), TagType::Category, 8);

        assert!(platform_tag.is_platform_tag());
        assert!(!category_tag.is_platform_tag());

        assert!(platform_tag.is_required_for_platform("Windows"));
        assert!(!platform_tag.is_required_for_platform("Mac"));
        assert!(!category_tag.is_required_for_platform("Windows"));
    }

    #[test]
    fn test_invalid_tag_type() {
        let data = [
            b't',
            b'e',
            b's',
            b't',
            0, // Tag name "test\0"
            0x99,
            0x99,        // Invalid tag type
            0b0000_0001, // Bit mask
        ];

        let result = InstallTag::read_options(
            &mut Cursor::new(&data),
            binrw::Endian::Big,
            8, // entry_count
        );

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            binrw::Error::Custom { .. }
        ));
    }

    #[test]
    fn test_empty_tag_name() {
        let mut tag = InstallTag::new(String::new(), TagType::Category, 8);
        tag.add_file(0);

        // Should handle empty names correctly
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        tag.write(&mut cursor).expect("Operation should succeed");

        let parsed = InstallTag::read_options(&mut Cursor::new(&buffer), binrw::Endian::Big, 8)
            .expect("Operation should succeed");

        assert_eq!(parsed.name, "");
        assert_eq!(parsed.tag_type, TagType::Category);
        assert!(parsed.has_file(0));
    }

    #[test]
    fn test_large_bit_mask() {
        let mut tag = InstallTag::new("Large".to_string(), TagType::Category, 1000);

        // Test various file indices across multiple bytes
        tag.add_file(0); // First bit of first byte
        tag.add_file(7); // Last bit of first byte
        tag.add_file(8); // First bit of second byte
        tag.add_file(999); // Last possible file

        assert!(tag.has_file(0));
        assert!(tag.has_file(7));
        assert!(tag.has_file(8));
        assert!(tag.has_file(999));
        assert!(!tag.has_file(500)); // Random unset bit

        assert_eq!(tag.file_count(), 4);
        assert_eq!(tag.bit_mask.len(), 125); // (1000 + 7) / 8 = 125
    }
}

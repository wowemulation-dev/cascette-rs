//! Download manifest file entries with 40-bit file sizes and `EncodingKeys`

use crate::download::error::{DownloadError, Result};
use crate::download::header::DownloadHeader;
use crate::download::priority::PriorityCategory;
use binrw::{BinRead, BinResult, BinWrite};
use cascette_crypto::EncodingKey;
use std::io::{Read, Seek, Write};

/// 40-bit file size for download manifest entries
///
/// Download manifests support files larger than 4GB by using 5-byte
/// big-endian integer fields for file sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileSize40(u64);

impl FileSize40 {
    /// Maximum value for 40-bit integer (2^40 - 1 = 1,099,511,627,775 bytes ≈ 1TB)
    pub const MAX: u64 = 0xFF_FFFF_FFFF;

    /// Create a new 40-bit file size
    ///
    /// Returns an error if the size exceeds the 40-bit maximum.
    pub fn new(size: u64) -> Result<Self> {
        if size > Self::MAX {
            return Err(DownloadError::FileSizeTooLarge(size));
        }
        Ok(Self(size))
    }

    /// Get the size as u64
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Create from 5-byte big-endian array
    pub fn from_bytes(bytes: &[u8; 5]) -> Self {
        let size = (u64::from(bytes[0]) << 32)
            | (u64::from(bytes[1]) << 24)
            | (u64::from(bytes[2]) << 16)
            | (u64::from(bytes[3]) << 8)
            | u64::from(bytes[4]);
        Self(size)
    }

    /// Convert to 5-byte big-endian array
    pub fn to_bytes(self) -> [u8; 5] {
        [
            (self.0 >> 32) as u8,
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }

    /// Check if this is a large file (>4GB)
    pub fn is_large_file(self) -> bool {
        self.0 > u64::from(u32::MAX)
    }

    /// Convert to human-readable string with units
    pub fn to_human_readable(self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.0 as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if size.fract() == 0.0 {
            format!("{:.0} {}", size, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

impl std::fmt::Display for FileSize40 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for FileSize40 {
    fn from(size: u32) -> Self {
        Self(u64::from(size))
    }
}

impl TryFrom<u64> for FileSize40 {
    type Error = DownloadError;

    fn try_from(size: u64) -> Result<Self> {
        Self::new(size)
    }
}

impl BinRead for FileSize40 {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut bytes = [0u8; 5];
        reader.read_exact(&mut bytes)?;
        Ok(Self::from_bytes(&bytes))
    }
}

impl BinWrite for FileSize40 {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        writer.write_all(&self.to_bytes())?;
        Ok(())
    }
}

/// Download manifest file entry
///
/// Represents a file in the download manifest with encoding key, size,
/// priority, and optional checksum and flags depending on version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFileEntry {
    /// Encoding key identifying the encoded version of the file
    pub encoding_key: EncodingKey,
    /// File size using 40-bit representation
    pub file_size: FileSize40,
    /// Priority value (signed, lower values = higher priority)
    pub priority: i8,
    /// Optional checksum (present if `header.has_checksum` is true)
    pub checksum: Option<u32>,
    /// Optional flags (present if `header.flag_size` > 0 in V2+)
    pub flags: Option<Vec<u8>>,
}

impl DownloadFileEntry {
    /// Create a new download file entry
    pub fn new(encoding_key: EncodingKey, file_size: u64, priority: i8) -> Result<Self> {
        Ok(Self {
            encoding_key,
            file_size: FileSize40::new(file_size)?,
            priority,
            checksum: None,
            flags: None,
        })
    }

    /// Set the checksum for this entry
    #[must_use]
    pub fn with_checksum(mut self, checksum: u32) -> Self {
        self.checksum = Some(checksum);
        self
    }

    /// Set the flags for this entry
    #[must_use]
    pub fn with_flags(mut self, flags: Vec<u8>) -> Self {
        self.flags = Some(flags);
        self
    }

    /// Calculate effective priority considering base priority adjustment
    ///
    /// In version 3+, all priorities are adjusted by subtracting the
    /// `base_priority` from the header.
    pub fn effective_priority(&self, header: &DownloadHeader) -> i8 {
        match header {
            DownloadHeader::V3(_) => {
                // Use saturating arithmetic to prevent overflow
                self.priority.saturating_sub(header.base_priority())
            }
            _ => self.priority,
        }
    }

    /// Get the priority category for download planning
    pub fn priority_category(&self, header: &DownloadHeader) -> PriorityCategory {
        let effective = self.effective_priority(header);
        PriorityCategory::from_priority(effective)
    }

    /// Check if this file is considered high priority
    pub fn is_high_priority(&self, header: &DownloadHeader) -> bool {
        let effective = self.effective_priority(header);
        effective <= 1
    }

    /// Check if this file is essential for basic gameplay
    pub fn is_essential(&self, header: &DownloadHeader) -> bool {
        let effective = self.effective_priority(header);
        effective <= 0
    }

    /// Check if this file is critical and must be downloaded first
    pub fn is_critical(&self, header: &DownloadHeader) -> bool {
        let effective = self.effective_priority(header);
        effective < 0
    }

    /// Get the download priority rank (0 = highest priority)
    pub fn download_rank(&self, header: &DownloadHeader) -> u8 {
        let effective = self.effective_priority(header);
        // Convert signed priority to unsigned rank
        // Critical: -128 to -1 -> 0 to 127
        // Essential: 0 -> 128
        // High: 1-2 -> 129-130
        // Normal: 3-5 -> 131-133
        // Low: 6+ -> 134+
        if effective < 0 {
            (128i16 + i16::from(effective)) as u8 // Maps -128..-1 to 0..127
        } else {
            128u8.saturating_add(effective as u8) // Maps 0..127 to 128..255
        }
    }

    /// Calculate entry size in bytes when serialized
    pub fn serialized_size(&self, header: &DownloadHeader) -> usize {
        let mut size = 16 + 5 + 1; // encoding_key + file_size + priority

        if header.has_checksum() {
            size += 4; // checksum
        }

        size += header.flag_size() as usize; // flags

        size
    }

    /// Validate entry consistency with header
    pub fn validate(&self, header: &DownloadHeader) -> Result<()> {
        // Check checksum consistency
        if header.has_checksum() && self.checksum.is_none() {
            return Err(DownloadError::MissingChecksum);
        }
        if !header.has_checksum() && self.checksum.is_some() {
            return Err(DownloadError::ChecksumsNotEnabled);
        }

        // Check flags consistency
        let expected_flag_size = header.flag_size();
        match (&self.flags, expected_flag_size) {
            (None, 0) => {} // No flags expected, none provided - OK
            (Some(flags), size) if flags.len() == size as usize => {} // Correct size - OK
            (Some(_), 0) => {
                return Err(DownloadError::FlagsNotEnabled);
            }
            (Some(flags), expected_size) => {
                return Err(DownloadError::InvalidFlagSize(flags.len(), expected_size));
            }
            (None, _) => {
                return Err(DownloadError::MissingFlags);
            }
        }

        Ok(())
    }
}

impl BinRead for DownloadFileEntry {
    type Args<'a> = &'a DownloadHeader;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        header: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read encoding key (always 16 bytes)
        let mut key_bytes = [0u8; 16];
        reader.read_exact(&mut key_bytes)?;
        let encoding_key = EncodingKey::from_bytes(key_bytes);

        // Read 40-bit file size
        let file_size = FileSize40::read_options(reader, endian, ())?;

        // Read priority (signed byte)
        let priority = i8::read_options(reader, endian, ())?;

        // Read optional checksum (big-endian)
        let checksum = if header.has_checksum() {
            Some(u32::read_options(reader, binrw::Endian::Big, ())?)
        } else {
            None
        };

        // Read optional flags (version 2+)
        let flags = if header.flag_size() > 0 {
            let mut flag_bytes = vec![0u8; header.flag_size() as usize];
            reader.read_exact(&mut flag_bytes)?;
            Some(flag_bytes)
        } else {
            None
        };

        let entry = DownloadFileEntry {
            encoding_key,
            file_size,
            priority,
            checksum,
            flags,
        };

        // Validate entry consistency
        entry.validate(header).map_err(|e| binrw::Error::Custom {
            pos: reader.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        Ok(entry)
    }
}

impl BinWrite for DownloadFileEntry {
    type Args<'a> = &'a DownloadHeader;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        header: Self::Args<'_>,
    ) -> BinResult<()> {
        // Validate before writing
        self.validate(header).map_err(|e| binrw::Error::Custom {
            pos: writer.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        // Write encoding key (always 16 bytes)
        writer.write_all(self.encoding_key.as_bytes())?;

        // Write 40-bit file size
        self.file_size.write_options(writer, endian, ())?;

        // Write priority
        self.priority.write_options(writer, endian, ())?;

        // Write optional checksum (big-endian)
        if header.has_checksum() {
            if let Some(checksum) = self.checksum {
                checksum.write_options(writer, binrw::Endian::Big, ())?;
            } else {
                return Err(binrw::Error::Custom {
                    pos: writer.stream_position().unwrap_or(0),
                    err: Box::new(DownloadError::MissingChecksum),
                });
            }
        }

        // Write optional flags
        if header.flag_size() > 0 {
            if let Some(ref flags) = self.flags {
                writer.write_all(flags)?;
            } else {
                return Err(binrw::Error::Custom {
                    pos: writer.stream_position().unwrap_or(0),
                    err: Box::new(DownloadError::MissingFlags),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::download::header::DownloadHeader;
    use binrw::io::Cursor;

    #[test]
    fn test_file_size_40_creation() {
        // Normal sizes
        let size_1kb = FileSize40::new(1024).expect("Operation should succeed");
        assert_eq!(size_1kb.as_u64(), 1024);

        let size_one_gb = FileSize40::new(1024 * 1024 * 1024).expect("Operation should succeed");
        assert_eq!(size_one_gb.as_u64(), 1024 * 1024 * 1024);

        // Maximum 40-bit value
        let size_max = FileSize40::new(FileSize40::MAX).expect("Operation should succeed");
        assert_eq!(size_max.as_u64(), FileSize40::MAX);

        // Oversized should fail
        let result = FileSize40::new(FileSize40::MAX + 1);
        assert!(matches!(result, Err(DownloadError::FileSizeTooLarge(_))));
    }

    #[test]
    fn test_file_size_40_bytes() {
        let test_values = [0u64, 255, 65535, 16_777_215, 4_294_967_295, FileSize40::MAX];

        for &value in &test_values {
            let size = FileSize40::new(value).expect("Operation should succeed");
            let bytes = size.to_bytes();
            let restored = FileSize40::from_bytes(&bytes);
            assert_eq!(restored.as_u64(), value);
        }
    }

    #[test]
    fn test_file_size_40_human_readable() {
        assert_eq!(
            FileSize40::new(0)
                .expect("Operation should succeed")
                .to_human_readable(),
            "0 B"
        );
        assert_eq!(
            FileSize40::new(512)
                .expect("Operation should succeed")
                .to_human_readable(),
            "512 B"
        );
        assert_eq!(
            FileSize40::new(1024)
                .expect("Operation should succeed")
                .to_human_readable(),
            "1 KB"
        );
        assert_eq!(
            FileSize40::new(1536)
                .expect("Operation should succeed")
                .to_human_readable(),
            "1.5 KB"
        );
        assert_eq!(
            FileSize40::new(1024 * 1024)
                .expect("Operation should succeed")
                .to_human_readable(),
            "1 MB"
        );
        assert_eq!(
            FileSize40::new(1024 * 1024 * 1024)
                .expect("Operation should succeed")
                .to_human_readable(),
            "1 GB"
        );
        // Note: 1TB (1024^4) exceeds 40-bit limit, so use a smaller value
        assert_eq!(
            FileSize40::new(512u64 * 1024 * 1024 * 1024)
                .expect("Operation should succeed")
                .to_human_readable(),
            "512 GB"
        );
    }

    #[test]
    fn test_file_size_40_large_file_detection() {
        let small_file = FileSize40::new(u64::from(u32::MAX)).expect("Operation should succeed");
        assert!(!small_file.is_large_file());

        let large_file =
            FileSize40::new(u64::from(u32::MAX) + 1).expect("Operation should succeed");
        assert!(large_file.is_large_file());
    }

    #[test]
    fn test_download_entry_creation() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let entry = DownloadFileEntry::new(ekey, 1024, 5).expect("Operation should succeed");

        assert_eq!(entry.encoding_key, ekey);
        assert_eq!(entry.file_size.as_u64(), 1024);
        assert_eq!(entry.priority, 5);
        assert_eq!(entry.checksum, None);
        assert_eq!(entry.flags, None);
    }

    #[test]
    fn test_download_entry_with_options() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let entry = DownloadFileEntry::new(ekey, 1024, 5)
            .expect("Operation should succeed")
            .with_checksum(0x1234_5678)
            .with_flags(vec![0xAB, 0xCD]);

        assert_eq!(entry.checksum, Some(0x1234_5678));
        assert_eq!(entry.flags, Some(vec![0xAB, 0xCD]));
    }

    #[test]
    fn test_effective_priority_calculation() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let entry = DownloadFileEntry::new(ekey, 1024, 5).expect("Operation should succeed");

        // V1/V2 - no base priority adjustment
        let header_v1 = DownloadHeader::new_v1(1, 0, false);
        assert_eq!(entry.effective_priority(&header_v1), 5);

        let header_v2 = DownloadHeader::new_v2(1, 0, false, 0);
        assert_eq!(entry.effective_priority(&header_v2), 5);

        // V3 - with base priority adjustment
        let header_v3 = DownloadHeader::new_v3(1, 0, false, 0, -2);
        assert_eq!(entry.effective_priority(&header_v3), 7); // 5 - (-2) = 7

        let header_v3_pos = DownloadHeader::new_v3(1, 0, false, 0, 3);
        assert_eq!(entry.effective_priority(&header_v3_pos), 2); // 5 - 3 = 2
    }

    #[test]
    fn test_priority_categories() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let header = DownloadHeader::new_v1(1, 0, false);

        let critical = DownloadFileEntry::new(ekey, 1024, -5).expect("Operation should succeed");
        assert_eq!(
            critical.priority_category(&header),
            PriorityCategory::Critical
        );
        assert!(critical.is_critical(&header));
        assert!(critical.is_essential(&header));
        assert!(critical.is_high_priority(&header));

        let essential = DownloadFileEntry::new(ekey, 1024, 0).expect("Operation should succeed");
        assert_eq!(
            essential.priority_category(&header),
            PriorityCategory::Essential
        );
        assert!(!essential.is_critical(&header));
        assert!(essential.is_essential(&header));
        assert!(essential.is_high_priority(&header));

        let high = DownloadFileEntry::new(ekey, 1024, 1).expect("Operation should succeed");
        assert_eq!(high.priority_category(&header), PriorityCategory::High);
        assert!(!high.is_critical(&header));
        assert!(!high.is_essential(&header));
        assert!(high.is_high_priority(&header));

        let normal = DownloadFileEntry::new(ekey, 1024, 3).expect("Operation should succeed");
        assert_eq!(normal.priority_category(&header), PriorityCategory::Normal);
        assert!(!normal.is_critical(&header));
        assert!(!normal.is_essential(&header));
        assert!(!normal.is_high_priority(&header));

        let low = DownloadFileEntry::new(ekey, 1024, 10).expect("Operation should succeed");
        assert_eq!(low.priority_category(&header), PriorityCategory::Low);
        assert!(!low.is_critical(&header));
        assert!(!low.is_essential(&header));
        assert!(!low.is_high_priority(&header));
    }

    #[test]
    fn test_download_rank() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let header = DownloadHeader::new_v1(1, 0, false);

        let critical = DownloadFileEntry::new(ekey, 1024, -1).expect("Operation should succeed");
        assert_eq!(critical.download_rank(&header), 127); // 128 + (-1) = 127

        let essential = DownloadFileEntry::new(ekey, 1024, 0).expect("Operation should succeed");
        assert_eq!(essential.download_rank(&header), 128); // 128 + 0 = 128

        let normal = DownloadFileEntry::new(ekey, 1024, 5).expect("Operation should succeed");
        assert_eq!(normal.download_rank(&header), 133); // 128 + 5 = 133
    }

    #[test]
    fn test_entry_validation() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let mut entry = DownloadFileEntry::new(ekey, 1024, 5).expect("Operation should succeed");

        // Valid with no checksum/flags
        let header_v1 = DownloadHeader::new_v1(1, 0, false);
        assert!(entry.validate(&header_v1).is_ok());

        // Invalid - missing checksum when required
        let header_checksum = DownloadHeader::new_v1(1, 0, true);
        assert!(matches!(
            entry.validate(&header_checksum),
            Err(DownloadError::MissingChecksum)
        ));

        // Valid with checksum
        entry.checksum = Some(0x1234_5678);
        assert!(entry.validate(&header_checksum).is_ok());

        // Invalid - unexpected checksum
        assert!(matches!(
            entry.validate(&header_v1),
            Err(DownloadError::ChecksumsNotEnabled)
        ));

        // Test flags validation
        let header_flags = DownloadHeader::new_v2(1, 0, false, 2);
        entry.checksum = None; // Remove checksum for this test

        // Missing flags
        assert!(matches!(
            entry.validate(&header_flags),
            Err(DownloadError::MissingFlags)
        ));

        // Wrong flag size
        entry.flags = Some(vec![0xAB]); // 1 byte instead of 2
        assert!(matches!(
            entry.validate(&header_flags),
            Err(DownloadError::InvalidFlagSize(1, 2))
        ));

        // Correct flag size
        entry.flags = Some(vec![0xAB, 0xCD]);
        assert!(entry.validate(&header_flags).is_ok());
    }

    #[test]
    fn test_entry_serialization_size() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let entry = DownloadFileEntry::new(ekey, 1024, 5).expect("Operation should succeed");

        // V1 - no checksum, no flags: 16 + 5 + 1 = 22 bytes
        let header_v1 = DownloadHeader::new_v1(1, 0, false);
        assert_eq!(entry.serialized_size(&header_v1), 22);

        // V1 with checksum: 22 + 4 = 26 bytes
        let header_v1_checksum = DownloadHeader::new_v1(1, 0, true);
        assert_eq!(entry.serialized_size(&header_v1_checksum), 26);

        // V2 with flags: 22 + 2 = 24 bytes
        let header_v2_flags = DownloadHeader::new_v2(1, 0, false, 2);
        assert_eq!(entry.serialized_size(&header_v2_flags), 24);

        // V3 with checksum and flags: 22 + 4 + 1 = 27 bytes
        let header_v3_full = DownloadHeader::new_v3(1, 0, true, 1, 0);
        assert_eq!(entry.serialized_size(&header_v3_full), 27);
    }

    #[test]
    fn test_entry_round_trip() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let original = DownloadFileEntry::new(ekey, 1024, 5)
            .expect("Operation should succeed")
            .with_checksum(0x1234_5678)
            .with_flags(vec![0xAB, 0xCD]);

        let header = DownloadHeader::new_v2(1, 0, true, 2);

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Operation should succeed");

        // Deserialize
        let parsed =
            DownloadFileEntry::read_options(&mut Cursor::new(&buffer), binrw::Endian::Big, &header)
                .expect("Operation should succeed");

        assert_eq!(original, parsed);
    }
}

//! Patch Archive entry structure

use crate::patch_archive::utils::read_null_terminated_string;
use binrw::{
    BinRead, BinResult, BinWrite,
    io::{Read, Seek, Write},
};

/// Patch entry with old/new content key mapping and compression info
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchEntry {
    /// MD5 hash of original file content
    pub old_content_key: [u8; 16],
    /// MD5 hash of patched file content
    pub new_content_key: [u8; 16],
    /// MD5 hash of patch data for CDN lookup
    pub patch_encoding_key: [u8; 16],
    /// Compression specification string
    pub compression_info: String,
    /// Additional patch metadata
    pub additional_data: Vec<u8>,
}

impl BinRead for PatchEntry {
    type Args<'a> = (u8, u8, u8); // (file_key_size, old_key_size, patch_key_size)

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let (file_key_size, old_key_size, patch_key_size) = args;

        // Read keys (fixed size arrays, zero-padded if key size < 16)
        let mut old_key = [0u8; 16];
        let mut new_key = [0u8; 16];
        let mut patch_key = [0u8; 16];

        reader.read_exact(&mut old_key[..old_key_size as usize])?;
        reader.read_exact(&mut new_key[..file_key_size as usize])?;
        reader.read_exact(&mut patch_key[..patch_key_size as usize])?;

        // Read compression info (null-terminated string)
        let compression_info = read_null_terminated_string(reader)?;

        // For now, assume no additional data
        // This could be extended based on actual format requirements
        let additional_data = Vec::new();

        Ok(PatchEntry {
            old_content_key: old_key,
            new_content_key: new_key,
            patch_encoding_key: patch_key,
            compression_info,
            additional_data,
        })
    }
}

impl BinWrite for PatchEntry {
    type Args<'a> = (u8, u8, u8); // (file_key_size, old_key_size, patch_key_size)

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let (file_key_size, old_key_size, patch_key_size) = args;

        // Write keys (only the required number of bytes)
        writer.write_all(&self.old_content_key[..old_key_size as usize])?;
        writer.write_all(&self.new_content_key[..file_key_size as usize])?;
        writer.write_all(&self.patch_encoding_key[..patch_key_size as usize])?;

        // Write compression info with null terminator
        writer.write_all(self.compression_info.as_bytes())?;
        writer.write_all(&[0])?;

        // Write additional data
        writer.write_all(&self.additional_data)?;

        Ok(())
    }
}

impl PatchEntry {
    /// Create new patch entry with MD5 keys
    pub fn new(
        old_content_key: [u8; 16],
        new_content_key: [u8; 16],
        patch_encoding_key: [u8; 16],
        compression_info: String,
    ) -> Self {
        Self {
            old_content_key,
            new_content_key,
            patch_encoding_key,
            compression_info,
            additional_data: Vec::new(),
        }
    }

    /// Get content key as hex string for display
    pub fn old_content_key_hex(&self) -> String {
        hex::encode(self.old_content_key)
    }

    /// Get new content key as hex string for display
    pub fn new_content_key_hex(&self) -> String {
        hex::encode(self.new_content_key)
    }

    /// Get patch encoding key as hex string for display
    pub fn patch_encoding_key_hex(&self) -> String {
        hex::encode(self.patch_encoding_key)
    }

    /// Calculate serialized size of this entry
    pub fn serialized_size(
        &self,
        file_key_size: u8,
        old_key_size: u8,
        patch_key_size: u8,
    ) -> usize {
        (old_key_size + file_key_size + patch_key_size) as usize
            + self.compression_info.len()
            + 1 // null terminator
            + self.additional_data.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_patch_entry_creation() {
        let entry = PatchEntry::new([0x01; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());

        assert_eq!(entry.old_content_key, [0x01; 16]);
        assert_eq!(entry.new_content_key, [0x02; 16]);
        assert_eq!(entry.patch_encoding_key, [0x03; 16]);
        assert_eq!(entry.compression_info, "{*=z}");
        assert!(entry.additional_data.is_empty());
    }

    #[test]
    fn test_patch_entry_round_trip() {
        let entry = PatchEntry::new([0x11; 16], [0x22; 16], [0x33; 16], "{22=n,*=z}".to_string());

        // Serialize
        let mut writer = Vec::new();
        let args = (16u8, 16u8, 16u8);
        entry
            .write_options(&mut Cursor::new(&mut writer), binrw::Endian::Little, args)
            .expect("Operation should succeed");

        // Deserialize
        let parsed =
            PatchEntry::read_options(&mut Cursor::new(&writer), binrw::Endian::Little, args)
                .expect("Operation should succeed");

        assert_eq!(parsed, entry);
    }

    #[test]
    fn test_patch_entry_with_shorter_keys() {
        let entry = PatchEntry::new([0xAA; 16], [0xBB; 16], [0xCC; 16], "{*=n}".to_string());

        // Test with 8-byte keys
        let mut writer = Vec::new();
        let args = (8u8, 8u8, 8u8);
        entry
            .write_options(&mut Cursor::new(&mut writer), binrw::Endian::Little, args)
            .expect("Operation should succeed");

        // Should only write first 8 bytes of each key
        assert_eq!(writer.len(), 8 + 8 + 8 + "{*=n}".len() + 1);

        // Deserialize
        let parsed =
            PatchEntry::read_options(&mut Cursor::new(&writer), binrw::Endian::Little, args)
                .expect("Operation should succeed");

        // Keys should be truncated but compression info preserved
        assert_eq!(&parsed.old_content_key[..8], &[0xAA; 8]);
        assert_eq!(&parsed.old_content_key[8..], &[0; 8]);
        assert_eq!(parsed.compression_info, "{*=n}");
    }

    #[test]
    fn test_hex_display() {
        let entry = PatchEntry::new(
            [
                0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
                0xCD, 0xEF,
            ],
            [
                0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
                0x32, 0x10,
            ],
            [
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0x88,
            ],
            "{*=z}".to_string(),
        );

        assert_eq!(
            entry.old_content_key_hex(),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            entry.new_content_key_hex(),
            "fedcba9876543210fedcba9876543210"
        );
        assert_eq!(
            entry.patch_encoding_key_hex(),
            "11223344556677881122334455667788"
        );
    }
}

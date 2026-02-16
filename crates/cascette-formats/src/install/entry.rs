//! Install manifest file entry parsing and building

use binrw::{BinRead, BinResult, BinWrite};
use cascette_crypto::ContentKey;
use std::io::{Read, Seek, Write};

/// File entry in an install manifest
///
/// Each entry represents a file that can be installed, containing:
/// - File path (null-terminated string)
/// - Content key (16-byte MD5 hash)
/// - File size (4-byte big-endian integer)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallFileEntry {
    /// File path relative to game installation directory
    pub path: String,
    /// Content key (MD5 hash) identifying the file content
    pub content_key: ContentKey,
    /// File size in bytes
    pub file_size: u32,
    /// File type byte (V2 only, `None` for V1 manifests)
    pub file_type: Option<u8>,
}

impl InstallFileEntry {
    /// Create a new install file entry (V1, no file_type)
    pub fn new(path: String, content_key: ContentKey, file_size: u32) -> Self {
        Self {
            path,
            content_key,
            file_size,
            file_type: None,
        }
    }

    /// Create a new V2 install file entry with file_type
    pub fn new_v2(path: String, content_key: ContentKey, file_size: u32, file_type: u8) -> Self {
        Self {
            path,
            content_key,
            file_size,
            file_type: Some(file_type),
        }
    }

    /// Get the file name from the path
    pub fn file_name(&self) -> Option<&str> {
        self.path.split(['\\', '/']).next_back()
    }

    /// Get the directory path (parent directory)
    pub fn directory(&self) -> Option<&str> {
        let path = &self.path;
        match path.rfind(['\\', '/']) {
            Some(pos) => Some(&path[..pos]),
            None => None,
        }
    }

    /// Get the file extension
    #[allow(clippy::expect_used)] // file_name() already succeeded on line above
    pub fn extension(&self) -> Option<&str> {
        self.file_name()?
            .rfind('.')
            .map(|pos| &self.file_name().expect("File name should exist")[pos + 1..])
    }

    /// Check if the file path matches a pattern (case-insensitive)
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        let path_lower = self.path.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        // Support simple glob patterns with * wildcard
        if pattern_lower.contains('*') {
            let parts: Vec<&str> = pattern_lower.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return path_lower.starts_with(prefix) && path_lower.ends_with(suffix);
            } else if parts.len() == 3 && parts[0].is_empty() && parts[2].is_empty() {
                // Pattern like "*word*"
                return path_lower.contains(parts[1]);
            }
        }

        path_lower.contains(&pattern_lower)
    }

    /// Normalize path separators to forward slashes
    pub fn normalized_path(&self) -> String {
        self.path.replace('\\', "/")
    }
}

impl BinRead for InstallFileEntry {
    type Args<'a> = (u8, u8); // (ckey_length, version)

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        (ckey_length, version): Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read null-terminated file path
        let mut path_bytes = Vec::new();
        loop {
            let byte = u8::read_options(reader, endian, ())?;
            if byte == 0 {
                break;
            }
            path_bytes.push(byte);
        }
        let path = String::from_utf8(path_bytes).map_err(|e| binrw::Error::Custom {
            pos: reader.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        // Read content key
        let mut key_bytes = vec![0u8; ckey_length as usize];
        reader.read_exact(&mut key_bytes)?;

        // Convert to fixed-size array for ContentKey
        if key_bytes.len() != 16 {
            return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Invalid content key length: expected 16, got {}",
                        key_bytes.len()
                    ),
                )),
            });
        }

        let mut key_array = [0u8; 16];
        key_array.copy_from_slice(&key_bytes);
        let content_key = ContentKey::from_bytes(key_array);

        // Read file size (big-endian)
        let file_size = u32::read_options(reader, binrw::Endian::Big, ())?;

        // V2: read file_type byte
        let file_type = if version >= 2 {
            Some(u8::read_options(reader, endian, ())?)
        } else {
            None
        };

        Ok(Self {
            path,
            content_key,
            file_size,
            file_type,
        })
    }
}

impl BinWrite for InstallFileEntry {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write null-terminated file path
        writer.write_all(self.path.as_bytes())?;
        writer.write_all(&[0])?;

        // Write content key (always 16 bytes for MD5)
        writer.write_all(self.content_key.as_bytes())?;

        // Write file size (big-endian)
        self.file_size
            .write_options(writer, binrw::Endian::Big, ())?;

        // V2: write file_type byte if present
        if let Some(ft) = self.file_type {
            writer.write_all(&[ft])?;
        }

        Ok(())
    }
}

// Manual WriteEndian implementation required by binrw
impl binrw::meta::WriteEndian for InstallFileEntry {
    const ENDIAN: binrw::meta::EndianKind = binrw::meta::EndianKind::Endian(binrw::Endian::Big);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    #[test]
    fn test_file_entry_new() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let entry = InstallFileEntry::new(
            "Interface\\Icons\\INV_Misc_QuestionMark.blp".to_string(),
            content_key,
            1024,
        );

        assert_eq!(entry.path, "Interface\\Icons\\INV_Misc_QuestionMark.blp");
        assert_eq!(entry.content_key, content_key);
        assert_eq!(entry.file_size, 1024);
    }

    #[test]
    fn test_file_name_extraction() {
        let entry = InstallFileEntry::new(
            "Interface\\Icons\\INV_Misc_QuestionMark.blp".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            1024,
        );

        assert_eq!(entry.file_name(), Some("INV_Misc_QuestionMark.blp"));
        assert_eq!(entry.directory(), Some("Interface\\Icons"));
        assert_eq!(entry.extension(), Some("blp"));

        // Test with forward slashes
        let entry2 = InstallFileEntry::new(
            "interface/icons/test.txt".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            512,
        );

        assert_eq!(entry2.file_name(), Some("test.txt"));
        assert_eq!(entry2.directory(), Some("interface/icons"));
        assert_eq!(entry2.extension(), Some("txt"));

        // Test file in root
        let entry3 = InstallFileEntry::new(
            "root.file".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            256,
        );

        assert_eq!(entry3.file_name(), Some("root.file"));
        assert_eq!(entry3.directory(), None);
        assert_eq!(entry3.extension(), Some("file"));
    }

    #[test]
    fn test_pattern_matching() {
        let entry = InstallFileEntry::new(
            "Interface\\Icons\\INV_Misc_QuestionMark.blp".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            1024,
        );

        // Exact match (case insensitive)
        assert!(entry.matches_pattern("interface\\icons\\inv_misc_questionmark.blp"));

        // Partial match
        assert!(entry.matches_pattern("Icons"));
        assert!(entry.matches_pattern("QuestionMark"));
        assert!(entry.matches_pattern(".blp"));

        // Wildcard patterns
        assert!(entry.matches_pattern("Interface*blp"));
        assert!(entry.matches_pattern("*QuestionMark*"));
        assert!(entry.matches_pattern("*.blp"));

        // Non-matches
        assert!(!entry.matches_pattern("Sound"));
        assert!(!entry.matches_pattern("*.mp3"));
        assert!(!entry.matches_pattern("World*"));
    }

    #[test]
    fn test_path_normalization() {
        let entry = InstallFileEntry::new(
            "Interface\\Icons\\Test.blp".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            1024,
        );

        assert_eq!(entry.normalized_path(), "Interface/Icons/Test.blp");

        let entry2 = InstallFileEntry::new(
            "already/normalized/path.txt".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            512,
        );

        assert_eq!(entry2.normalized_path(), "already/normalized/path.txt");
    }

    #[test]
    fn test_file_entry_parsing() {
        // "test.txt\0" + content key (16 bytes) + size (4 bytes big-endian)
        let mut data = Vec::new();
        data.extend_from_slice(b"test.txt");
        data.push(0); // null terminator
        data.extend_from_slice(&[
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef,
        ]); // content key
        data.extend_from_slice(&[0x00, 0x00, 0x04, 0x00]); // size 1024 big-endian

        let entry = InstallFileEntry::read_options(
            &mut Cursor::new(&data),
            binrw::Endian::Big,
            (16, 1), // (ckey_length, version)
        )
        .expect("Operation should succeed");

        assert_eq!(entry.path, "test.txt");
        assert_eq!(
            entry.content_key,
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed")
        );
        assert_eq!(entry.file_size, 1024);
    }

    #[test]
    fn test_file_entry_round_trip() {
        let original = InstallFileEntry::new(
            "Interface\\Glues\\MainMenu.blp".to_string(),
            ContentKey::from_hex("fedcba9876543210fedcba9876543210")
                .expect("Operation should succeed"),
            2048,
        );

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write(&mut cursor)
            .expect("Operation should succeed");

        // Deserialize
        let parsed = InstallFileEntry::read_options(
            &mut Cursor::new(&buffer),
            binrw::Endian::Big,
            (16, 1), // (ckey_length, version)
        )
        .expect("Operation should succeed");

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_empty_path() {
        let entry = InstallFileEntry::new(
            String::new(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            0,
        );

        assert_eq!(entry.file_name(), Some(""));
        assert_eq!(entry.directory(), None);
        assert_eq!(entry.extension(), None);

        // Should handle serialization/deserialization of empty path
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        entry.write(&mut cursor).expect("Operation should succeed");

        let parsed =
            InstallFileEntry::read_options(&mut Cursor::new(&buffer), binrw::Endian::Big, (16, 1))
                .expect("Operation should succeed");

        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_long_path() {
        // Test with a very long path to ensure no buffer overflow issues
        let long_path = "a/".repeat(500) + "file.txt";
        let entry = InstallFileEntry::new(
            long_path.clone(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            12345,
        );

        assert_eq!(entry.path, long_path);
        assert_eq!(entry.file_name(), Some("file.txt"));

        // Test serialization of long path
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        entry.write(&mut cursor).expect("Operation should succeed");

        let parsed =
            InstallFileEntry::read_options(&mut Cursor::new(&buffer), binrw::Endian::Big, (16, 1))
                .expect("Operation should succeed");

        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_special_characters_in_path() {
        // Test paths with various special characters
        let paths = [
            "Test (1).txt",
            "Test-File_Name.blp",
            "Файл.txt", // Cyrillic
            "测试.txt", // Chinese
            "Tëst.txt", // Accented characters
        ];

        for path in &paths {
            let entry = InstallFileEntry::new(
                (*path).to_string(),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                1024,
            );

            // Test round-trip
            let mut buffer = Vec::new();
            let mut cursor = Cursor::new(&mut buffer);
            entry.write(&mut cursor).expect("Operation should succeed");

            let parsed = InstallFileEntry::read_options(
                &mut Cursor::new(&buffer),
                binrw::Endian::Big,
                (16, 1),
            )
            .expect("Operation should succeed");

            assert_eq!(entry, parsed);
        }
    }

    #[test]
    fn test_invalid_content_key_length() {
        let data = [
            b't', b'e', b's', b't', 0, // "test\0"
            0x01, 0x02, // Invalid: only 2 bytes instead of 16
        ];

        let result = InstallFileEntry::read_options(
            &mut Cursor::new(&data),
            binrw::Endian::Big,
            (2, 1), // (invalid ckey_length, version)
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_big_endian_file_size() {
        let entry = InstallFileEntry::new(
            "test.txt".to_string(),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            0x1234_5678, // Large value to test endianness
        );

        // Serialize
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        entry.write(&mut cursor).expect("Operation should succeed");

        // Check that file size is stored in big-endian
        let size_offset = buffer.len() - 4; // Last 4 bytes
        assert_eq!(buffer[size_offset], 0x12);
        assert_eq!(buffer[size_offset + 1], 0x34);
        assert_eq!(buffer[size_offset + 2], 0x56);
        assert_eq!(buffer[size_offset + 3], 0x78);

        // Verify round-trip
        let parsed =
            InstallFileEntry::read_options(&mut Cursor::new(&buffer), binrw::Endian::Big, (16, 1))
                .expect("Operation should succeed");

        assert_eq!(entry, parsed);
    }
}

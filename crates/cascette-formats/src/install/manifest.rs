//! Install manifest main structure and parsing logic

use crate::install::{
    entry::InstallFileEntry,
    error::{InstallError, Result},
    header::InstallHeader,
    tag::InstallTag,
};
use binrw::{BinRead, BinWrite, io::Cursor};

/// Complete install manifest containing header, tags, and file entries
///
/// Install manifests define which game files should be installed on disk
/// and use tags to organize files by platform, architecture, locale, etc.
/// The manifest uses a binary format with big-endian multi-byte fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallManifest {
    /// Manifest header with metadata
    pub header: InstallHeader,
    /// Tags for file categorization and filtering
    pub tags: Vec<InstallTag>,
    /// File entries with paths, content keys, and sizes
    pub entries: Vec<InstallFileEntry>,
}

impl InstallManifest {
    /// Parse install manifest from binary data
    ///
    /// # Format Layout
    /// 1. Header (10 bytes)
    /// 2. Tags (variable length)
    /// 3. File entries (variable length)
    ///
    /// # Errors
    /// Returns error if:
    /// - Invalid header magic or version
    /// - Truncated or corrupted data
    /// - Invalid UTF-8 strings in paths/names
    /// - Inconsistent counts between header and actual data
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Parse header
        let header = InstallHeader::read(&mut cursor)?;
        header.validate()?;

        // Parse tags
        let mut tags = Vec::with_capacity(header.tag_count as usize);
        for _ in 0..header.tag_count {
            let tag =
                InstallTag::read_options(&mut cursor, binrw::Endian::Big, header.entry_count)?;
            tags.push(tag);
        }

        // Parse file entries
        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            let entry = InstallFileEntry::read_options(
                &mut cursor,
                binrw::Endian::Big,
                header.ckey_length,
            )?;
            entries.push(entry);
        }

        let manifest = Self {
            header,
            tags,
            entries,
        };

        // Validate internal consistency
        manifest.validate()?;

        Ok(manifest)
    }

    /// Build install manifest to binary data
    ///
    /// Serializes the manifest in the correct binary format with proper
    /// endianness and null-terminated strings.
    pub fn build(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        // Write header
        self.header.write(&mut cursor)?;

        // Write tags
        for tag in &self.tags {
            tag.write(&mut cursor)?;
        }

        // Write file entries
        for entry in &self.entries {
            entry.write(&mut cursor)?;
        }

        Ok(buffer)
    }

    /// Validate internal consistency of the manifest
    pub fn validate(&self) -> Result<()> {
        // Validate header
        self.header.validate()?;

        // Validate counts match actual data
        if self.tags.len() != self.header.tag_count as usize {
            return Err(InstallError::BitMaskSizeMismatch {
                expected: self.header.tag_count as usize,
                actual: self.tags.len(),
            });
        }

        if self.entries.len() != self.header.entry_count as usize {
            return Err(InstallError::BitMaskSizeMismatch {
                expected: self.header.entry_count as usize,
                actual: self.entries.len(),
            });
        }

        // Validate all tag bit masks have correct size
        let expected_mask_size = self.header.bit_mask_size();
        for tag in &self.tags {
            if tag.bit_mask.len() != expected_mask_size {
                return Err(InstallError::BitMaskSizeMismatch {
                    expected: expected_mask_size,
                    actual: tag.bit_mask.len(),
                });
            }
        }

        Ok(())
    }

    /// Get total size of all files in the manifest
    pub fn total_install_size(&self) -> u64 {
        self.entries
            .iter()
            .map(|entry| u64::from(entry.file_size))
            .sum()
    }

    /// Find tag by name
    pub fn find_tag(&self, name: &str) -> Option<&InstallTag> {
        self.tags.iter().find(|tag| tag.name == name)
    }

    /// Find mutable tag by name
    pub fn find_tag_mut(&mut self, name: &str) -> Option<&mut InstallTag> {
        self.tags.iter_mut().find(|tag| tag.name == name)
    }

    /// Get files associated with a specific tag
    pub fn get_files_for_tag(&self, tag_name: &str) -> Vec<(usize, &InstallFileEntry)> {
        let Some(tag) = self.find_tag(tag_name) else {
            return Vec::new();
        };

        self.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| tag.has_file(*index))
            .collect()
    }

    /// Get files matching all specified tags (intersection)
    pub fn get_files_for_tags(&self, tag_names: &[&str]) -> Vec<(usize, &InstallFileEntry)> {
        let tags: Vec<&InstallTag> = tag_names
            .iter()
            .filter_map(|name| self.find_tag(name))
            .collect();

        // If any requested tag doesn't exist, return empty
        if tags.is_empty() || tags.len() != tag_names.len() {
            return Vec::new();
        }

        self.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| tags.iter().all(|tag| tag.has_file(*index)))
            .collect()
    }

    /// Get files matching any of the specified tags (union)
    pub fn get_files_for_any_tag(&self, tag_names: &[&str]) -> Vec<(usize, &InstallFileEntry)> {
        let tags: Vec<&InstallTag> = tag_names
            .iter()
            .filter_map(|name| self.find_tag(name))
            .collect();

        if tags.is_empty() {
            return Vec::new();
        }

        self.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| tags.iter().any(|tag| tag.has_file(*index)))
            .collect()
    }

    /// Calculate install size for specific tags
    pub fn calculate_install_size(&self, tag_names: &[&str]) -> u64 {
        self.get_files_for_tags(tag_names)
            .into_iter()
            .map(|(_, entry)| u64::from(entry.file_size))
            .sum()
    }

    /// Get statistics about the manifest
    pub fn stats(&self) -> InstallStats {
        let total_size = self.total_install_size();
        let tagged_files = self
            .tags
            .iter()
            .map(super::tag::InstallTag::file_count)
            .max()
            .unwrap_or(0);

        InstallStats {
            total_files: self.entries.len(),
            total_tags: self.tags.len(),
            total_size,
            tagged_files,
        }
    }

    /// Find files by path pattern (case-insensitive, supports * wildcard)
    pub fn find_files(&self, pattern: &str) -> Vec<(usize, &InstallFileEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.matches_pattern(pattern))
            .collect()
    }

    /// Get unique file extensions in the manifest
    pub fn get_extensions(&self) -> Vec<String> {
        let mut extensions: Vec<String> = self
            .entries
            .iter()
            .filter_map(|entry| entry.extension())
            .map(str::to_lowercase)
            .collect();

        extensions.sort();
        extensions.dedup();
        extensions
    }

    /// Get files by extension (case-insensitive)
    pub fn get_files_by_extension(&self, extension: &str) -> Vec<(usize, &InstallFileEntry)> {
        let ext_lower = extension.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry
                    .extension()
                    .is_some_and(|e| e.to_lowercase() == ext_lower)
            })
            .collect()
    }

    /// Verify round-trip compatibility
    pub fn verify_round_trip(data: &[u8]) -> Result<()> {
        let manifest = Self::parse(data)?;
        let rebuilt = manifest.build()?;

        if data != rebuilt.as_slice() {
            return Err(InstallError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Round-trip verification failed: rebuilt data differs from original",
            )));
        }

        Ok(())
    }
}

/// Statistics about an install manifest
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallStats {
    /// Total number of files
    pub total_files: usize,
    /// Total number of tags
    pub total_tags: usize,
    /// Total size of all files in bytes
    pub total_size: u64,
    /// Number of files that have at least one tag
    pub tagged_files: usize,
}

impl crate::CascFormat for InstallManifest {
    fn parse(data: &[u8]) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::install::{builder::InstallManifestBuilder, tag::TagType};
    use cascette_crypto::ContentKey;

    fn create_test_manifest() -> InstallManifest {
        InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("x86_64".to_string(), TagType::Architecture)
            .add_tag("enUS".to_string(), TagType::Locale)
            .add_file(
                "Interface\\Icons\\test1.blp".to_string(),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                1024,
            )
            .add_file(
                "Sound\\Music\\test2.mp3".to_string(),
                ContentKey::from_hex("fedcba9876543210fedcba9876543210")
                    .expect("Operation should succeed"),
                2048,
            )
            .add_file(
                "World\\Maps\\test3.wdt".to_string(),
                ContentKey::from_hex("11111111111111112222222222222222")
                    .expect("Operation should succeed"),
                4096,
            )
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .associate_file_with_tag(0, "x86_64")
            .expect("Operation should succeed")
            .associate_file_with_tag(1, "Windows")
            .expect("Operation should succeed")
            .associate_file_with_tag(2, "enUS")
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed")
    }

    #[test]
    fn test_manifest_parsing() {
        let manifest = create_test_manifest();
        let data = manifest.build().expect("Operation should succeed");

        // Parse it back
        let parsed = InstallManifest::parse(&data).expect("Operation should succeed");

        assert_eq!(parsed.header.tag_count, 3);
        assert_eq!(parsed.header.entry_count, 3);
        assert_eq!(parsed.tags.len(), 3);
        assert_eq!(parsed.entries.len(), 3);
    }

    #[test]
    fn test_manifest_validation() {
        let manifest = create_test_manifest();
        assert!(manifest.validate().is_ok());

        // Test invalid header
        let mut invalid_manifest = manifest.clone();
        invalid_manifest.header.magic = *b"XX";
        assert!(matches!(
            invalid_manifest.validate(),
            Err(InstallError::InvalidMagic(_))
        ));

        // Test count mismatch
        let mut invalid_counts = manifest;
        invalid_counts.header.tag_count = 5; // Wrong count
        assert!(matches!(
            invalid_counts.validate(),
            Err(InstallError::BitMaskSizeMismatch { .. })
        ));
    }

    #[test]
    fn test_total_install_size() {
        let manifest = create_test_manifest();
        assert_eq!(manifest.total_install_size(), 1024 + 2048 + 4096);
    }

    #[test]
    fn test_find_tag() {
        let manifest = create_test_manifest();

        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");
        assert_eq!(windows_tag.tag_type, TagType::Platform);
        assert!(windows_tag.has_file(0));
        assert!(windows_tag.has_file(1));
        assert!(!windows_tag.has_file(2));

        assert!(manifest.find_tag("NonExistent").is_none());
    }

    #[test]
    fn test_get_files_for_tag() {
        let manifest = create_test_manifest();

        let windows_files = manifest.get_files_for_tag("Windows");
        assert_eq!(windows_files.len(), 2);
        assert_eq!(windows_files[0].0, 0); // First file
        assert_eq!(windows_files[1].0, 1); // Second file

        let enus_files = manifest.get_files_for_tag("enUS");
        assert_eq!(enus_files.len(), 1);
        assert_eq!(enus_files[0].0, 2); // Third file
    }

    #[test]
    fn test_get_files_for_tags_intersection() {
        let manifest = create_test_manifest();

        // Files that have both Windows AND x86_64 tags
        let filtered_files = manifest.get_files_for_tags(&["Windows", "x86_64"]);
        assert_eq!(filtered_files.len(), 1);
        assert_eq!(filtered_files[0].0, 0); // Only first file has both

        // Non-existent tag combination
        let empty_files = manifest.get_files_for_tags(&["Windows", "NonExistent"]);
        assert_eq!(empty_files.len(), 0);
    }

    #[test]
    fn test_get_files_for_any_tag_union() {
        let manifest = create_test_manifest();

        // Files that have Windows OR enUS tags
        let filtered_files = manifest.get_files_for_any_tag(&["Windows", "enUS"]);
        assert_eq!(filtered_files.len(), 3); // All three files match

        // Files that have x86_64 OR enUS tags
        let arch_or_locale = manifest.get_files_for_any_tag(&["x86_64", "enUS"]);
        assert_eq!(arch_or_locale.len(), 2); // Files 0 and 2
    }

    #[test]
    fn test_calculate_install_size() {
        let manifest = create_test_manifest();

        let windows_size = manifest.calculate_install_size(&["Windows"]);
        assert_eq!(windows_size, 1024 + 2048); // Files 0 and 1

        let x64_size = manifest.calculate_install_size(&["x86_64"]);
        assert_eq!(x64_size, 1024); // Only file 0

        let intersection_size = manifest.calculate_install_size(&["Windows", "x86_64"]);
        assert_eq!(intersection_size, 1024); // Only file 0 has both
    }

    #[test]
    fn test_manifest_stats() {
        let manifest = create_test_manifest();
        let stats = manifest.stats();

        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.total_tags, 3);
        assert_eq!(stats.total_size, 1024 + 2048 + 4096);
        assert_eq!(stats.tagged_files, 2); // Maximum files in any tag
    }

    #[test]
    fn test_find_files_pattern() {
        let manifest = create_test_manifest();

        // Find by extension
        let blp_files = manifest.find_files("*.blp");
        assert_eq!(blp_files.len(), 1);
        assert!(
            std::path::Path::new(&blp_files[0].1.path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("blp"))
        );

        // Find by directory
        let interface_files = manifest.find_files("Interface*");
        assert_eq!(interface_files.len(), 1);
        assert!(interface_files[0].1.path.starts_with("Interface"));

        // Find by partial name
        let test_files = manifest.find_files("test");
        assert_eq!(test_files.len(), 3); // All files have "test" in name
    }

    #[test]
    fn test_get_extensions() {
        let manifest = create_test_manifest();
        let extensions = manifest.get_extensions();

        assert_eq!(extensions, vec!["blp", "mp3", "wdt"]);
    }

    #[test]
    fn test_get_files_by_extension() {
        let manifest = create_test_manifest();

        let blp_files = manifest.get_files_by_extension("blp");
        assert_eq!(blp_files.len(), 1);
        assert_eq!(blp_files[0].1.extension(), Some("blp"));

        let mp3_files = manifest.get_files_by_extension("MP3"); // Case insensitive
        assert_eq!(mp3_files.len(), 1);
        assert_eq!(mp3_files[0].1.extension(), Some("mp3"));

        let nonexistent = manifest.get_files_by_extension("txt");
        assert_eq!(nonexistent.len(), 0);
    }

    #[test]
    fn test_round_trip() {
        let original = create_test_manifest();
        let data = original.build().expect("Operation should succeed");
        let parsed = InstallManifest::parse(&data).expect("Operation should succeed");

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_verify_round_trip() {
        let manifest = create_test_manifest();
        let data = manifest.build().expect("Operation should succeed");

        // Should pass round-trip verification
        assert!(InstallManifest::verify_round_trip(&data).is_ok());

        // Corrupted data should fail
        let mut corrupted_data = data;
        corrupted_data[10] = 0xFF; // Corrupt some byte
        assert!(InstallManifest::verify_round_trip(&corrupted_data).is_err());
    }

    #[test]
    fn test_empty_manifest() {
        let empty = InstallManifestBuilder::new()
            .build()
            .expect("Operation should succeed");
        let data = empty.build().expect("Operation should succeed");
        let parsed = InstallManifest::parse(&data).expect("Operation should succeed");

        assert_eq!(parsed.header.tag_count, 0);
        assert_eq!(parsed.header.entry_count, 0);
        assert_eq!(parsed.tags.len(), 0);
        assert_eq!(parsed.entries.len(), 0);
        assert_eq!(parsed.total_install_size(), 0);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_large_manifest() {
        // Test with many files to ensure scalability
        let mut builder = InstallManifestBuilder::new();
        builder = builder.add_tag("Windows".to_string(), TagType::Platform);

        for i in 0..1000 {
            let path = format!("file_{i:04}.bin");
            let content_key = ContentKey::from_data(path.as_bytes());
            builder = builder.add_file(
                path,
                content_key,
                u32::try_from(i).expect("Operation should succeed") * 100,
            );

            // Associate every 10th file with Windows tag
            if i % 10 == 0 {
                builder = builder
                    .associate_file_with_tag(i, "Windows")
                    .expect("Operation should succeed");
            }
        }

        let manifest = builder.build().expect("Operation should succeed");
        assert_eq!(manifest.entries.len(), 1000);
        assert_eq!(manifest.tags.len(), 1);

        // Test serialization and parsing
        let data = manifest.build().expect("Operation should succeed");
        let parsed = InstallManifest::parse(&data).expect("Operation should succeed");
        assert_eq!(manifest, parsed);

        // Test tag filtering
        let windows_files = manifest.get_files_for_tag("Windows");
        assert_eq!(windows_files.len(), 100); // Every 10th file
    }

    #[test]
    fn test_invalid_manifest_data() {
        // Test with truncated data
        let data = [b'I', b'N', 1]; // Incomplete header
        assert!(InstallManifest::parse(&data).is_err());

        // Test with invalid magic
        let data = [b'X', b'X', 1, 16, 0, 0, 0, 0, 0, 0];
        assert!(InstallManifest::parse(&data).is_err());

        // Test with version mismatch
        let data = [b'I', b'N', 2, 16, 0, 0, 0, 0, 0, 0]; // Version 2
        assert!(InstallManifest::parse(&data).is_err());
    }
}

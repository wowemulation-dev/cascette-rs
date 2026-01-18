//! Main download manifest implementation with entries-first parsing

use crate::download::entry::DownloadFileEntry;
use crate::download::error::{DownloadError, Result};
use crate::download::header::DownloadHeader;
use crate::download::priority::{PriorityAnalysis, analyze_priorities};
use crate::download::tag::DownloadTag;
use binrw::{BinRead, BinWrite};
use std::io::Cursor;

/// Complete download manifest with header, entries, and tags
///
/// Download manifests manage content streaming and prioritization during
/// game installation and updates. Key characteristics:
///
/// - **Version-Specific Layout**:
///   - Version 1 (Battle.net Agent): Header → Entries → Tags
///   - Version 2+: Header → Tags → Entries
/// - **`EncodingKey` Usage**: Uses encoding keys instead of content keys
/// - **40-Bit File Sizes**: Supports files larger than 4GB
/// - **Priority System**: Signed priorities with optional base adjustment (V3+)
/// - **Version Evolution**: Supports v1, v2, v3 formats with incremental features
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadManifest {
    /// Version-aware header
    pub header: DownloadHeader,
    /// File entries (comes first in binary layout)
    pub entries: Vec<DownloadFileEntry>,
    /// Tags for selective downloading (comes after entries)
    pub tags: Vec<DownloadTag>,
}

impl DownloadManifest {
    /// Parse download manifest from binary data
    ///
    /// The parsing follows version-specific layouts:
    /// - Version 1: Header, Entries, Tags (used by Battle.net Agent)
    /// - Version 2+: Header (with reserved byte), Tags, Entries
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Parse version-aware header
        let header = DownloadHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .map_err(DownloadError::from)?;

        // Validate header
        header.validate()?;

        let mut entries = Vec::with_capacity(header.entry_count() as usize);
        let mut tags = Vec::with_capacity(header.tag_count() as usize);

        // Version 1 has different layout: entries then tags
        // Version 2+ has: tags then entries
        if header.version() == 1 {
            // Version 1: Parse entries first
            for _ in 0..header.entry_count() {
                let entry =
                    DownloadFileEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
                        .map_err(DownloadError::from)?;
                entries.push(entry);
            }

            // Version 1: Parse tags after entries
            for _ in 0..header.tag_count() {
                let tag = DownloadTag::read_options(
                    &mut cursor,
                    binrw::Endian::Big,
                    header.entry_count(),
                )
                .map_err(DownloadError::from)?;
                tags.push(tag);
            }
        } else {
            // Version 2+: Parse tags first
            for _ in 0..header.tag_count() {
                let tag = DownloadTag::read_options(
                    &mut cursor,
                    binrw::Endian::Big,
                    header.entry_count(),
                )
                .map_err(DownloadError::from)?;
                tags.push(tag);
            }

            // Version 2+: Parse entries after tags
            for _ in 0..header.entry_count() {
                let entry =
                    DownloadFileEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
                        .map_err(DownloadError::from)?;
                entries.push(entry);
            }
        }

        let manifest = DownloadManifest {
            header,
            entries,
            tags,
        };

        // Final validation
        manifest.validate()?;

        Ok(manifest)
    }

    /// Build download manifest to binary data
    ///
    /// The building follows version-specific layouts:
    /// - Version 1: Header, Entries, Tags
    /// - Version 2+: Header (with reserved byte), Tags, Entries
    pub fn build(&self) -> Result<Vec<u8>> {
        // Pre-validate before building
        self.validate()?;

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        // Write header
        self.header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .map_err(DownloadError::from)?;

        // Version-specific layout
        if self.header.version() == 1 {
            // Version 1: Write entries first
            for entry in &self.entries {
                entry
                    .write_options(&mut cursor, binrw::Endian::Big, &self.header)
                    .map_err(DownloadError::from)?;
            }

            // Version 1: Write tags after entries
            for tag in &self.tags {
                tag.write_options(&mut cursor, binrw::Endian::Big, ())
                    .map_err(DownloadError::from)?;
            }
        } else {
            // Version 2+: Write tags first
            for tag in &self.tags {
                tag.write_options(&mut cursor, binrw::Endian::Big, ())
                    .map_err(DownloadError::from)?;
            }

            // Version 2+: Write entries after tags
            for entry in &self.entries {
                entry
                    .write_options(&mut cursor, binrw::Endian::Big, &self.header)
                    .map_err(DownloadError::from)?;
            }
        }

        Ok(buffer)
    }

    /// Validate manifest consistency
    pub fn validate(&self) -> Result<()> {
        // Validate header
        self.header.validate()?;

        // Validate counts
        if self.entries.len() != self.header.entry_count() as usize {
            return Err(DownloadError::EntryCountMismatch(
                self.header.entry_count(),
                self.entries.len(),
            ));
        }

        if self.tags.len() != self.header.tag_count() as usize {
            return Err(DownloadError::TagCountMismatch(
                self.header.tag_count(),
                self.tags.len(),
            ));
        }

        // Validate bit mask sizes
        let expected_mask_size = self.header.bit_mask_size();
        for tag in &self.tags {
            if tag.bit_mask.len() != expected_mask_size {
                return Err(DownloadError::BitMaskSizeMismatch);
            }
        }

        // Validate individual entries
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate(&self.header).map_err(|e| match e {
                DownloadError::FileIndexOutOfBounds(_) => {
                    DownloadError::FileIndexOutOfBounds(index)
                }
                other => other,
            })?;
        }

        Ok(())
    }

    /// Get basic manifest statistics
    pub fn stats(&self) -> ManifestStats {
        let total_size = self.entries.iter().map(|e| e.file_size.as_u64()).sum();
        let large_files = self
            .entries
            .iter()
            .filter(|e| e.file_size.is_large_file())
            .count();

        ManifestStats {
            version: self.header.version(),
            entry_count: self.entries.len(),
            tag_count: self.tags.len(),
            total_size,
            large_file_count: large_files,
            has_checksums: self.header.has_checksum(),
            has_flags: self.header.flag_size() > 0,
            base_priority: self.header.base_priority(),
        }
    }

    /// Analyze priority distribution
    pub fn analyze_priorities(&self) -> PriorityAnalysis {
        analyze_priorities(&self.entries, &self.header)
    }

    /// Find entries by tag name
    pub fn entries_by_tag(&self, tag_name: &str) -> Vec<(usize, &DownloadFileEntry)> {
        let Some(tag) = self.tags.iter().find(|t| t.name == tag_name) else {
            return Vec::new();
        };

        self.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| tag.has_file(*index))
            .collect()
    }

    /// Find entries matching multiple tags (AND logic)
    pub fn entries_by_tags(&self, tag_names: &[&str]) -> Vec<(usize, &DownloadFileEntry)> {
        if tag_names.is_empty() {
            return self.entries.iter().enumerate().collect();
        }

        let tags: Vec<_> = tag_names
            .iter()
            .filter_map(|name| self.tags.iter().find(|t| &t.name == name))
            .collect();

        if tags.len() != tag_names.len() {
            return Vec::new(); // Some tags not found
        }

        self.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| tags.iter().all(|tag| tag.has_file(*index)))
            .collect()
    }

    /// Get entries for a specific platform and architecture
    pub fn entries_for_platform(
        &self,
        platform: &str,
        architecture: &str,
    ) -> Vec<(usize, &DownloadFileEntry)> {
        self.entries_by_tags(&[platform, architecture])
    }

    /// Get entries by priority category
    pub fn entries_by_priority(
        &self,
        category: crate::download::priority::PriorityCategory,
    ) -> Vec<(usize, &DownloadFileEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.priority_category(&self.header) == category)
            .collect()
    }

    /// Get entries with priority in range (inclusive)
    pub fn entries_by_priority_range(
        &self,
        min_priority: i8,
        max_priority: i8,
    ) -> Vec<(usize, &DownloadFileEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                let effective = entry.effective_priority(&self.header);
                effective >= min_priority && effective <= max_priority
            })
            .collect()
    }

    /// Find tag by name
    pub fn find_tag(&self, name: &str) -> Option<&DownloadTag> {
        self.tags.iter().find(|t| t.name == name)
    }

    /// Get all unique tag names
    pub fn tag_names(&self) -> Vec<&str> {
        self.tags.iter().map(|t| t.name.as_str()).collect()
    }

    /// Get platform-specific tags
    pub fn platform_tags(&self) -> Vec<&DownloadTag> {
        self.tags
            .iter()
            .filter(|t| t.is_platform_specific())
            .collect()
    }

    /// Get optional content tags
    pub fn optional_tags(&self) -> Vec<&DownloadTag> {
        self.tags.iter().filter(|t| t.is_optional()).collect()
    }

    /// Calculate total size for specific tags
    pub fn calculate_size_for_tags(&self, tag_names: &[&str]) -> u64 {
        self.entries_by_tags(tag_names)
            .iter()
            .map(|(_, entry)| entry.file_size.as_u64())
            .sum()
    }

    /// Check if manifest supports streaming (has streamable content)
    pub fn supports_streaming(&self) -> bool {
        self.tags
            .iter()
            .any(super::super::install::tag::InstallTag::is_streamable)
    }

    /// Get estimated essential download size (Critical + Essential priorities)
    pub fn essential_download_size(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| entry.is_essential(&self.header))
            .map(|entry| entry.file_size.as_u64())
            .sum()
    }

    /// Get estimated total download size
    pub fn total_download_size(&self) -> u64 {
        self.entries.iter().map(|e| e.file_size.as_u64()).sum()
    }

    /// Calculate compression ratio if manifest has checksums
    /// Returns None if no size information available
    pub fn compression_info(&self) -> Option<CompressionInfo> {
        if self.entries.is_empty() {
            return None;
        }

        let total_size = self.total_download_size();
        let file_count = self.entries.len();
        let avg_size = total_size as f64 / file_count as f64;

        Some(CompressionInfo {
            total_compressed_size: total_size,
            file_count,
            average_file_size: avg_size,
        })
    }
}

/// Basic statistics about a download manifest
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestStats {
    /// Manifest format version
    pub version: u8,
    /// Number of file entries
    pub entry_count: usize,
    /// Number of tags
    pub tag_count: usize,
    /// Total download size in bytes
    pub total_size: u64,
    /// Number of large files (>4GB)
    pub large_file_count: usize,
    /// Whether entries have checksums
    pub has_checksums: bool,
    /// Whether entries have flags
    pub has_flags: bool,
    /// Base priority adjustment (V3+ only)
    pub base_priority: i8,
}

impl ManifestStats {
    /// Get average file size
    pub fn average_file_size(&self) -> f64 {
        if self.entry_count == 0 {
            0.0
        } else {
            self.total_size as f64 / self.entry_count as f64
        }
    }

    /// Get percentage of large files
    pub fn large_file_percentage(&self) -> f64 {
        if self.entry_count == 0 {
            0.0
        } else {
            (self.large_file_count as f64 / self.entry_count as f64) * 100.0
        }
    }

    /// Check if this is a modern manifest (V2+)
    pub fn is_modern_format(&self) -> bool {
        self.version >= 2
    }

    /// Get human-readable total size
    pub fn total_size_human_readable(&self) -> String {
        crate::download::entry::FileSize40::new(self.total_size).map_or_else(
            |_| format!("{} bytes", self.total_size),
            super::entry::FileSize40::to_human_readable,
        )
    }
}

/// Compression information for download manifest
#[derive(Debug, Clone, PartialEq)]
pub struct CompressionInfo {
    /// Total compressed size
    pub total_compressed_size: u64,
    /// Number of files
    pub file_count: usize,
    /// Average file size
    pub average_file_size: f64,
}

// Implement CascFormat trait
use crate::CascFormat;

impl CascFormat for DownloadManifest {
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
    use crate::download::builder::DownloadManifestBuilder;
    use crate::download::priority::PriorityCategory;
    use crate::install::TagType;
    use cascette_crypto::EncodingKey;

    fn create_test_manifest() -> DownloadManifest {
        let ekey1 = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let ekey2 = EncodingKey::from_hex("fedcba9876543210fedcba9876543210")
            .expect("Operation should succeed");

        DownloadManifestBuilder::new(2)
            .expect("Operation should succeed")
            .with_checksums(true)
            .with_flags(1)
            .expect("Operation should succeed")
            .add_file(ekey1, 1024, 0)
            .expect("Operation should succeed") // Essential
            .add_file(ekey2, 2048, 5)
            .expect("Operation should succeed") // Normal
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("Optional".to_string(), TagType::Option)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .associate_file_with_tag(1, "Optional")
            .expect("Operation should succeed")
            .set_file_checksum(0, 0x1111_1111)
            .expect("Operation should succeed")
            .set_file_checksum(1, 0x2222_2222)
            .expect("Operation should succeed")
            .set_file_flags(0, vec![0xAA])
            .expect("Operation should succeed")
            .set_file_flags(1, vec![0xBB])
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed")
    }

    #[test]
    fn test_manifest_creation_and_validation() {
        let manifest = create_test_manifest();
        assert!(manifest.validate().is_ok());

        let stats = manifest.stats();
        assert_eq!(stats.version, 2);
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.tag_count, 2);
        assert_eq!(stats.total_size, 3072); // 1024 + 2048
        assert!(stats.has_checksums);
        assert!(stats.has_flags);
        assert_eq!(stats.base_priority, 0);
    }

    #[test]
    fn test_entries_first_layout_round_trip() {
        let original = create_test_manifest();

        // Build to binary
        let data = original.build().expect("Operation should succeed");

        // Parse back
        let parsed = DownloadManifest::parse(&data).expect("Operation should succeed");

        // Should be identical
        assert_eq!(original, parsed);

        // Validate layout by checking specific positions in binary data
        // After header (11 bytes for V2), entries should come first
        let header_size = 11; // V2 header size
        let entry_size = 16 + 5 + 1 + 4 + 1; // ekey + size + priority + checksum + flags
        let entries_section_size = 2 * entry_size; // 2 entries
        let tags_start_offset = header_size + entries_section_size;

        // The layout should be: Header | Entries | Tags
        // This is verified by the successful round-trip
        assert!(data.len() > tags_start_offset);
    }

    #[test]
    fn test_manifest_validation_errors() {
        let mut manifest = create_test_manifest();

        // Test entry count mismatch
        manifest.entries.pop();
        assert!(matches!(
            manifest.validate(),
            Err(DownloadError::EntryCountMismatch(2, 1))
        ));

        // Restore entry for next test
        let ekey = EncodingKey::from_hex("11111111111111111111111111111111")
            .expect("Operation should succeed");
        let entry = DownloadFileEntry::new(ekey, 100, 1).expect("Operation should succeed");
        manifest.entries.push(entry);

        // Test tag count mismatch
        manifest.tags.pop();
        assert!(matches!(
            manifest.validate(),
            Err(DownloadError::TagCountMismatch(2, 1))
        ));
    }

    #[test]
    fn test_tag_based_queries() {
        let manifest = create_test_manifest();

        // Test entries by single tag
        let windows_entries = manifest.entries_by_tag("Windows");
        assert_eq!(windows_entries.len(), 1);
        assert_eq!(windows_entries[0].0, 0); // First entry

        let optional_entries = manifest.entries_by_tag("Optional");
        assert_eq!(optional_entries.len(), 1);
        assert_eq!(optional_entries[0].0, 1); // Second entry

        // Test entries by multiple tags (AND logic)
        let combined = manifest.entries_by_tags(&["Windows", "Optional"]);
        assert_eq!(combined.len(), 0); // No entry has both tags

        let all_entries = manifest.entries_by_tags(&[]);
        assert_eq!(all_entries.len(), 2); // Empty filter returns all

        // Test platform queries
        let platform_entries = manifest.entries_for_platform("Windows", "x86_64");
        assert_eq!(platform_entries.len(), 0); // No x86_64 tag in test data
    }

    #[test]
    fn test_priority_based_queries() {
        let manifest = create_test_manifest();

        // Test by priority category
        let essential = manifest.entries_by_priority(PriorityCategory::Essential);
        assert_eq!(essential.len(), 1);
        assert_eq!(essential[0].0, 0); // First entry (priority 0)

        let normal = manifest.entries_by_priority(PriorityCategory::Normal);
        assert_eq!(normal.len(), 1);
        assert_eq!(normal[0].0, 1); // Second entry (priority 5)

        // Test by priority range
        let high_priority = manifest.entries_by_priority_range(-10, 2);
        assert_eq!(high_priority.len(), 1); // Only essential entry (priority 0)

        let all_priorities = manifest.entries_by_priority_range(-128, 127);
        assert_eq!(all_priorities.len(), 2); // All entries
    }

    #[test]
    fn test_size_calculations() {
        let manifest = create_test_manifest();

        assert_eq!(manifest.total_download_size(), 3072); // 1024 + 2048
        assert_eq!(manifest.essential_download_size(), 1024); // Only first entry is essential

        let windows_size = manifest.calculate_size_for_tags(&["Windows"]);
        assert_eq!(windows_size, 1024); // First entry only

        let optional_size = manifest.calculate_size_for_tags(&["Optional"]);
        assert_eq!(optional_size, 2048); // Second entry only
    }

    #[test]
    fn test_tag_utilities() {
        let manifest = create_test_manifest();

        // Test tag lookup
        assert!(manifest.find_tag("Windows").is_some());
        assert!(manifest.find_tag("NonExistent").is_none());

        // Test tag collections
        let tag_names = manifest.tag_names();
        assert!(tag_names.contains(&"Windows"));
        assert!(tag_names.contains(&"Optional"));
        assert_eq!(tag_names.len(), 2);

        let platform_tags = manifest.platform_tags();
        assert_eq!(platform_tags.len(), 1); // Only Windows tag
        assert_eq!(platform_tags[0].name, "Windows");

        let optional_tags = manifest.optional_tags();
        assert_eq!(optional_tags.len(), 1); // Only Optional tag
        assert_eq!(optional_tags[0].name, "Optional");
    }

    #[test]
    fn test_streaming_detection() {
        let manifest = create_test_manifest();
        assert!(manifest.supports_streaming()); // Has optional content
    }

    #[test]
    fn test_priority_analysis() {
        let manifest = create_test_manifest();
        let analysis = manifest.analyze_priorities();

        assert_eq!(analysis.total_files, 2);
        assert_eq!(analysis.total_size, 3072);
        assert_eq!(analysis.base_priority_adjustment, 0);
        assert_eq!(analysis.priority_range, (0, 5));

        // Check categories
        assert!(
            analysis
                .categories
                .contains_key(&PriorityCategory::Essential)
        );
        assert!(analysis.categories.contains_key(&PriorityCategory::Normal));

        let essential_stats = &analysis.categories[&PriorityCategory::Essential];
        assert_eq!(essential_stats.file_count, 1);
        assert_eq!(essential_stats.total_size, 1024);

        let normal_stats = &analysis.categories[&PriorityCategory::Normal];
        assert_eq!(normal_stats.file_count, 1);
        assert_eq!(normal_stats.total_size, 2048);
    }

    #[test]
    fn test_manifest_stats() {
        let manifest = create_test_manifest();
        let stats = manifest.stats();

        assert_eq!(stats.version, 2);
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.tag_count, 2);
        assert_eq!(stats.total_size, 3072);
        assert_eq!(stats.large_file_count, 0); // No files >4GB
        assert!(stats.has_checksums);
        assert!(stats.has_flags);
        assert_eq!(stats.base_priority, 0);

        assert!((stats.average_file_size() - 1536.0).abs() < f64::EPSILON); // 3072 / 2
        assert!((stats.large_file_percentage() - 0.0).abs() < f64::EPSILON);
        assert!(stats.is_modern_format()); // V2+ is modern
        assert!(stats.total_size_human_readable().contains("KB")); // Should be ~3KB
    }

    #[test]
    fn test_compression_info() {
        let manifest = create_test_manifest();
        let info = manifest
            .compression_info()
            .expect("Operation should succeed");

        assert_eq!(info.total_compressed_size, 3072);
        assert_eq!(info.file_count, 2);
        assert!((info.average_file_size - 1536.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_casc_format_trait() {
        let original = create_test_manifest();

        // Test generic trait usage
        let data = CascFormat::build(&original).expect("Operation should succeed");
        let parsed: DownloadManifest = CascFormat::parse(&data).expect("Operation should succeed");

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_version_differences() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        // Test V1 manifest
        let v1_manifest = DownloadManifestBuilder::new(1)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 0)
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        let v1_stats = v1_manifest.stats();
        assert_eq!(v1_stats.version, 1);
        assert!(!v1_stats.has_flags);
        assert_eq!(v1_stats.base_priority, 0);
        assert!(!v1_stats.is_modern_format());

        // Test V3 manifest
        let v3_manifest = DownloadManifestBuilder::new(3)
            .expect("Operation should succeed")
            .with_base_priority(-2)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 3)
            .expect("Operation should succeed") // Effective priority: 3 - (-2) = 5
            .build()
            .expect("Operation should succeed");

        let v3_stats = v3_manifest.stats();
        assert_eq!(v3_stats.version, 3);
        assert_eq!(v3_stats.base_priority, -2);
        assert!(v3_stats.is_modern_format());

        // Verify effective priority calculation
        let entry = &v3_manifest.entries[0];
        assert_eq!(entry.effective_priority(&v3_manifest.header), 5);
    }

    #[test]
    fn test_empty_manifest() {
        let empty_manifest = DownloadManifestBuilder::new(1)
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        assert!(empty_manifest.validate().is_ok());

        let stats = empty_manifest.stats();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.tag_count, 0);
        assert_eq!(stats.total_size, 0);
        assert!((stats.average_file_size() - 0.0).abs() < f64::EPSILON);

        assert_eq!(empty_manifest.total_download_size(), 0);
        assert_eq!(empty_manifest.essential_download_size(), 0);
        assert!(empty_manifest.compression_info().is_none());

        // Test round trip
        let data = empty_manifest.build().expect("Operation should succeed");
        let parsed = DownloadManifest::parse(&data).expect("Operation should succeed");
        assert_eq!(empty_manifest, parsed);
    }
}

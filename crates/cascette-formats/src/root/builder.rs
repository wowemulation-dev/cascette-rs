//! Root file builder for creating CASC root files

use crate::root::{
    block::RootBlock,
    entry::{RootRecord, calculate_name_hash},
    error::{Result, RootError},
    flags::{ContentFlags, LocaleFlags},
    header::RootHeader,
    version::RootVersion,
};
use cascette_crypto::md5::{ContentKey, FileDataId};
use std::collections::HashMap;
use std::io::Cursor;

/// Builder for creating root files
#[derive(Debug)]
pub struct RootBuilder {
    /// Target version
    version: RootVersion,
    /// Blocks organized by (locale, content) flags
    blocks: HashMap<(LocaleFlags, ContentFlags), RootBlock>,
}

impl RootBuilder {
    /// Create new builder for specified version
    pub fn new(version: RootVersion) -> Self {
        Self {
            version,
            blocks: HashMap::new(),
        }
    }

    /// Add file with automatic name hash calculation
    pub fn add_file(
        &mut self,
        fdid: FileDataId,
        ckey: ContentKey,
        path: Option<&str>,
        locale: LocaleFlags,
        content: ContentFlags,
    ) {
        let name_hash = path.map(calculate_name_hash);
        let record = RootRecord::new(fdid, ckey, name_hash);
        self.add_record(record, locale, content);
    }

    /// Add file with explicit name hash
    pub fn add_file_with_hash(
        &mut self,
        fdid: FileDataId,
        ckey: ContentKey,
        name_hash: Option<u64>,
        locale: LocaleFlags,
        content: ContentFlags,
    ) {
        let record = RootRecord::new(fdid, ckey, name_hash);
        self.add_record(record, locale, content);
    }

    /// Add file to specific block (internal API for rebuild)
    pub fn add_file_in_block(
        &mut self,
        fdid: FileDataId,
        ckey: ContentKey,
        name_hash: Option<u64>,
        locale: LocaleFlags,
        content: ContentFlags,
    ) {
        let record = RootRecord::new(fdid, ckey, name_hash);
        self.add_record(record, locale, content);
    }

    /// Add record to appropriate block
    fn add_record(&mut self, record: RootRecord, locale: LocaleFlags, content: ContentFlags) {
        let key = (locale, content);
        let block = self
            .blocks
            .entry(key)
            .or_insert_with(|| RootBlock::new(content, locale));

        block.add_record(record);
    }

    /// Build complete root file
    pub fn build(&mut self) -> Result<Vec<u8>> {
        if self.blocks.is_empty() {
            return Err(RootError::CorruptedBlockHeader(
                "No blocks to build".to_string(),
            ));
        }

        // Sort records within each block for optimal delta encoding
        for block in self.blocks.values_mut() {
            block.sort_records();
        }

        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);

        // Calculate file statistics
        let total_files: u32 = self
            .blocks
            .values()
            .map(super::block::RootBlock::num_records)
            .sum();
        let named_files: u32 = self
            .blocks
            .values()
            .map(|b| {
                // We're building the file, so we can assume the count fits in u32
                #[allow(clippy::cast_possible_truncation)]
                {
                    b.records.iter().filter(|r| r.has_name_hash()).count() as u32
                }
            })
            .sum();

        // Write header if needed
        if self.version.has_header() {
            let header = match self.version {
                RootVersion::V1 => unreachable!("V1 has no header"),
                RootVersion::V2 => RootHeader::new_v2(total_files, named_files),
                RootVersion::V3 => RootHeader::new_v3v4(3, total_files, named_files),
                RootVersion::V4 => RootHeader::new_v3v4(4, total_files, named_files),
            };
            header.write(&mut cursor)?;
        }

        // Write blocks in deterministic order for reproducible builds
        let mut sorted_blocks: Vec<_> = self.blocks.iter().collect();
        sorted_blocks.sort_by_key(|(key, _)| (key.0.value(), key.1.value));

        for (_, block) in sorted_blocks {
            let has_named_files = named_files > 0;
            block.write(&mut cursor, self.version, has_named_files)?;
        }

        Ok(output)
    }

    /// Get current block count
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Get current file count
    pub fn file_count(&self) -> usize {
        self.blocks.values().map(|b| b.records.len()).sum()
    }

    /// Clear all blocks
    pub fn clear(&mut self) {
        self.blocks.clear();
    }

    /// Remove a file by FileDataID from all blocks
    ///
    /// Returns true if the file was found and removed from at least one block.
    pub fn remove_file(&mut self, fdid: FileDataId) -> bool {
        let mut removed = false;

        for block in self.blocks.values_mut() {
            let original_len = block.records.len();
            block.records.retain(|r| r.file_data_id != fdid);
            if block.records.len() < original_len {
                removed = true;
            }
        }

        // Clean up empty blocks
        self.blocks.retain(|_, block| !block.records.is_empty());

        removed
    }

    /// Remove a file by FileDataID from a specific block (locale/content flags)
    ///
    /// Returns true if the file was found and removed.
    pub fn remove_file_from_block(
        &mut self,
        fdid: FileDataId,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> bool {
        let key = (locale, content);
        if let Some(block) = self.blocks.get_mut(&key) {
            let original_len = block.records.len();
            block.records.retain(|r| r.file_data_id != fdid);
            let removed = block.records.len() < original_len;

            // Clean up empty block
            if block.records.is_empty() {
                self.blocks.remove(&key);
            }

            removed
        } else {
            false
        }
    }

    /// Check if a file exists by FileDataID (in any block)
    pub fn has_file(&self, fdid: FileDataId) -> bool {
        self.blocks
            .values()
            .any(|block| block.records.iter().any(|r| r.file_data_id == fdid))
    }

    /// Check if a file exists in a specific block
    pub fn has_file_in_block(
        &self,
        fdid: FileDataId,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> bool {
        let key = (locale, content);
        self.blocks
            .get(&key)
            .is_some_and(|block| block.records.iter().any(|r| r.file_data_id == fdid))
    }

    /// Find a file by FileDataID and return its content key
    ///
    /// If the file exists in multiple blocks, returns the first match.
    pub fn find_file(&self, fdid: FileDataId) -> Option<ContentKey> {
        for block in self.blocks.values() {
            if let Some(record) = block.records.iter().find(|r| r.file_data_id == fdid) {
                return Some(record.content_key);
            }
        }
        None
    }

    /// Find all entries for a FileDataID across all blocks
    pub fn find_all_entries(
        &self,
        fdid: FileDataId,
    ) -> Vec<(LocaleFlags, ContentFlags, ContentKey)> {
        let mut results = Vec::new();
        for ((locale, content), block) in &self.blocks {
            for record in &block.records {
                if record.file_data_id == fdid {
                    results.push((*locale, *content, record.content_key));
                }
            }
        }
        results
    }

    /// Update a file's content key by FileDataID (in all blocks where it exists)
    ///
    /// Returns the number of entries updated.
    pub fn update_file(&mut self, fdid: FileDataId, new_ckey: ContentKey) -> usize {
        let mut updated = 0;
        for block in self.blocks.values_mut() {
            for record in &mut block.records {
                if record.file_data_id == fdid {
                    record.content_key = new_ckey;
                    updated += 1;
                }
            }
        }
        updated
    }

    /// Get version
    pub const fn version(&self) -> RootVersion {
        self.version
    }

    /// Set version (useful for format conversion)
    pub fn set_version(&mut self, version: RootVersion) {
        self.version = version;
    }

    /// Get block statistics
    pub fn block_stats(&self) -> Vec<(LocaleFlags, ContentFlags, usize)> {
        self.blocks
            .iter()
            .map(|((locale, content), block)| (*locale, *content, block.records.len()))
            .collect()
    }

    /// Optimize blocks by merging compatible ones
    /// This reduces the number of blocks when files have compatible flags
    pub fn optimize_blocks(&mut self) {
        // For now, this is a placeholder
        // In a more advanced implementation, we could:
        // 1. Merge blocks with identical locale flags but compatible content flags
        // 2. Split large blocks to improve access patterns
        // 3. Reorder blocks for better compression

        // Sort records in all blocks for better delta compression
        for block in self.blocks.values_mut() {
            block.sort_records();
        }
    }

    /// Estimate final file size
    pub fn estimate_size(&self) -> usize {
        let header_size = if self.version.has_header() {
            match self.version {
                RootVersion::V1 => 0,
                RootVersion::V2 => 12,                   // magic + 2 x u32
                RootVersion::V3 | RootVersion::V4 => 20, // magic + header_size + version + 2 x u32 + padding
            }
        } else {
            0
        };

        let named_files = self
            .blocks
            .values()
            .map(|b| {
                // We're estimating size for building, so we can assume count fits in u32
                #[allow(clippy::cast_possible_truncation)]
                {
                    b.records.iter().filter(|r| r.has_name_hash()).count() as u32
                }
            })
            .sum::<u32>();

        let blocks_size: usize = self
            .blocks
            .values()
            .map(|b| b.calculate_size(self.version, named_files > 0))
            .sum();

        header_size + blocks_size
    }

    /// Create builder from existing root file (for modification)
    pub fn from_root_file(root_file: &crate::root::file::RootFile) -> Self {
        let mut builder = Self::new(root_file.version);

        // Add all records from existing file
        for block in &root_file.blocks {
            for record in &block.records {
                builder.add_record(record.clone(), block.locale_flags(), block.content_flags());
            }
        }

        builder
    }

    /// Validate builder state before building
    pub fn validate(&self) -> Result<()> {
        if self.blocks.is_empty() {
            return Err(RootError::CorruptedBlockHeader(
                "No blocks to validate".to_string(),
            ));
        }

        // Check that all blocks have records
        for ((locale, content), block) in &self.blocks {
            if block.records.is_empty() {
                return Err(RootError::CorruptedBlockHeader(format!(
                    "Empty block for locale {locale:?} content {content:?}"
                )));
            }

            // Check FileDataID uniqueness within block
            let mut seen_fdids = std::collections::HashSet::new();
            for record in &block.records {
                if !seen_fdids.insert(record.file_data_id) {
                    return Err(RootError::CorruptedBlockHeader(format!(
                        "Duplicate FileDataID {} in block",
                        record.file_data_id.get()
                    )));
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

    fn create_test_builder() -> RootBuilder {
        let mut builder = RootBuilder::new(RootVersion::V2);

        // Add test files
        builder.add_file(
            FileDataId::new(100),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some("Interface\\Icons\\INV_Misc_QuestionMark.blp"),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        builder.add_file(
            FileDataId::new(200),
            ContentKey::from_hex("fedcba9876543210fedcba9876543210")
                .expect("Operation should succeed"),
            Some("World\\Maps\\TestMap\\TestMap.wdt"),
            LocaleFlags::new(LocaleFlags::ENUS | LocaleFlags::DEDE),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        builder.add_file(
            FileDataId::new(300),
            ContentKey::from_hex("abcdefabcdefabcdefabcdefabcdefab")
                .expect("Operation should succeed"),
            None,
            LocaleFlags::new(LocaleFlags::ALL),
            ContentFlags::new(ContentFlags::INSTALL | ContentFlags::NO_NAME_HASH),
        );

        builder
    }

    #[test]
    fn test_builder_basic() {
        let builder = create_test_builder();

        assert_eq!(builder.file_count(), 3);
        assert!(builder.block_count() > 0);
        assert_eq!(builder.version(), RootVersion::V2);
    }

    #[test]
    fn test_builder_build_v1() {
        let mut builder = RootBuilder::new(RootVersion::V1);
        builder.add_file(
            FileDataId::new(42),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some("test.txt"),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        let data = builder.build().expect("Test operation should succeed");
        assert!(!data.is_empty());

        // V1 should start directly with block data (no header)
        assert!(data.len() >= 12); // At least block header size
    }

    #[test]
    fn test_builder_build_v2() {
        let mut builder = create_test_builder();
        let data = builder.build().expect("Test operation should succeed");
        assert!(!data.is_empty());

        // V2 should start with TSFM magic (real WoW output)
        assert_eq!(&data[0..4], b"TSFM");
    }

    #[test]
    fn test_builder_build_v3() {
        let mut builder = RootBuilder::new(RootVersion::V3);
        builder.add_file(
            FileDataId::new(42),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some("test.txt"),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        let data = builder.build().expect("Test operation should succeed");
        assert!(!data.is_empty());

        // V3 should start with TSFM magic (real WoW output)
        assert_eq!(&data[0..4], b"TSFM");

        // Check header structure (TSFM + header_size + version + ...)
        assert!(data.len() >= 20); // Minimum header size for V3
    }

    #[test]
    fn test_builder_build_v4() {
        let mut builder = RootBuilder::new(RootVersion::V4);
        builder.add_file(
            FileDataId::new(42),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some("test.txt"),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL | ContentFlags::BUNDLE), // Use V4 features
        );

        let data = builder.build().expect("Test operation should succeed");
        assert!(!data.is_empty());
        assert_eq!(&data[0..4], b"TSFM");
    }

    #[test]
    fn test_builder_round_trip() {
        for version in [
            RootVersion::V1,
            RootVersion::V2,
            RootVersion::V3,
            RootVersion::V4,
        ] {
            let mut builder = RootBuilder::new(version);

            // Add test data
            builder.add_file(
                FileDataId::new(123),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                Some("test/file.txt"),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );

            let data = builder.build().expect("Test operation should succeed");
            let parsed =
                crate::root::file::RootFile::parse(&data).expect("Test operation should succeed");

            assert_eq!(parsed.version, version);
            assert!(parsed.total_files() >= 1);

            // Test file resolution works
            let resolved = parsed.resolve_by_id(
                FileDataId::new(123),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );
            assert!(resolved.is_some());
        }
    }

    #[test]
    fn test_builder_empty_blocks() {
        let mut builder = RootBuilder::new(RootVersion::V2);

        // Building without files should fail
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_optimize_blocks() {
        let mut builder = create_test_builder();

        let original_count = builder.file_count();
        builder.optimize_blocks();

        // File count should remain the same
        assert_eq!(builder.file_count(), original_count);

        // Should still build successfully
        let data = builder.build().expect("Test operation should succeed");
        assert!(!data.is_empty());
    }

    #[test]
    fn test_builder_estimate_size() {
        let builder = create_test_builder();
        let estimated_size = builder.estimate_size();

        // Should have reasonable estimate
        assert!(estimated_size > 50); // At least header + minimal block data
        assert!(estimated_size < 10_000); // But not unreasonably large

        // Actual size should be close to estimate
        let mut builder_copy = RootBuilder::new(builder.version());
        for ((locale, content), block) in &builder.blocks {
            for record in &block.records {
                builder_copy.add_record(record.clone(), *locale, *content);
            }
        }

        let actual_data = builder_copy.build().expect("Test operation should succeed");
        let actual_size = actual_data.len();

        // Allow some variance but should be in the right ballpark
        assert!(
            estimated_size >= actual_size / 2 && estimated_size <= actual_size * 2,
            "Estimate {estimated_size} vs actual {actual_size} too far apart"
        );
    }

    #[test]
    fn test_builder_validation() {
        let builder = create_test_builder();
        assert!(builder.validate().is_ok());

        let empty_builder = RootBuilder::new(RootVersion::V2);
        assert!(empty_builder.validate().is_err());
    }

    #[test]
    fn test_builder_block_stats() {
        let builder = create_test_builder();
        let stats = builder.block_stats();

        assert!(!stats.is_empty());

        // Check that stats sum up correctly
        let total_files: usize = stats.iter().map(|(_, _, count)| *count).sum();
        assert_eq!(total_files, builder.file_count());
    }

    #[test]
    fn test_builder_clear() {
        let mut builder = create_test_builder();
        assert!(builder.file_count() > 0);
        assert!(builder.block_count() > 0);

        builder.clear();
        assert_eq!(builder.file_count(), 0);
        assert_eq!(builder.block_count(), 0);
    }

    #[test]
    fn test_builder_version_change() {
        let mut builder = create_test_builder();
        assert_eq!(builder.version(), RootVersion::V2);

        builder.set_version(RootVersion::V4);
        assert_eq!(builder.version(), RootVersion::V4);

        // Should still build with new version
        let data = builder.build().expect("Test operation should succeed");
        let parsed =
            crate::root::file::RootFile::parse(&data).expect("Test operation should succeed");
        assert_eq!(parsed.version, RootVersion::V4);
    }

    #[test]
    fn test_builder_with_explicit_hash() {
        let mut builder = RootBuilder::new(RootVersion::V2);

        builder.add_file_with_hash(
            FileDataId::new(42),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some(0x1234_567890abcdef),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        let data = builder.build().expect("Test operation should succeed");
        let parsed =
            crate::root::file::RootFile::parse(&data).expect("Test operation should succeed");

        assert_eq!(parsed.total_files(), 1);

        // Should be able to resolve by the explicit hash
        let resolved = parsed.resolve_by_hash(
            0x1234_567890abcdef,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_some());
    }

    #[test]
    fn test_builder_remove_file() {
        let mut builder = create_test_builder();
        let initial_count = builder.file_count();
        assert!(initial_count >= 3);

        // Check file exists
        assert!(builder.has_file(FileDataId::new(100)));

        // Remove file
        let removed = builder.remove_file(FileDataId::new(100));
        assert!(removed);
        assert!(!builder.has_file(FileDataId::new(100)));
        assert_eq!(builder.file_count(), initial_count - 1);

        // Try to remove non-existent file
        let removed_again = builder.remove_file(FileDataId::new(100));
        assert!(!removed_again);

        // Should still build successfully
        let data = builder.build().expect("Test operation should succeed");
        assert!(!data.is_empty());
    }

    #[test]
    fn test_builder_remove_file_from_block() {
        let mut builder = RootBuilder::new(RootVersion::V2);

        let locale = LocaleFlags::new(LocaleFlags::ENUS);
        let content = ContentFlags::new(ContentFlags::INSTALL);

        builder.add_file(
            FileDataId::new(100),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some("test.txt"),
            locale,
            content,
        );

        assert!(builder.has_file_in_block(FileDataId::new(100), locale, content));

        let removed = builder.remove_file_from_block(FileDataId::new(100), locale, content);
        assert!(removed);
        assert!(!builder.has_file_in_block(FileDataId::new(100), locale, content));
    }

    #[test]
    fn test_builder_find_file() {
        let builder = create_test_builder();

        // Find existing file
        let ckey = builder.find_file(FileDataId::new(100));
        assert!(ckey.is_some());

        // Find non-existent file
        let ckey = builder.find_file(FileDataId::new(99999));
        assert!(ckey.is_none());
    }

    #[test]
    fn test_builder_find_all_entries() {
        let mut builder = RootBuilder::new(RootVersion::V2);

        let ckey = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        // Add same file to multiple blocks (different locales)
        builder.add_file(
            FileDataId::new(100),
            ckey,
            Some("test.txt"),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        builder.add_file(
            FileDataId::new(100),
            ckey,
            Some("test.txt"),
            LocaleFlags::new(LocaleFlags::DEDE),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        let entries = builder.find_all_entries(FileDataId::new(100));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_builder_update_file() {
        let mut builder = create_test_builder();

        let new_ckey = ContentKey::from_hex("ffffffffffffffffffffffffffffffff")
            .expect("Operation should succeed");

        let updated = builder.update_file(FileDataId::new(100), new_ckey);
        assert_eq!(updated, 1);

        // Verify update
        let found_ckey = builder.find_file(FileDataId::new(100));
        assert_eq!(found_ckey, Some(new_ckey));
    }

    #[test]
    fn test_builder_has_file() {
        let builder = create_test_builder();

        assert!(builder.has_file(FileDataId::new(100)));
        assert!(builder.has_file(FileDataId::new(200)));
        assert!(builder.has_file(FileDataId::new(300)));
        assert!(!builder.has_file(FileDataId::new(99999)));
    }
}

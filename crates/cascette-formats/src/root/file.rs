//! Main root file parser and container

use crate::root::{
    block::RootBlock,
    entry::{RootEntry, RootLookupTables},
    error::{Result, RootError},
    flags::{ContentFlags, LocaleFlags},
    header::RootHeader,
    version::RootVersion,
};
use cascette_crypto::md5::{ContentKey, FileDataId};
use std::io::{Cursor, Read, Seek, SeekFrom};

/// Complete root file with header, blocks, and lookup tables
#[derive(Debug)]
pub struct RootFile {
    /// File format version
    pub version: RootVersion,
    /// File header (None for V1)
    pub header: Option<RootHeader>,
    /// Data blocks containing file records
    pub blocks: Vec<RootBlock>,
    /// Lookup tables for efficient resolution
    lookups: RootLookupTables,
}

impl RootFile {
    /// Parse root file from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        Self::parse_from_reader(&mut cursor)
    }

    /// Parse root file from reader
    pub fn parse_from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        // Detect version (preliminary - may be updated from header)
        let detected_version = RootVersion::detect(reader)?;

        // Parse header if present
        let header = if detected_version.has_header() {
            Some(RootHeader::read(reader, detected_version)?)
        } else {
            None
        };

        // Use header's version if available (handles extended header cases)
        let version = match &header {
            Some(h) => h.version(),
            None => detected_version,
        };

        // Determine if file has named files
        let has_named_files = match &header {
            Some(h) => h.named_files() > 0,
            None => true, // V1 always has named files
        };

        // Parse blocks
        let mut blocks = Vec::new();

        loop {
            // Check if we've reached end of file
            let current_pos = reader.stream_position()?;
            reader.seek(SeekFrom::End(0))?;
            let end_pos = reader.stream_position()?;
            reader.seek(SeekFrom::Start(current_pos))?;

            if current_pos >= end_pos {
                break;
            }

            // Try to parse next block
            match RootBlock::parse(reader, version, has_named_files) {
                Ok(block) => {
                    // Skip empty blocks -- they can appear between valid blocks.
                    // EOF termination is handled by the position check above.
                    if block.num_records() > 0 {
                        blocks.push(block);
                    }
                }
                Err(e) => {
                    // If we haven't parsed any blocks yet, this is a real error
                    if blocks.is_empty() {
                        return Err(e);
                    }
                    // Otherwise assume we've reached the end
                    break;
                }
            }
        }

        // Build lookup tables
        let mut root_file = Self {
            version,
            header,
            blocks,
            lookups: RootLookupTables::new(),
        };

        root_file.build_lookups();

        Ok(root_file)
    }

    /// Get total number of files
    pub fn total_files(&self) -> u32 {
        self.header.as_ref().map_or_else(
            || {
                self.blocks
                    .iter()
                    .map(super::block::RootBlock::num_records)
                    .sum()
            },
            super::header::RootHeader::total_files,
        )
    }

    /// Get number of named files
    pub fn named_files(&self) -> u32 {
        self.header.as_ref().map_or_else(
            || {
                self.blocks
                    .iter()
                    .map(|b| {
                        // CASC root files are not expected to have more than 4 billion records per block
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            b.records.iter().filter(|r| r.has_name_hash()).count() as u32
                        }
                    })
                    .sum()
            },
            super::header::RootHeader::named_files,
        )
    }

    /// Get number of blocks
    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// Resolve file by `FileDataID`
    pub fn resolve_by_id(
        &self,
        fdid: FileDataId,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> Option<ContentKey> {
        self.lookups.resolve_by_id(fdid, locale, content)
    }

    /// Resolve file by path
    pub fn resolve_by_path(
        &self,
        path: &str,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> Option<ContentKey> {
        self.lookups.resolve_by_path(path, locale, content)
    }

    /// Resolve file by name hash
    pub fn resolve_by_hash(
        &self,
        name_hash: u64,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> Option<ContentKey> {
        self.lookups.resolve_by_hash(name_hash, locale, content)
    }

    /// Get all entries for a `FileDataID`
    pub fn get_entries_by_id(&self, fdid: FileDataId) -> Option<&Vec<RootEntry>> {
        self.lookups.get_entries_by_id(fdid)
    }

    /// Get all entries for a path
    pub fn get_entries_by_path(&self, path: &str) -> Option<&Vec<RootEntry>> {
        self.lookups.get_entries_by_path(path)
    }

    /// Iterate over all file records
    pub fn iter_records(&self) -> impl Iterator<Item = &crate::root::entry::RootRecord> {
        self.blocks.iter().flat_map(|block| block.records.iter())
    }

    /// Get lookup table statistics
    pub fn lookup_stats(&self) -> (usize, usize) {
        (self.lookups.fdid_count(), self.lookups.name_count())
    }

    /// Build lookup tables from blocks
    fn build_lookups(&mut self) {
        // Calculate capacity hints
        let total_records: usize = self.blocks.iter().map(|b| b.records.len()).sum();
        let named_records: usize = self
            .blocks
            .iter()
            .map(|b| b.records.iter().filter(|r| r.has_name_hash()).count())
            .sum();

        self.lookups = RootLookupTables::with_capacity(total_records, named_records);

        for (block_idx, block) in self.blocks.iter().enumerate() {
            for record in &block.records {
                let entry = RootEntry::new(
                    block_idx,
                    record.content_key,
                    block.locale_flags(),
                    block.content_flags(),
                );

                self.lookups
                    .add_entry(record.file_data_id, record.name_hash, entry);
            }
        }
    }

    /// Rebuild lookup tables (useful after modifying blocks)
    pub fn rebuild_lookups(&mut self) {
        self.lookups.clear();
        self.build_lookups();
    }

    /// Validate file structure and data integrity
    pub fn validate(&self) -> Result<()> {
        // Check version consistency
        if let Some(header) = &self.header
            && header.version() != self.version
        {
            return Err(RootError::CorruptedBlockHeader(format!(
                "Version mismatch: header says {:?}, detected {:?}",
                header.version(),
                self.version
            )));
        }

        // Validate block structure
        for (i, block) in self.blocks.iter().enumerate() {
            // Check record count consistency
            // CASC blocks are not expected to have more than 4 billion records
            #[allow(clippy::cast_possible_truncation)]
            let actual_record_count = block.records.len() as u32;
            if block.header.num_records != actual_record_count {
                return Err(RootError::CorruptedBlockHeader(format!(
                    "Block {}: header says {} records, found {}",
                    i,
                    block.header.num_records,
                    block.records.len()
                )));
            }

            // Check FileDataID ordering for delta encoding efficiency
            // FileDataIDs should be in ascending order within a block
            let mut prev_id: Option<u32> = None;
            for record in &block.records {
                let current_id = record.file_data_id.get();
                if let Some(prev) = prev_id
                    && current_id < prev
                {
                    return Err(RootError::InvalidDelta);
                }
                prev_id = Some(current_id);
            }

            // Check name hash presence consistency
            let has_names = block.has_name_hashes(self.version, self.named_files() > 0);
            for (j, record) in block.records.iter().enumerate() {
                if has_names && !record.has_name_hash() {
                    return Err(RootError::CorruptedBlockHeader(format!(
                        "Block {i} record {j}: expected name hash but none found"
                    )));
                }
                if !has_names && record.has_name_hash() {
                    return Err(RootError::CorruptedBlockHeader(format!(
                        "Block {i} record {j}: unexpected name hash found"
                    )));
                }
            }
        }

        // Validate header file counts if present
        if let Some(header) = &self.header {
            let actual_total = self.total_files();
            let actual_named = self.named_files();

            if header.total_files() != actual_total {
                return Err(RootError::CorruptedBlockHeader(format!(
                    "Header total_files ({}) doesn't match actual ({})",
                    header.total_files(),
                    actual_total
                )));
            }

            if header.named_files() != actual_named {
                return Err(RootError::CorruptedBlockHeader(format!(
                    "Header named_files ({}) doesn't match actual ({})",
                    header.named_files(),
                    actual_named
                )));
            }
        }

        Ok(())
    }

    /// Get file format summary
    pub fn summary(&self) -> String {
        format!(
            "Root File {}: {} blocks, {} total files, {} named files",
            self.version,
            self.num_blocks(),
            self.total_files(),
            self.named_files()
        )
    }
}

impl crate::CascFormat for RootFile {
    fn parse(data: &[u8]) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Use builder for round-trip building
        let mut builder = crate::root::builder::RootBuilder::new(self.version);

        // Add all records
        for block in &self.blocks {
            for record in &block.records {
                builder.add_file_in_block(
                    record.file_data_id,
                    record.content_key,
                    record.name_hash,
                    block.locale_flags(),
                    block.content_flags(),
                );
            }
        }

        builder
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{CascFormat, root::builder::RootBuilder};

    fn create_test_root(version: RootVersion) -> RootFile {
        let mut builder = RootBuilder::new(version);

        // Add some test files
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

        // Add file without name hash
        builder.add_file(
            FileDataId::new(300),
            ContentKey::from_hex("abcdefabcdefabcdefabcdefabcdefab")
                .expect("Operation should succeed"),
            None,
            LocaleFlags::new(LocaleFlags::ALL),
            ContentFlags::new(ContentFlags::INSTALL | ContentFlags::NO_NAME_HASH),
        );

        let data = builder.build().expect("Operation should succeed");
        RootFile::parse(&data).expect("Operation should succeed")
    }

    #[test]
    fn test_parse_v1_root() {
        let root = create_test_root(RootVersion::V1);

        assert_eq!(root.version, RootVersion::V1);
        assert!(root.header.is_none());
        assert!(!root.blocks.is_empty());
        assert!(root.total_files() >= 3);
    }

    #[test]
    fn test_parse_v2_root() {
        let root = create_test_root(RootVersion::V2);

        assert_eq!(root.version, RootVersion::V2);
        assert!(root.header.is_some());
        assert!(!root.blocks.is_empty());
        assert!(root.total_files() >= 3);
    }

    #[test]
    fn test_parse_v3_root() {
        let root = create_test_root(RootVersion::V3);

        assert_eq!(root.version, RootVersion::V3);
        assert!(root.header.is_some());
        assert!(!root.blocks.is_empty());
        assert!(root.total_files() >= 3);
    }

    #[test]
    fn test_parse_v4_root() {
        let root = create_test_root(RootVersion::V4);

        assert_eq!(root.version, RootVersion::V4);
        assert!(root.header.is_some());
        assert!(!root.blocks.is_empty());
        assert!(root.total_files() >= 3);
    }

    #[test]
    fn test_file_resolution_by_id() {
        let root = create_test_root(RootVersion::V2);

        // Should resolve existing file
        let resolved = root.resolve_by_id(
            FileDataId::new(100),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_some());

        // Should not resolve with wrong locale
        let resolved = root.resolve_by_id(
            FileDataId::new(100),
            LocaleFlags::new(LocaleFlags::FRFR),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_none());

        // Should not resolve non-existent file
        let resolved = root.resolve_by_id(
            FileDataId::new(999_999),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_none());
    }

    #[test]
    fn test_file_resolution_by_path() {
        let root = create_test_root(RootVersion::V2);

        // Should resolve existing file
        let resolved = root.resolve_by_path(
            "Interface\\Icons\\INV_Misc_QuestionMark.blp",
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_some());

        // Should resolve with normalized path
        let resolved = root.resolve_by_path(
            "interface/icons/inv_misc_questionmark.blp",
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_some());

        // Should not resolve non-existent path
        let resolved = root.resolve_by_path(
            "NonExistent\\Path\\File.blp",
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_none());
    }

    #[test]
    fn test_multi_locale_resolution() {
        let root = create_test_root(RootVersion::V2);

        // File 200 supports both ENUS and DEDE
        let resolved_enus = root.resolve_by_id(
            FileDataId::new(200),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        let resolved_dede = root.resolve_by_id(
            FileDataId::new(200),
            LocaleFlags::new(LocaleFlags::DEDE),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        assert!(resolved_enus.is_some());
        assert!(resolved_dede.is_some());
        assert_eq!(resolved_enus, resolved_dede); // Same content key
    }

    #[test]
    fn test_validate_structure() {
        let root = create_test_root(RootVersion::V2);
        assert!(root.validate().is_ok());
    }

    #[test]
    fn test_round_trip() {
        for version in [
            RootVersion::V1,
            RootVersion::V2,
            RootVersion::V3,
            RootVersion::V4,
        ] {
            let original = create_test_root(version);
            let built = original.build().expect("Operation should succeed");
            let restored = RootFile::parse(&built).expect("Operation should succeed");

            assert_eq!(original.version, restored.version);
            assert_eq!(original.total_files(), restored.total_files());
            assert_eq!(original.named_files(), restored.named_files());
            assert_eq!(original.num_blocks(), restored.num_blocks());

            // Test resolution still works
            let resolved1 = original.resolve_by_id(
                FileDataId::new(100),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );
            let resolved2 = restored.resolve_by_id(
                FileDataId::new(100),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );
            assert_eq!(resolved1, resolved2);
        }
    }

    #[test]
    fn test_lookup_stats() {
        let root = create_test_root(RootVersion::V2);
        let (fdid_count, name_count) = root.lookup_stats();

        assert!(fdid_count > 0);
        assert!(name_count > 0);
        assert!(name_count <= fdid_count); // Not all files have names
    }

    #[test]
    fn test_iterate_records() {
        let root = create_test_root(RootVersion::V2);

        let records: Vec<_> = root.iter_records().collect();
        assert!(records.len() >= 3);

        // Check we have the expected FileDataIDs
        let fdids: Vec<_> = records.iter().map(|r| r.file_data_id.get()).collect();
        assert!(fdids.contains(&100));
        assert!(fdids.contains(&200));
        assert!(fdids.contains(&300));
    }

    #[test]
    fn test_summary_string() {
        let root = create_test_root(RootVersion::V2);
        let summary = root.summary();

        assert!(summary.contains("Root File"));
        assert!(summary.contains("V2"));
        assert!(summary.contains("blocks"));
        assert!(summary.contains("files"));
    }
}

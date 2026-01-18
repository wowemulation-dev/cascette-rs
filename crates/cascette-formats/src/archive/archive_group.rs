//! Archive-Group Support for CDN Index Merging
//!
//! Archive-groups are client-generated mega-indices that combine multiple CDN archive
//! indices into a single searchable structure for performance optimization.
//!
//! Key differences from regular archive indices:
//! - Uses 6-byte offsets (2 bytes archive index + 4 bytes offset)
//! - Contains deduplicated entries from multiple archives
//! - Generated locally by the client, not downloaded from CDN
//! - Uses standard CDN index format but with modified offset field

use std::collections::HashMap;
use std::io::{Read, Seek, Write};

use crate::archive::{ArchiveIndex, ArchiveResult, IndexFooter};

/// Assign archive index using Battle.net's hash-based algorithm
///
/// This implements the deterministic assignment: `archive_index = hash(encoding_key) % 65536`
/// ensuring compatibility with Battle.net client behavior.
pub fn assign_archive_index(encoding_key: &[u8]) -> u16 {
    let hash = cascette_crypto::md5::ContentKey::from_data(encoding_key);
    u16::from_be_bytes([hash.as_bytes()[0], hash.as_bytes()[1]])
}

/// Entry in an archive-group with 6-byte composite offset
#[derive(Debug, Clone)]
pub struct ArchiveGroupEntry {
    /// Encoding key (typically 16 bytes)
    pub encoding_key: Vec<u8>,
    /// Archive index (which archive file contains this entry)
    pub archive_index: u16,
    /// Offset within that archive
    pub offset: u32,
    /// Size of the entry
    pub size: u32,
}

impl ArchiveGroupEntry {
    /// Create a new archive-group entry
    pub fn new(encoding_key: Vec<u8>, archive_index: u16, offset: u32, size: u32) -> Self {
        Self {
            encoding_key,
            archive_index,
            offset,
            size,
        }
    }

    /// Get the 6-byte combined offset value
    pub fn combined_offset(&self) -> [u8; 6] {
        let mut result = [0u8; 6];
        result[0..2].copy_from_slice(&self.archive_index.to_be_bytes());
        result[2..6].copy_from_slice(&self.offset.to_be_bytes());
        result
    }

    /// Parse a 6-byte combined offset
    pub fn parse_combined_offset(data: &[u8]) -> ArchiveResult<(u16, u32)> {
        if data.len() != 6 {
            return Err(crate::archive::ArchiveError::InvalidFormat(format!(
                "Expected 6-byte offset, got {}",
                data.len()
            )));
        }
        let archive_idx = u16::from_be_bytes([data[0], data[1]]);
        let offset = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
        Ok((archive_idx, offset))
    }
}

/// Archive-Group - a mega-index combining multiple CDN archive indices
#[derive(Debug, Clone)]
pub struct ArchiveGroup {
    /// All entries from all archives, deduplicated
    pub entries: Vec<ArchiveGroupEntry>,
    /// Footer metadata
    pub footer: IndexFooter,
}

impl ArchiveGroup {
    /// Parse an archive-group from a reader
    /// This uses the standard CDN index format but interprets 6-byte offsets
    pub fn parse<R: Read + Seek>(reader: &mut R) -> ArchiveResult<Self> {
        // Parse as a standard archive index - it now handles archive-groups properly
        let index = ArchiveIndex::parse(reader)?;

        // Verify this is an archive-group (6-byte offsets)
        if !index.is_archive_group() {
            return Err(crate::archive::ArchiveError::InvalidFormat(format!(
                "Not an archive-group: offset_bytes = {}, expected 6",
                index.footer.offset_bytes
            )));
        }

        // Convert entries to archive-group format
        // The index parser now properly sets archive_index for archive-groups
        let mut entries = Vec::with_capacity(index.entries.len());
        for entry in index.entries {
            // For archive-groups, the parser has already split the 6-byte offset
            let archive_index = entry.archive_index.unwrap_or(0);

            entries.push(ArchiveGroupEntry {
                encoding_key: entry.encoding_key,
                archive_index,
                offset: entry.offset as u32,
                size: entry.size,
            });
        }

        Ok(ArchiveGroup {
            entries,
            footer: index.footer,
        })
    }

    /// Find an entry by encoding key
    pub fn find_entry(&self, encoding_key: &[u8]) -> Option<&ArchiveGroupEntry> {
        self.entries
            .binary_search_by(|e| e.encoding_key.as_slice().cmp(encoding_key))
            .ok()
            .map(|idx| &self.entries[idx])
    }
}

/// Builder for creating archive-groups from multiple archive indices
pub struct ArchiveGroupBuilder {
    /// Map of encoding keys to entries (for deduplication)
    entries: HashMap<Vec<u8>, ArchiveGroupEntry>,
}

impl ArchiveGroupBuilder {
    /// Create a new archive-group builder
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add all entries from an archive index
    ///
    /// # Arguments
    /// * `archive_index` - The index number of this archive (0-based)
    /// * `index` - The archive index to add entries from
    pub fn add_archive(&mut self, archive_index: u16, index: &ArchiveIndex) -> &mut Self {
        for entry in &index.entries {
            // Only add if not already present (deduplication)
            self.entries
                .entry(entry.encoding_key.clone())
                .or_insert_with(|| {
                    ArchiveGroupEntry::new(
                        entry.encoding_key.clone(),
                        archive_index,
                        entry.offset as u32,
                        entry.size,
                    )
                });
        }
        self
    }

    /// Add a single entry
    pub fn add_entry(&mut self, entry: ArchiveGroupEntry) -> &mut Self {
        self.entries
            .entry(entry.encoding_key.clone())
            .or_insert(entry);
        self
    }

    /// Add an entry using hash-based archive index assignment
    ///
    /// This method uses the Battle.net compatible algorithm to determine
    /// the archive index automatically from the encoding key.
    pub fn add_entry_with_hash_assignment(
        &mut self,
        encoding_key: Vec<u8>,
        offset: u32,
        size: u32,
    ) -> &mut Self {
        let archive_index = assign_archive_index(&encoding_key);
        self.add_entry(ArchiveGroupEntry::new(
            encoding_key,
            archive_index,
            offset,
            size,
        ))
    }

    /// Build the archive-group and write to a writer
    pub fn build<W: Write + Seek>(self, mut writer: W) -> ArchiveResult<ArchiveGroup> {
        // Convert HashMap to sorted Vec
        let mut entries: Vec<ArchiveGroupEntry> = self.entries.into_values().collect();
        entries.sort_by(|a, b| a.encoding_key.cmp(&b.encoding_key));

        // Calculate data size for chunking
        let entry_count = entries.len();
        let bytes_per_entry = 16 + 6 + 4; // key + offset + size
        let total_data_size = entry_count * bytes_per_entry;
        let chunk_count = total_data_size.div_ceil(0x1000); // 4KB chunks

        // Write entries in chunks (archive-groups don't use TOC)
        for chunk_idx in 0..chunk_count {
            let start_idx = chunk_idx * (0x1000 / bytes_per_entry);
            let end_idx = ((chunk_idx + 1) * (0x1000 / bytes_per_entry)).min(entry_count);

            // Prepare chunk data
            let mut chunk_data = Vec::with_capacity(0x1000);

            for entry in &entries[start_idx..end_idx] {
                // Write encoding key
                chunk_data.extend_from_slice(&entry.encoding_key);

                // Write 6-byte combined offset
                chunk_data.extend_from_slice(&entry.combined_offset());

                // Write size
                chunk_data.extend_from_slice(&entry.size.to_be_bytes());
            }

            // Pad chunk to 4KB
            chunk_data.resize(0x1000, 0);

            writer.write_all(&chunk_data)?;
        }

        // Write footer
        let footer = IndexFooter {
            toc_hash: [0; 8], // No TOC hash for archive-groups
            version: 1,       // Standard version
            reserved: [0, 0],
            page_size_kb: 4,
            offset_bytes: 6, // 6-byte offsets for archive-groups!
            size_bytes: 4,
            ekey_length: 16,
            footer_hash_bytes: 8,
            element_count: entry_count as u32,
            footer_hash: vec![0; 8],
        };

        // Prepare footer bytes
        let mut footer_bytes = Vec::new();
        footer_bytes.extend_from_slice(&footer.toc_hash);
        footer_bytes.push(footer.version);
        footer_bytes.extend_from_slice(&footer.reserved);
        footer_bytes.push(footer.page_size_kb);
        footer_bytes.push(footer.offset_bytes);
        footer_bytes.push(footer.size_bytes);
        footer_bytes.push(footer.ekey_length);
        footer_bytes.push(footer.footer_hash_bytes);
        footer_bytes.extend_from_slice(&footer.element_count.to_le_bytes());

        // Calculate MD5 hash of footer (excluding the hash itself)
        let content_key = cascette_crypto::md5::ContentKey::from_data(&footer_bytes);
        let footer_hash = content_key.as_bytes()[..8].to_vec();
        footer_bytes.extend_from_slice(&footer_hash);

        writer.write_all(&footer_bytes)?;

        Ok(ArchiveGroup { entries, footer })
    }
}

impl Default for ArchiveGroupBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_combined_offset() {
        let entry = ArchiveGroupEntry::new(
            vec![0x12, 0x34],
            0x0042,      // Archive index 66
            0x1234_5678, // Offset
            1024,        // Size
        );

        let combined = entry.combined_offset();
        assert_eq!(combined.len(), 6);
        assert_eq!(combined, [0x00, 0x42, 0x12, 0x34, 0x56, 0x78]);

        let (idx, offset) =
            ArchiveGroupEntry::parse_combined_offset(&combined).expect("Valid offset parsing");
        assert_eq!(idx, 0x0042);
        assert_eq!(offset, 0x1234_5678);
    }

    #[test]
    fn test_deduplication() {
        let mut builder = ArchiveGroupBuilder::new();

        // Add duplicate entries
        builder.add_entry(ArchiveGroupEntry::new(vec![0xAA, 0xBB], 1, 100, 50));

        builder.add_entry(ArchiveGroupEntry::new(
            vec![0xAA, 0xBB], // Same key
            2,
            200,
            50,
        ));

        // Should keep only first occurrence
        assert_eq!(builder.entries.len(), 1);
        let entry = builder
            .entries
            .get(&vec![0xAA, 0xBB])
            .expect("Entry should exist");
        assert_eq!(entry.archive_index, 1); // First one wins
    }

    #[test]
    fn test_hash_based_assignment() {
        // Test the hash-based assignment algorithm
        let test_key = vec![
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0,
        ];

        let archive_index = assign_archive_index(&test_key);

        // Verify it's deterministic (same key gives same result)
        let archive_index2 = assign_archive_index(&test_key);
        assert_eq!(archive_index, archive_index2);

        // Verify it's within valid range (u16 max is 65535)
        // archive_index is u16, so this is always true, but demonstrates the range

        // Test with builder method
        let mut builder = ArchiveGroupBuilder::new();
        builder.add_entry_with_hash_assignment(test_key.clone(), 100, 1024);

        assert_eq!(builder.entries.len(), 1);
        let entry = builder.entries.get(&test_key).expect("Entry should exist");
        assert_eq!(entry.archive_index, archive_index);
        assert_eq!(entry.offset, 100);
        assert_eq!(entry.size, 1024);
    }

    #[test]
    fn test_hash_distribution() {
        // Test that different keys produce different archive indices
        let key1 = vec![0x00; 16];
        let key2 = vec![0xFF; 16];
        let key3 = vec![0xAA; 16];

        let idx1 = assign_archive_index(&key1);
        let idx2 = assign_archive_index(&key2);
        let idx3 = assign_archive_index(&key3);

        // These should be different (though not guaranteed, it's highly likely)
        assert_ne!(idx1, idx2);
        assert_ne!(idx1, idx3);
        assert_ne!(idx2, idx3);
    }
}

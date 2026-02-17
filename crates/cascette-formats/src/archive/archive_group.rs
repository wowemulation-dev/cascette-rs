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

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
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

        // Write entries in chunks, collecting last keys for TOC and block hashes
        let entries_per_chunk = 0x1000 / bytes_per_entry;
        let hash_bytes: u8 = 8;
        let mut toc_keys: Vec<Vec<u8>> = Vec::with_capacity(chunk_count);
        let mut block_hashes: Vec<Vec<u8>> = Vec::with_capacity(chunk_count);

        for chunk_idx in 0..chunk_count {
            let start_idx = chunk_idx * entries_per_chunk;
            let end_idx = ((chunk_idx + 1) * entries_per_chunk).min(entry_count);

            // Prepare chunk data
            let mut chunk_data = Vec::with_capacity(0x1000);

            for entry in &entries[start_idx..end_idx] {
                chunk_data.extend_from_slice(&entry.encoding_key);
                chunk_data.extend_from_slice(&entry.size.to_be_bytes());
                chunk_data.extend_from_slice(&entry.combined_offset());
            }

            // Pad chunk to 4KB
            chunk_data.resize(0x1000, 0);

            // Record last key of each chunk for TOC
            if end_idx > start_idx {
                toc_keys.push(entries[end_idx - 1].encoding_key.clone());
            }

            // Compute block hash
            block_hashes.push(super::index::calculate_block_hash(&chunk_data, hash_bytes));

            writer.write_all(&chunk_data)?;
        }

        // Write TOC: keys then block hashes
        for key in &toc_keys {
            // Pad to 16 bytes (ekey_length)
            let mut padded = vec![0u8; 16];
            let copy_len = key.len().min(16);
            padded[..copy_len].copy_from_slice(&key[..copy_len]);
            writer.write_all(&padded)?;
        }

        for block_hash in &block_hashes {
            writer.write_all(block_hash)?;
        }

        // Compute TOC hash using padded keys
        let padded_toc_keys: Vec<Vec<u8>> = toc_keys
            .iter()
            .map(|key| {
                let mut padded = vec![0u8; 16];
                let copy_len = key.len().min(16);
                padded[..copy_len].copy_from_slice(&key[..copy_len]);
                padded
            })
            .collect();
        let toc_hash =
            super::index::calculate_toc_hash(&padded_toc_keys, &block_hashes, hash_bytes);

        // Create footer
        let mut footer = IndexFooter {
            toc_hash: {
                let mut arr = [0u8; 8];
                let copy_len = toc_hash.len().min(8);
                arr[..copy_len].copy_from_slice(&toc_hash[..copy_len]);
                arr
            },
            version: 1,
            reserved: [0, 0],
            page_size_kb: 4,
            offset_bytes: 6, // 6-byte offsets for archive-groups
            size_bytes: 4,
            ekey_length: 16,
            footer_hash_bytes: hash_bytes,
            element_count: entry_count as u32,
            footer_hash: vec![0; hash_bytes as usize],
        };
        footer.footer_hash = footer.calculate_footer_hash();

        footer.write(&mut writer)?;

        Ok(ArchiveGroup { entries, footer })
    }
}

impl Default for ArchiveGroupBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in the min-heap for k-way merge.
///
/// Each entry holds a reference to a key from one of the source archives,
/// along with the metadata needed to write an archive group entry and to
/// advance the cursor within that source archive.
struct HeapEntry<'a> {
    /// Encoding key (borrowed from source ArchiveIndex entry)
    key: &'a [u8],
    /// Archive number for the 6-byte composite offset
    archive_idx: u16,
    /// Offset within that archive
    offset: u32,
    /// Entry size
    size: u32,
    /// Index into the `archives` slice (identifies which source)
    source_idx: usize,
    /// Current position in that archive's entries (for advancing)
    cursor: usize,
}

impl PartialEq for HeapEntry<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for HeapEntry<'_> {}

impl PartialOrd for HeapEntry<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// BinaryHeap is a max-heap; we want min-heap behavior (smallest key first).
// Reverse the comparison so the smallest key has the highest priority.
impl Ord for HeapEntry<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .key
            .cmp(self.key)
            .then_with(|| other.source_idx.cmp(&self.source_idx))
    }
}

/// K-way merge of pre-sorted archive indices into an archive group.
///
/// Each `ArchiveIndex` must have entries sorted by encoding key (format
/// invariant). Duplicate keys across archives are deduplicated, keeping the
/// first occurrence (lowest `source_idx`).
///
/// This matches Agent.exe `tact::CdnIndex::BuildMergedIndex` which uses a
/// binary min-heap for O(N log K) merging where K is the number of indices.
pub fn build_merged<W: Write + Seek>(
    archives: &[(u16, &ArchiveIndex)],
    mut writer: W,
) -> ArchiveResult<ArchiveGroup> {
    let hash_bytes: u8 = 8;
    let ekey_length: u8 = 16;
    let bytes_per_entry = ekey_length as usize + 6 + 4; // key + 6-byte offset + 4-byte size
    let entries_per_chunk = 0x1000 / bytes_per_entry;

    // Initialize heap with the first entry from each non-empty archive
    let mut heap = BinaryHeap::with_capacity(archives.len());
    for (source_idx, (archive_idx, index)) in archives.iter().enumerate() {
        if let Some(entry) = index.entries.first() {
            heap.push(HeapEntry {
                key: &entry.encoding_key,
                archive_idx: *archive_idx,
                offset: entry.offset as u32,
                size: entry.size,
                source_idx,
                cursor: 0,
            });
        }
    }

    let mut entries = Vec::new();
    let mut toc_keys: Vec<Vec<u8>> = Vec::new();
    let mut block_hashes: Vec<Vec<u8>> = Vec::new();
    let mut chunk_data = Vec::with_capacity(0x1000);
    let mut chunk_entry_count = 0usize;
    let mut prev_key: Option<Vec<u8>> = None;

    while let Some(entry) = heap.pop() {
        // Advance the source cursor and push the next entry from the same source
        let next_cursor = entry.cursor + 1;
        let (src_archive_idx, src_index) = &archives[entry.source_idx];
        if next_cursor < src_index.entries.len() {
            let next = &src_index.entries[next_cursor];
            heap.push(HeapEntry {
                key: &next.encoding_key,
                archive_idx: *src_archive_idx,
                offset: next.offset as u32,
                size: next.size,
                source_idx: entry.source_idx,
                cursor: next_cursor,
            });
        }

        // Deduplication: skip if key matches previous output
        if let Some(ref prev) = prev_key
            && prev.as_slice() == entry.key
        {
            continue;
        }

        // Write entry to chunk buffer
        let group_entry = ArchiveGroupEntry::new(
            entry.key.to_vec(),
            entry.archive_idx,
            entry.offset,
            entry.size,
        );

        chunk_data.extend_from_slice(&group_entry.encoding_key);
        chunk_data.extend_from_slice(&group_entry.size.to_be_bytes());
        chunk_data.extend_from_slice(&group_entry.combined_offset());
        chunk_entry_count += 1;

        prev_key = Some(entry.key.to_vec());
        entries.push(group_entry);

        // Flush chunk when full
        if chunk_entry_count == entries_per_chunk {
            chunk_data.resize(0x1000, 0);
            toc_keys.push(
                entries
                    .last()
                    .map(|e| e.encoding_key.clone())
                    .unwrap_or_default(),
            );
            block_hashes.push(super::index::calculate_block_hash(&chunk_data, hash_bytes));
            writer.write_all(&chunk_data)?;
            chunk_data.clear();
            chunk_entry_count = 0;
        }
    }

    // Flush final partial chunk (if any entries remain)
    if chunk_entry_count > 0 {
        chunk_data.resize(0x1000, 0);
        toc_keys.push(
            entries
                .last()
                .map(|e| e.encoding_key.clone())
                .unwrap_or_default(),
        );
        block_hashes.push(super::index::calculate_block_hash(&chunk_data, hash_bytes));
        writer.write_all(&chunk_data)?;
    }

    // Write TOC: keys then block hashes
    let padded_toc_keys: Vec<Vec<u8>> = toc_keys
        .iter()
        .map(|key| {
            let mut padded = vec![0u8; ekey_length as usize];
            let copy_len = key.len().min(ekey_length as usize);
            padded[..copy_len].copy_from_slice(&key[..copy_len]);
            padded
        })
        .collect();

    for padded_key in &padded_toc_keys {
        writer.write_all(padded_key)?;
    }
    for block_hash in &block_hashes {
        writer.write_all(block_hash)?;
    }

    // Compute TOC hash and build footer
    let toc_hash = super::index::calculate_toc_hash(&padded_toc_keys, &block_hashes, hash_bytes);

    let mut footer = IndexFooter {
        toc_hash: {
            let mut arr = [0u8; 8];
            let copy_len = toc_hash.len().min(8);
            arr[..copy_len].copy_from_slice(&toc_hash[..copy_len]);
            arr
        },
        version: 1,
        reserved: [0, 0],
        page_size_kb: 4,
        offset_bytes: 6,
        size_bytes: 4,
        ekey_length,
        footer_hash_bytes: hash_bytes,
        element_count: entries.len() as u32,
        footer_hash: vec![0; hash_bytes as usize],
    };
    footer.footer_hash = footer.calculate_footer_hash();
    footer.write(&mut writer)?;

    Ok(ArchiveGroup { entries, footer })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::archive::ArchiveIndexBuilder;

    /// Helper: build an ArchiveIndex with the given (key, offset, size) entries.
    /// Entries are sorted by key (format invariant).
    fn make_index(entries: &[(&[u8], u64, u32)]) -> ArchiveIndex {
        let mut builder = ArchiveIndexBuilder::new();
        for &(key, offset, size) in entries {
            builder.add_entry(key.to_vec(), size, offset);
        }
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        builder.build(&mut cursor).expect("build index")
    }

    #[test]
    fn test_build_merged_equivalence() {
        // Build archive group via both paths and assert byte-identical output.
        let key_a = [0x01u8; 16];
        let key_b = [0x02u8; 16];
        let key_c = [0x03u8; 16];

        let idx0 = make_index(&[(&key_a, 0, 100), (&key_c, 200, 300)]);
        let idx1 = make_index(&[(&key_b, 50, 150)]);

        // Path 1: ArchiveGroupBuilder
        let mut builder = ArchiveGroupBuilder::new();
        builder.add_archive(0, &idx0);
        builder.add_archive(1, &idx1);
        let mut buf_builder = Vec::new();
        let _group_builder = builder
            .build(std::io::Cursor::new(&mut buf_builder))
            .expect("builder build");

        // Path 2: build_merged
        let mut buf_merged = Vec::new();
        let _group_merged = build_merged(
            &[(0, &idx0), (1, &idx1)],
            std::io::Cursor::new(&mut buf_merged),
        )
        .expect("merged build");

        assert_eq!(
            buf_builder, buf_merged,
            "Builder and build_merged must produce identical bytes"
        );
    }

    #[test]
    fn test_build_merged_deduplication() {
        // Same key in two archives: first occurrence (lower source_idx) wins.
        let key = [0xAA; 16];
        let idx0 = make_index(&[(&key, 100, 50)]);
        let idx1 = make_index(&[(&key, 200, 60)]);

        let mut buf = Vec::new();
        let group = build_merged(&[(10, &idx0), (20, &idx1)], std::io::Cursor::new(&mut buf))
            .expect("merged build");

        assert_eq!(group.entries.len(), 1);
        assert_eq!(group.entries[0].archive_index, 10); // first archive wins
        assert_eq!(group.entries[0].offset, 100);
        assert_eq!(group.entries[0].size, 50);
    }

    #[test]
    fn test_build_merged_empty() {
        let mut buf = Vec::new();
        let group = build_merged(&[], std::io::Cursor::new(&mut buf)).expect("merged build empty");

        assert_eq!(group.entries.len(), 0);
        assert_eq!(group.footer.element_count, 0);
        assert_eq!(group.footer.offset_bytes, 6);
    }

    #[test]
    fn test_build_merged_single_archive() {
        let key_a = [0x10; 16];
        let key_b = [0x20; 16];
        let idx = make_index(&[(&key_a, 0, 100), (&key_b, 100, 200)]);

        // build_merged with single archive
        let mut buf_merged = Vec::new();
        let group_merged = build_merged(&[(5, &idx)], std::io::Cursor::new(&mut buf_merged))
            .expect("merged build");

        // ArchiveGroupBuilder with single archive
        let mut builder = ArchiveGroupBuilder::new();
        builder.add_archive(5, &idx);
        let mut buf_builder = Vec::new();
        let _group_builder = builder
            .build(std::io::Cursor::new(&mut buf_builder))
            .expect("builder build");

        assert_eq!(group_merged.entries.len(), 2);
        assert_eq!(buf_builder, buf_merged);
    }

    #[test]
    fn test_build_merged_many_entries() {
        // Test with enough entries to span multiple 4KB chunks.
        // Each entry is 26 bytes, so 170 fit per chunk. Use 400 entries.
        let mut entries: Vec<([u8; 16], u64, u32)> = Vec::new();
        for i in 0u32..400 {
            let mut key = [0u8; 16];
            key[0..4].copy_from_slice(&i.to_be_bytes());
            entries.push((key, i as u64 * 100, (i + 1) * 10));
        }

        // Split into two archives
        let entries_a: Vec<(&[u8], u64, u32)> = entries[..200]
            .iter()
            .map(|(k, o, s)| (k.as_slice(), *o, *s))
            .collect();
        let entries_b: Vec<(&[u8], u64, u32)> = entries[200..]
            .iter()
            .map(|(k, o, s)| (k.as_slice(), *o, *s))
            .collect();

        let idx_a = make_index(&entries_a);
        let idx_b = make_index(&entries_b);

        // build_merged
        let mut buf_merged = Vec::new();
        let group = build_merged(
            &[(0, &idx_a), (1, &idx_b)],
            std::io::Cursor::new(&mut buf_merged),
        )
        .expect("merged build");

        assert_eq!(group.entries.len(), 400);
        assert_eq!(group.footer.element_count, 400);

        // Verify entries are sorted
        for w in group.entries.windows(2) {
            assert!(w[0].encoding_key < w[1].encoding_key);
        }

        // Compare with builder
        let mut builder = ArchiveGroupBuilder::new();
        builder.add_archive(0, &idx_a);
        builder.add_archive(1, &idx_b);
        let mut buf_builder = Vec::new();
        builder
            .build(std::io::Cursor::new(&mut buf_builder))
            .expect("builder build");

        assert_eq!(buf_builder, buf_merged);
    }

    #[test]
    fn test_build_merged_parseable() {
        // Verify the output of build_merged can be parsed back as an ArchiveGroup.
        let key_a = [0x11; 16];
        let key_b = [0x22; 16];
        let idx = make_index(&[(&key_a, 0, 100), (&key_b, 200, 300)]);

        let mut buf = Vec::new();
        let original =
            build_merged(&[(7, &idx)], std::io::Cursor::new(&mut buf)).expect("merged build");

        let parsed =
            ArchiveGroup::parse(&mut std::io::Cursor::new(&buf)).expect("parse merged output");

        assert_eq!(parsed.entries.len(), original.entries.len());
        for (orig, pars) in original.entries.iter().zip(parsed.entries.iter()) {
            assert_eq!(orig.encoding_key, pars.encoding_key);
            assert_eq!(orig.archive_index, pars.archive_index);
            assert_eq!(orig.offset, pars.offset);
            assert_eq!(orig.size, pars.size);
        }
    }

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

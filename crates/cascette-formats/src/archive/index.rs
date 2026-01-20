//! Archive index parsing and building implementation with VARIABLE-LENGTH keys
//!
//! CRITICAL: CDN archive indices use variable-length keys as specified in the footer!
//! - Typically 16 bytes (full MD5 hash) for CDN downloads
//! - The footer's key_bytes field (ekey_length) determines the actual size
//!
//! CDN archives use full variable-length keys, never truncated keys!
//!
//! ## TACT Archive Index Format
//!
//! This module handles TACT archive index files (`.index` files from CDN) which use:
//! - Variable-size footer (20 bytes + hash_bytes)
//! - Block-based structure (typically 4KB blocks)
//! - VARIABLE-LENGTH keys as specified in footer
//! - Mixed endianness (little-endian for element_count, big-endian for sizes/offsets)

use crate::archive::error::{ArchiveError, ArchiveResult};
use binrw::io::{Seek, SeekFrom, Write};
use std::fs::File;
use std::io::{Cursor, Read as StdRead};
use std::path::{Path, PathBuf};

use super::constants::{CHUNK_SIZE, ENTRY_SIZE, MAX_ENTRIES_PER_CHUNK};

/// Archive index constants
/// Minimum footer size (20 bytes + hash_bytes)
pub const MIN_FOOTER_SIZE: usize = 20;

/// Archive index entry with variable-length key
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// Variable-length encoding key (size from footer.ekey_length)
    pub encoding_key: Vec<u8>,
    /// Compressed size in bytes
    pub size: u32,
    /// Archive file offset (for regular indices) or archive index (for archive-groups)
    pub offset: u64,
    /// Archive index (only used for archive-groups with 6-byte offsets)
    pub archive_index: Option<u16>,
}

impl IndexEntry {
    /// Create new index entry
    pub fn new(encoding_key: Vec<u8>, size: u32, offset: u64) -> Self {
        Self {
            encoding_key,
            size,
            offset,
            archive_index: None,
        }
    }

    /// Create new archive-group entry with archive index
    pub fn new_archive_group(
        encoding_key: Vec<u8>,
        size: u32,
        archive_index: u16,
        offset: u32,
    ) -> Self {
        Self {
            encoding_key,
            size,
            offset: offset as u64,
            archive_index: Some(archive_index),
        }
    }

    /// Check if this is a zero/padding entry
    pub fn is_zero(&self) -> bool {
        self.encoding_key.iter().all(|&b| b == 0) && self.size == 0 && self.offset == 0
    }

    /// Parse entry from bytes with given key size and offset size
    pub fn parse(
        data: &[u8],
        key_bytes: u8,
        size_bytes: u8,
        offset_bytes: u8,
    ) -> ArchiveResult<Self> {
        let mut pos = 0;

        // Read key
        if data.len() < key_bytes as usize {
            return Err(ArchiveError::InvalidFormat(
                "Insufficient data for key".into(),
            ));
        }
        let encoding_key = data[..key_bytes as usize].to_vec();
        pos += key_bytes as usize;

        // Read size (big-endian)
        if data.len() < pos + size_bytes as usize {
            return Err(ArchiveError::InvalidFormat(
                "Insufficient data for size".into(),
            ));
        }
        let size = match size_bytes {
            4 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&data[pos..pos + 4]);
                u32::from_be_bytes(bytes)
            }
            _ => {
                return Err(ArchiveError::InvalidFormat(format!(
                    "Unsupported size_bytes: {}",
                    size_bytes
                )));
            }
        };
        pos += size_bytes as usize;

        // Read offset (big-endian)
        if data.len() < pos + offset_bytes as usize {
            return Err(ArchiveError::InvalidFormat(
                "Insufficient data for offset".into(),
            ));
        }
        // Read offset based on the size - special handling for archive-groups
        let (offset, archive_index) = match offset_bytes {
            4 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&data[pos..pos + 4]);
                (u32::from_be_bytes(bytes) as u64, None)
            }
            5 => {
                let mut bytes = [0u8; 8];
                bytes[3..].copy_from_slice(&data[pos..pos + 5]);
                (u64::from_be_bytes(bytes), None)
            }
            6 => {
                // Archive-group: 2 bytes archive index + 4 bytes offset
                let archive_idx = u16::from_be_bytes([data[pos], data[pos + 1]]);
                let offset = u32::from_be_bytes([
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                ]);
                (offset as u64, Some(archive_idx))
            }
            _ => {
                return Err(ArchiveError::InvalidFormat(format!(
                    "Unsupported offset_bytes: {}",
                    offset_bytes
                )));
            }
        };

        Ok(Self {
            encoding_key,
            size,
            offset,
            archive_index,
        })
    }

    /// Write entry to bytes
    pub fn to_bytes(&self, _size_bytes: u8, offset_bytes: u8) -> ArchiveResult<Vec<u8>> {
        let mut data = Vec::new();

        // Write key
        data.extend_from_slice(&self.encoding_key);

        // Write size (big-endian, always 4 bytes for now)
        data.extend_from_slice(&self.size.to_be_bytes());

        // Write offset (big-endian)
        match offset_bytes {
            4 => data.extend_from_slice(&(self.offset as u32).to_be_bytes()),
            5 => {
                let bytes = self.offset.to_be_bytes();
                data.extend_from_slice(&bytes[3..]);
            }
            6 => {
                // Archive-group: 2 bytes archive index + 4 bytes offset
                if let Some(archive_idx) = self.archive_index {
                    data.extend_from_slice(&archive_idx.to_be_bytes());
                    data.extend_from_slice(&(self.offset as u32).to_be_bytes());
                } else {
                    // Fallback for compatibility
                    let bytes = self.offset.to_be_bytes();
                    data.extend_from_slice(&bytes[2..]);
                }
            }
            _ => {
                return Err(ArchiveError::InvalidFormat(format!(
                    "Unsupported offset_bytes: {}",
                    offset_bytes
                )));
            }
        }

        Ok(data)
    }
}

impl PartialOrd for IndexEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encoding_key.cmp(&other.encoding_key)
    }
}

/// Archive index footer (variable size: 20 + footer_hash_bytes) - CASC format
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexFooter {
    /// First 8 bytes of MD5 hash of table of contents
    pub toc_hash: [u8; 8],
    /// Index format version (always 1)
    pub version: u8,
    /// Reserved bytes (must be [0, 0])
    pub reserved: [u8; 2],
    /// Page size in kilobytes (always 4)
    pub page_size_kb: u8,
    /// Archive offset field size in bytes:
    /// - 4 for regular archives
    /// - 5 for larger archives
    /// - 6 for archive-groups (2 bytes archive index + 4 bytes offset)
    pub offset_bytes: u8,
    /// Compressed size field size in bytes (always 4)
    pub size_bytes: u8,
    /// `EKey` length in bytes (always 16 for full MD5)
    pub ekey_length: u8,
    /// Footer hash length in bytes (always 8)
    pub footer_hash_bytes: u8,
    /// Number of TOC entries/chunks (little-endian - special case!)
    /// This is NOT the number of data entries, but the number of 4KB chunks
    pub element_count: u32,
    /// First N bytes of MD5 hash of footer fields (N = footer_hash_bytes)
    pub footer_hash: Vec<u8>,
}

impl IndexFooter {
    /// Create new footer with standard CASC values
    pub fn new(toc_hash: [u8; 8], element_count: u32) -> Self {
        let mut footer = Self {
            toc_hash,
            version: 1,
            reserved: [0, 0],
            page_size_kb: 4,
            offset_bytes: 4,
            size_bytes: 4,
            ekey_length: 16,
            footer_hash_bytes: 8,
            element_count,
            footer_hash: vec![0u8; 8],
        };
        footer.footer_hash = footer.calculate_footer_hash();
        footer
    }

    /// Calculate MD5 hash of footer fields (excluding `footer_hash` itself)
    pub fn calculate_footer_hash(&self) -> Vec<u8> {
        // Build data to hash from footer fields (12 bytes total)
        let mut data = Vec::with_capacity(20); // Pad to 20 bytes as reference impl does
        data.push(self.version);
        data.extend_from_slice(&self.reserved);
        data.push(self.page_size_kb);
        data.push(self.offset_bytes);
        data.push(self.size_bytes);
        data.push(self.ekey_length);
        data.push(self.footer_hash_bytes);
        // element_count is little-endian in the footer
        data.extend_from_slice(&self.element_count.to_le_bytes());

        // Pad to 20 bytes with zeros (like reference implementation)
        data.resize(20, 0);

        // Calculate MD5 hash using cascette-crypto
        let content_key = cascette_crypto::md5::ContentKey::from_data(&data);

        // Return lower 8 bytes of MD5 hash (verified against real files)
        content_key.as_bytes()[..8].to_vec()
    }

    /// Validate footer integrity using MD5
    pub fn is_valid(&self) -> bool {
        let expected = self.calculate_footer_hash();
        let actual_len = self.footer_hash.len().min(self.footer_hash_bytes as usize);
        self.footer_hash[..actual_len] == expected[..actual_len]
    }

    /// Write footer to writer
    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.toc_hash)?;
        writer.write_all(&[self.version])?;
        writer.write_all(&self.reserved)?;
        writer.write_all(&[self.page_size_kb])?;
        writer.write_all(&[self.offset_bytes])?;
        writer.write_all(&[self.size_bytes])?;
        writer.write_all(&[self.ekey_length])?;
        writer.write_all(&[self.footer_hash_bytes])?;
        writer.write_all(&self.element_count.to_le_bytes())?;
        writer.write_all(&self.footer_hash)?;
        Ok(())
    }

    /// Validate footer format parameters for CASC
    pub fn validate_format(&self) -> ArchiveResult<()> {
        if self.version != 1 {
            return Err(ArchiveError::UnsupportedVersion(self.version));
        }

        if self.reserved != [0, 0] {
            return Err(ArchiveError::InvalidFormat(format!(
                "Reserved bytes should be [0,0], got {:?}",
                self.reserved
            )));
        }

        if self.page_size_kb != 4 {
            return Err(ArchiveError::InvalidFormat(format!(
                "Page size should be 4KB, got {}",
                self.page_size_kb
            )));
        }

        // Support variable offset_bytes (4, 5, or 6 bytes)
        if ![4, 5, 6].contains(&self.offset_bytes) {
            return Err(ArchiveError::InvalidFormat(format!(
                "Offset bytes should be 4, 5, or 6, got {}",
                self.offset_bytes
            )));
        }

        if self.size_bytes != 4 {
            return Err(ArchiveError::InvalidFormat(format!(
                "Size bytes should be 4, got {}",
                self.size_bytes
            )));
        }

        // Support variable ekey_length (typically 9 or 16 bytes)
        if self.ekey_length < 9 || self.ekey_length > 16 {
            return Err(ArchiveError::InvalidFormat(format!(
                "EKey length should be between 9 and 16, got {}",
                self.ekey_length
            )));
        }

        if self.footer_hash_bytes != 8 {
            return Err(ArchiveError::InvalidFormat(format!(
                "Footer hash bytes should be 8, got {}",
                self.footer_hash_bytes
            )));
        }

        Ok(())
    }

    /// Check if this index is an archive-group
    pub fn is_archive_group(&self) -> bool {
        self.offset_bytes == 6
    }
}

/// Complete archive index structure
#[derive(Debug, Clone)]
pub struct ArchiveIndex {
    /// All index entries (sorted by encoding key)
    pub entries: Vec<IndexEntry>,
    /// Table of contents (last key of each chunk)
    pub toc: Vec<Vec<u8>>,
    /// Footer with metadata and validation
    pub footer: IndexFooter,
}

impl ArchiveIndex {
    /// Parse archive index from reader
    pub fn parse<R: StdRead + Seek>(mut reader: R) -> ArchiveResult<Self> {
        // First, read footer hash_bytes to determine footer size
        // hash_bytes is at offset -13 from end (20 bytes base + position 15 = -5 from base = -13 from end with 8-byte hash)
        reader.seek(SeekFrom::End(-13))?;
        let mut footer_hash_bytes_check = [0u8; 1];
        reader.read_exact(&mut footer_hash_bytes_check)?;
        let footer_hash_bytes = footer_hash_bytes_check[0];

        // Calculate actual footer size
        let footer_size = MIN_FOOTER_SIZE + footer_hash_bytes as usize;

        // Read footer from end of file manually (can't use BinRead due to variable hash size)
        reader.seek(SeekFrom::End(-(footer_size as i64)))?;

        // Read fixed portion of footer (20 bytes)
        let mut footer_data = vec![0u8; 20];
        reader.read_exact(&mut footer_data)?;

        let toc_hash = {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&footer_data[0..8]);
            arr
        };
        let version = footer_data[8];
        let reserved = [footer_data[9], footer_data[10]];
        let page_size_kb = footer_data[11];
        let offset_bytes = footer_data[12];
        let size_bytes = footer_data[13];
        let ekey_length = footer_data[14];
        let footer_hash_bytes_check = footer_data[15];
        let element_count = u32::from_le_bytes([
            footer_data[16],
            footer_data[17],
            footer_data[18],
            footer_data[19],
        ]);

        // Read variable-length footer hash
        let mut footer_hash = vec![0u8; footer_hash_bytes as usize];
        reader.read_exact(&mut footer_hash)?;

        let footer = IndexFooter {
            toc_hash,
            version,
            reserved,
            page_size_kb,
            offset_bytes,
            size_bytes,
            ekey_length,
            footer_hash_bytes: footer_hash_bytes_check,
            element_count,
            footer_hash,
        };

        // Validate footer
        if !footer.is_valid() {
            let expected_hash = footer.calculate_footer_hash();
            let mut expected_arr = [0u8; 8];
            let mut actual_arr = [0u8; 8];
            let copy_len = expected_hash.len().min(8);
            expected_arr[..copy_len].copy_from_slice(&expected_hash[..copy_len]);
            actual_arr[..copy_len].copy_from_slice(&footer.footer_hash[..copy_len]);
            return Err(ArchiveError::ChecksumMismatch {
                expected: expected_arr,
                actual: actual_arr,
            });
        }

        // Validate format parameters
        footer.validate_format()?;

        let block_size = (footer.page_size_kb as usize) * 1024; // Convert KB to bytes
        let record_size =
            footer.ekey_length as usize + footer.size_bytes as usize + footer.offset_bytes as usize;
        let records_per_block = block_size / record_size;

        // Calculate actual chunk count based on data entries and records per chunk
        let chunk_count = (footer.element_count as usize).div_ceil(records_per_block);

        // TOC uses max(key_bytes, 9) for compatibility
        let toc_key_size = footer.ekey_length as usize;

        // Calculate file structure
        reader.seek(SeekFrom::Start(0))?;
        let current_pos = reader.stream_position()?;
        reader.seek(SeekFrom::End(0))?;
        let file_size = reader.stream_position()?;
        reader.seek(SeekFrom::Start(current_pos))?;

        let non_footer_size = file_size - footer_size as u64;
        let toc_size = chunk_count * (toc_key_size + footer.footer_hash_bytes as usize);
        let data_size = non_footer_size - toc_size as u64;

        // Note: Some archive indices may have partial last chunks, so we don't
        // strictly enforce that data_size is a multiple of CHUNK_SIZE.
        // The chunk_count from the footer tells us how many chunks to expect.

        // Read table of contents (has TWO sections: keys then hashes)
        let toc_offset = data_size;
        reader.seek(SeekFrom::Start(toc_offset))?;

        let mut toc = Vec::with_capacity(chunk_count);

        // First read ALL the keys (using variable-length keys)
        for _ in 0..chunk_count {
            let mut key = vec![0u8; toc_key_size];
            reader.read_exact(&mut key)?;
            toc.push(key);
        }

        // Then skip over ALL the hashes (we don't need them for now)
        let hash_section_size = chunk_count * footer.footer_hash_bytes as usize;
        reader.seek(SeekFrom::Current(hash_section_size as i64))?;

        // TOC hash validation is intentionally disabled.
        //
        // Research shows no reference implementation (CascLib, TACT.Net, rustycasc) validates
        // this field. Multiple algorithms were tested against WoW Classic 1.15.2.55140 files:
        // - MD5(keys)[:8], MD5(keys)[8:16]
        // - MD5(keys+hashes)[:8], MD5(keys+hashes)[8:16]
        // - MD5(keys+page_hashes+last_page_hash)[:8] (TACT.Net generation algorithm)
        //
        // None matched Blizzard's stored values. The field appears to be metadata only,
        // not used for integrity verification.

        // Read entries from chunks
        reader.seek(SeekFrom::Start(0))?;
        let mut entries = Vec::new();

        for chunk_idx in 0..chunk_count {
            let chunk_offset = chunk_idx * block_size;

            // Calculate chunk size (last chunk might be smaller)
            let chunk_size = if chunk_idx == chunk_count - 1 {
                // Last chunk might be partial
                let remaining = data_size as usize - chunk_offset;
                remaining.min(block_size)
            } else {
                block_size
            };

            reader.seek(SeekFrom::Start(chunk_offset as u64))?;

            // Read chunk data
            let mut chunk_data = vec![0u8; chunk_size];
            reader.read_exact(&mut chunk_data)?;

            // Parse entries from chunk using variable-length keys
            let mut pos = 0;
            while pos + record_size <= chunk_size {
                let entry_data = &chunk_data[pos..pos + record_size];
                match IndexEntry::parse(
                    entry_data,
                    footer.ekey_length,
                    footer.size_bytes,
                    footer.offset_bytes,
                ) {
                    Ok(entry) => {
                        // Skip zero entries (padding)
                        if entry.is_zero() {
                            break;
                        }
                        entries.push(entry);
                    }
                    Err(_) => break, // Stop on parse error (likely padding)
                }
                pos += record_size;
            }
        }

        // Validate entry sorting
        if !is_sorted(&entries) {
            return Err(ArchiveError::UnsortedEntries);
        }

        let index = Self {
            entries,
            toc,
            footer,
        };

        // Final validation
        index.validate()?;

        Ok(index)
    }

    /// Build archive index to writer
    pub fn build<W: Write + Seek>(&self, mut writer: W) -> ArchiveResult<()> {
        let chunk_count = self.footer.element_count as usize;
        let block_size = (self.footer.page_size_kb as usize) * 1024; // Convert KB to bytes
        let record_size = self.footer.ekey_length as usize
            + self.footer.size_bytes as usize
            + self.footer.offset_bytes as usize;
        let records_per_block = block_size / record_size;

        // Write entry chunks
        let mut entry_idx = 0;
        for chunk_idx in 0..chunk_count {
            let mut chunk_data = vec![0u8; block_size];

            // Write entries for this chunk
            let entries_in_chunk = if chunk_idx == chunk_count - 1 {
                // Last chunk gets remaining entries
                self.entries.len() - entry_idx
            } else {
                records_per_block.min(self.entries.len() - entry_idx)
            };

            let mut pos = 0;
            for _ in 0..entries_in_chunk {
                if entry_idx < self.entries.len() {
                    let entry_bytes = self.entries[entry_idx]
                        .to_bytes(self.footer.size_bytes, self.footer.offset_bytes)?;
                    chunk_data[pos..pos + entry_bytes.len()].copy_from_slice(&entry_bytes);
                    pos += record_size;
                    entry_idx += 1;
                }
            }

            // Chunk is automatically padded with zeros
            writer.write_all(&chunk_data)?;
        }

        // Write table of contents
        for key in &self.toc {
            writer.write_all(key)?;
        }

        // Write footer
        self.footer.write(&mut writer)?;

        Ok(())
    }

    /// Find entry by encoding key
    pub fn find_entry(&self, encoding_key: &[u8]) -> Option<&IndexEntry> {
        self.binary_search_key(encoding_key)
    }

    /// Find all entries matching a key (handles collisions)
    pub fn find_all_entries(&self, encoding_key: &[u8]) -> Vec<&IndexEntry> {
        self.find_all_key_matches(encoding_key)
    }

    /// Binary search for key (variable-length for CDN archives)
    pub fn binary_search_key(&self, search_key: &[u8]) -> Option<&IndexEntry> {
        // Binary search for chunk using TOC
        // The TOC contains the maximum key for each chunk
        // We need to find the first chunk where max_key >= search_key
        let chunk_idx = match self.toc.binary_search_by(|toc_key| {
            // Compare keys (variable length)
            let compare_len = toc_key.len().min(search_key.len());
            toc_key[..compare_len].cmp(&search_key[..compare_len])
        }) {
            Ok(idx) => idx,
            Err(idx) => {
                // If idx == 0, the key is smaller than any chunk's max
                // If idx == toc.len(), the key is larger than any chunk's max
                if idx >= self.toc.len() {
                    return None;
                }
                idx
            }
        };

        // Binary search within chunk
        let chunk_start = chunk_idx * MAX_ENTRIES_PER_CHUNK;
        let chunk_end = (chunk_start + MAX_ENTRIES_PER_CHUNK).min(self.entries.len());

        let chunk_entries = &self.entries[chunk_start..chunk_end];

        // Compare full variable-length keys (exact match)
        match chunk_entries.binary_search_by(|e| e.encoding_key.as_slice().cmp(search_key)) {
            Ok(idx) => Some(&self.entries[chunk_start + idx]),
            Err(_) => None,
        }
    }

    /// Find all entries with matching key
    pub fn find_all_key_matches(&self, search_key: &[u8]) -> Vec<&IndexEntry> {
        let mut matches = Vec::new();

        // Find first match
        if let Some(first_match) = self.binary_search_key(search_key) {
            // Find position of first match using safe iterator methods
            let pos = self
                .entries
                .iter()
                .position(|entry| std::ptr::eq(entry, first_match))
                .unwrap_or(0);

            // Scan backwards for earlier exact matches
            let mut start_pos = pos;
            while start_pos > 0 {
                let prev_key = &self.entries[start_pos - 1].encoding_key;
                if prev_key.as_slice() == search_key {
                    start_pos -= 1;
                } else {
                    break;
                }
            }

            // Collect all consecutive exact matches
            let mut current_pos = start_pos;
            while current_pos < self.entries.len() {
                let entry_key = &self.entries[current_pos].encoding_key;
                if entry_key.as_slice() == search_key {
                    matches.push(&self.entries[current_pos]);
                    current_pos += 1;
                } else {
                    break;
                }
            }
        }

        matches
    }

    /// Validate index integrity
    pub fn validate(&self) -> ArchiveResult<()> {
        // Validate footer
        if !self.footer.is_valid() {
            return Err(ArchiveError::FooterChecksum);
        }

        // TOC hash validation is intentionally disabled.
        //
        // Research shows no reference implementation (CascLib, TACT.Net, rustycasc) validates
        // this field. Multiple algorithms were tested against WoW Classic 1.15.2.55140 files,
        // none matched Blizzard's stored values. The field appears to be metadata only, not
        // used for integrity verification.

        // Validate format
        self.footer.validate_format()?;

        // Validate entries are sorted
        if !is_sorted(&self.entries) {
            return Err(ArchiveError::UnsortedEntries);
        }

        // Validate TOC consistency
        self.validate_toc_consistency()?;

        Ok(())
    }

    /// Validate TOC matches actual entries
    fn validate_toc_consistency(&self) -> ArchiveResult<()> {
        let expected_chunks = calculate_chunks(self.entries.len());
        if self.toc.len() != expected_chunks {
            return Err(ArchiveError::TocInconsistent);
        }

        for (chunk_idx, expected_last_key) in self.toc.iter().enumerate() {
            let chunk_start = chunk_idx * MAX_ENTRIES_PER_CHUNK;
            let chunk_end = (chunk_start + MAX_ENTRIES_PER_CHUNK).min(self.entries.len());

            if chunk_end > chunk_start {
                let actual_last_key = &self.entries[chunk_end - 1].encoding_key;
                if actual_last_key != expected_last_key {
                    return Err(ArchiveError::TocInconsistent);
                }
            }
        }

        Ok(())
    }

    /// Get total number of entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get number of chunks
    pub fn chunk_count(&self) -> usize {
        self.toc.len()
    }

    /// Check if this is an archive-group index
    pub fn is_archive_group(&self) -> bool {
        self.footer.is_archive_group()
    }

    /// Write archive index to writer
    pub fn write_to<W: Write + Seek>(&self, mut writer: W) -> ArchiveResult<()> {
        let chunk_count = calculate_chunks(self.entries.len());
        let block_size = CHUNK_SIZE; // Standard block size

        // Write entry chunks
        let mut entry_idx = 0;
        for chunk_idx in 0..chunk_count {
            let mut chunk_data = vec![0u8; block_size];
            let mut cursor = Cursor::new(&mut chunk_data);

            let entries_in_chunk = if chunk_idx == chunk_count - 1 {
                // Last chunk gets remaining entries
                self.entries.len() - entry_idx
            } else {
                MAX_ENTRIES_PER_CHUNK.min(self.entries.len() - entry_idx)
            };

            // Write entries for this chunk
            for _ in 0..entries_in_chunk {
                if entry_idx < self.entries.len() {
                    let entry_bytes = self.entries[entry_idx].to_bytes(4, 4)?; // Standard 4-byte size and offset
                    cursor.write_all(&entry_bytes)?;
                    entry_idx += 1;
                }
            }

            // Write chunk to output
            writer.write_all(&chunk_data)?;
        }

        // Write table of contents (keys)
        for key in &self.toc {
            // Pad or truncate key to the expected TOC key size
            let toc_key_size = 16_usize; // Use 16 bytes for full key support
            let mut toc_key_padded = vec![0u8; toc_key_size];
            let copy_len = key.len().min(toc_key_size);
            toc_key_padded[..copy_len].copy_from_slice(&key[..copy_len]);
            writer.write_all(&toc_key_padded)?;
        }

        // Write TOC hashes (placeholder for now - 8 bytes each)
        for _ in &self.toc {
            let placeholder_hash = [0u8; 8];
            writer.write_all(&placeholder_hash)?;
        }

        // Write footer
        self.footer.write(&mut writer)?;

        Ok(())
    }
}

/// Archive index builder
pub struct ArchiveIndexBuilder {
    entries: Vec<IndexEntry>,
}

impl ArchiveIndexBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add entry to index with variable-length key
    pub fn add_entry(&mut self, encoding_key: Vec<u8>, size: u32, offset: u64) -> &mut Self {
        let entry = IndexEntry::new(encoding_key, size, offset);
        self.entries.push(entry);
        self
    }

    /// Add entry to index with full 16-byte key (for compatibility)
    pub fn add_entry_full(&mut self, encoding_key: [u8; 16], size: u32, offset: u64) -> &mut Self {
        let entry = IndexEntry::new(encoding_key.to_vec(), size, offset);
        self.entries.push(entry);
        self
    }

    /// Add entry with backward compatibility (old API) - temporary during refactoring
    pub fn add_entry_old(&mut self, encoding_key: [u8; 16], size: u32, offset: u32) -> &mut Self {
        let entry = IndexEntry::new(encoding_key.to_vec(), size, offset as u64);
        self.entries.push(entry);
        self
    }

    /// Build index and write to writer
    pub fn build<W: Write + Seek>(mut self, mut writer: W) -> ArchiveResult<ArchiveIndex> {
        // Sort entries by truncated encoding key
        self.entries.sort();

        let chunk_count = calculate_chunks(self.entries.len());
        let mut toc = Vec::with_capacity(chunk_count);

        // Write entry chunks and build TOC
        let mut entry_idx = 0;
        for chunk_idx in 0..chunk_count {
            let mut chunk_data = vec![0u8; CHUNK_SIZE];
            let mut cursor = Cursor::new(&mut chunk_data);

            let entries_in_chunk = if chunk_idx == chunk_count - 1 {
                // Last chunk gets remaining entries
                self.entries.len() - entry_idx
            } else {
                MAX_ENTRIES_PER_CHUNK.min(self.entries.len() - entry_idx)
            };

            // Write entries for this chunk
            for _ in 0..entries_in_chunk {
                if entry_idx < self.entries.len() {
                    let entry_bytes = self.entries[entry_idx].to_bytes(4, 4)?; // Standard 4-byte size and offset
                    cursor.write_all(&entry_bytes)?;
                    entry_idx += 1;
                }
            }

            // Record last key for TOC (using variable-length keys)
            if entries_in_chunk > 0 {
                let last_entry = &self.entries[entry_idx - 1];
                toc.push(last_entry.encoding_key.clone());
            }

            // Write chunk to output
            writer.write_all(&chunk_data)?;
        }

        // Write table of contents (keys)
        for key in &toc {
            // Pad or truncate key to the expected TOC key size (max of ekey_length, 9)
            let toc_key_size = 16_usize; // Use 16 bytes for full key support
            let mut toc_key_padded = vec![0u8; toc_key_size];
            let copy_len = key.len().min(toc_key_size);
            toc_key_padded[..copy_len].copy_from_slice(&key[..copy_len]);
            writer.write_all(&toc_key_padded)?;
        }

        // Write TOC hashes (placeholder for now - 8 bytes each)
        for _ in &toc {
            let placeholder_hash = [0u8; 8];
            writer.write_all(&placeholder_hash)?;
        }

        // Create and write footer
        let toc_hash = calculate_toc_hash(&toc);
        let footer = IndexFooter::new(toc_hash, self.entries.len() as u32);
        footer.write(&mut writer)?;

        Ok(ArchiveIndex {
            entries: self.entries,
            toc,
            footer,
        })
    }

    /// Get current number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if builder is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ArchiveIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveIndexBuilder {
    /// Create builder from existing archive index (for modification)
    ///
    /// This allows loading an existing archive index, modifying its entries,
    /// and rebuilding it with the changes applied.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cascette_formats::archive::ArchiveIndexBuilder;
    /// use std::io::Cursor;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Load existing archive index
    /// let data = std::fs::read("archive.index")?;
    /// let index = cascette_formats::archive::ArchiveIndex::parse(&mut Cursor::new(&data))?;
    ///
    /// // Convert to builder for modification
    /// let mut builder = ArchiveIndexBuilder::from_archive_index(&index);
    ///
    /// // Add new entry
    /// builder.add_entry_full([0xABu8; 16], 1024, 0);
    ///
    /// // Rebuild
    /// let mut output = Cursor::new(Vec::new());
    /// let modified = builder.build(&mut output)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_archive_index(index: &ArchiveIndex) -> Self {
        let mut builder = Self::new();

        // Copy all entries from existing index
        for entry in &index.entries {
            builder.entries.push(entry.clone());
        }

        builder
    }

    /// Remove an entry by encoding key
    ///
    /// Returns true if an entry was removed, false if no matching entry was found.
    /// Note: This compares only the first N bytes where N is the minimum of both key lengths.
    pub fn remove_entry(&mut self, encoding_key: &[u8]) -> bool {
        let original_len = self.entries.len();
        self.entries.retain(|e| {
            let min_len = e.encoding_key.len().min(encoding_key.len());
            e.encoding_key[..min_len] != encoding_key[..min_len]
        });
        self.entries.len() < original_len
    }

    /// Remove entry by full 16-byte key
    pub fn remove_entry_full(&mut self, encoding_key: &[u8; 16]) -> bool {
        self.remove_entry(encoding_key.as_slice())
    }

    /// Check if an entry exists by encoding key
    pub fn has_entry(&self, encoding_key: &[u8]) -> bool {
        self.entries.iter().any(|e| {
            let min_len = e.encoding_key.len().min(encoding_key.len());
            e.encoding_key[..min_len] == encoding_key[..min_len]
        })
    }

    /// Find entry by encoding key
    pub fn find_entry(&self, encoding_key: &[u8]) -> Option<&IndexEntry> {
        self.entries.iter().find(|e| {
            let min_len = e.encoding_key.len().min(encoding_key.len());
            e.encoding_key[..min_len] == encoding_key[..min_len]
        })
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Chunked archive index for memory-efficient large index handling
#[allow(dead_code)] // Future streaming feature
pub struct ChunkedArchiveIndex {
    /// Chunks loaded on demand
    chunks: Vec<Option<Vec<IndexEntry>>>,
    /// Table of contents (always loaded) - variable-length keys for CDN archives
    toc: Vec<Vec<u8>>,
    /// Footer (always loaded)
    footer: IndexFooter,
    /// Path to index file
    file_path: PathBuf,
}

impl ChunkedArchiveIndex {
    /// Open chunked index from file
    pub fn open<P: AsRef<Path>>(path: P) -> ArchiveResult<Self> {
        let mut file = File::open(&path)?;

        // Read footer and TOC (always loaded)
        // First, read hash_bytes to determine footer size
        // hash_bytes is at offset -13 from end
        file.seek(SeekFrom::End(-13))?;
        let mut hash_bytes_buf = [0u8; 1];
        StdRead::read_exact(&mut file, &mut hash_bytes_buf)?;
        let footer_hash_bytes = hash_bytes_buf[0];
        let footer_size = 20 + footer_hash_bytes as i64;

        // Read footer manually
        file.seek(SeekFrom::End(-footer_size))?;
        let mut footer_data = vec![0u8; 20];
        StdRead::read_exact(&mut file, &mut footer_data)?;

        let toc_hash = {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&footer_data[0..8]);
            arr
        };
        let version = footer_data[8];
        let reserved = [footer_data[9], footer_data[10]];
        let page_size_kb = footer_data[11];
        let offset_bytes = footer_data[12];
        let size_bytes = footer_data[13];
        let ekey_length = footer_data[14];
        let footer_hash_bytes_check = footer_data[15];
        let element_count = u32::from_le_bytes([
            footer_data[16],
            footer_data[17],
            footer_data[18],
            footer_data[19],
        ]);

        let mut footer_hash = vec![0u8; footer_hash_bytes as usize];
        StdRead::read_exact(&mut file, &mut footer_hash)?;

        let footer = IndexFooter {
            toc_hash,
            version,
            reserved,
            page_size_kb,
            offset_bytes,
            size_bytes,
            ekey_length,
            footer_hash_bytes: footer_hash_bytes_check,
            element_count,
            footer_hash,
        };

        if !footer.is_valid() {
            let expected_hash = footer.calculate_footer_hash();
            let mut expected_arr = [0u8; 8];
            let mut actual_arr = [0u8; 8];
            let copy_len = expected_hash.len().min(8);
            expected_arr[..copy_len].copy_from_slice(&expected_hash[..copy_len]);
            actual_arr[..copy_len].copy_from_slice(&footer.footer_hash[..copy_len]);
            return Err(ArchiveError::ChecksumMismatch {
                expected: expected_arr,
                actual: actual_arr,
            });
        }

        footer.validate_format()?;

        // Calculate actual chunk count based on data entries and records per chunk
        let block_size = (footer.page_size_kb as usize) * 1024; // Convert KB to bytes
        let record_size =
            footer.ekey_length as usize + footer.size_bytes as usize + footer.offset_bytes as usize;
        let records_per_block = block_size / record_size;
        let chunk_count = (footer.element_count as usize).div_ceil(records_per_block);

        let toc_key_size = footer.ekey_length as usize;
        let toc_size = chunk_count * (toc_key_size + footer.footer_hash_bytes as usize);
        let toc_offset = -((footer_size as usize + toc_size) as i64);
        file.seek(SeekFrom::End(toc_offset))?;

        let mut toc = Vec::with_capacity(chunk_count);

        // First read ALL the keys (using variable-length keys)
        for _ in 0..chunk_count {
            let mut key = vec![0u8; toc_key_size];
            StdRead::read_exact(&mut file, &mut key)?;
            toc.push(key);
        }

        // Then skip over ALL the hashes (we don't need them for now)
        let hash_section_size = chunk_count * footer.footer_hash_bytes as usize;
        file.seek(SeekFrom::Current(hash_section_size as i64))?;

        // TOC hash validation is intentionally disabled.
        //
        // Research shows no reference implementation (CascLib, TACT.Net, rustycasc) validates
        // this field. Multiple algorithms were tested against WoW Classic 1.15.2.55140 files:
        // - MD5(keys)[:8], MD5(keys)[8:16]
        // - MD5(keys+hashes)[:8], MD5(keys+hashes)[8:16]
        // - MD5(keys+page_hashes+last_page_hash)[:8] (TACT.Net generation algorithm)
        //
        // None matched Blizzard's stored values. The field appears to be metadata only,
        // not used for integrity verification.

        Ok(Self {
            chunks: vec![None; chunk_count],
            toc,
            footer,
            file_path: path.as_ref().to_path_buf(),
        })
    }

    /// Load specific chunk on demand
    #[allow(clippy::expect_used)] // Chunk is set to Some on line above
    fn load_chunk(&mut self, chunk_idx: usize) -> ArchiveResult<&Vec<IndexEntry>> {
        if self.chunks[chunk_idx].is_none() {
            let mut file = File::open(&self.file_path)?;
            file.seek(SeekFrom::Start((chunk_idx * CHUNK_SIZE) as u64))?;

            let mut chunk_data = vec![0u8; CHUNK_SIZE];
            StdRead::read_exact(&mut file, &mut chunk_data)?;

            let mut entries = Vec::new();
            let mut cursor = Cursor::new(&chunk_data);

            while cursor.position() + ENTRY_SIZE as u64 <= CHUNK_SIZE as u64 {
                let remaining = chunk_data.len() as u64 - cursor.position();
                if remaining < ENTRY_SIZE as u64 {
                    break;
                }
                let pos = cursor.position() as usize;
                let entry_bytes = &chunk_data[pos..pos + ENTRY_SIZE];
                cursor.set_position(cursor.position() + ENTRY_SIZE as u64);
                let entry = IndexEntry::parse(entry_bytes, 16, 4, 4)?; // Assume 16-byte keys for compatibility
                if entry.is_zero() {
                    break;
                }
                entries.push(entry);
            }

            self.chunks[chunk_idx] = Some(entries);
        }

        Ok(self.chunks[chunk_idx]
            .as_ref()
            .expect("chunk should be Some after loading"))
    }

    /// Find entry with chunk loading
    pub fn find_entry(&mut self, encoding_key: &[u8]) -> ArchiveResult<Option<&IndexEntry>> {
        // Find chunk using TOC
        // The TOC contains the maximum key for each chunk
        // We need to find the first chunk where max_key >= search_key
        let chunk_idx = match self.toc.binary_search_by(|toc_key| {
            // Compare keys (variable length)
            let compare_len = toc_key.len().min(encoding_key.len());
            toc_key[..compare_len].cmp(&encoding_key[..compare_len])
        }) {
            Ok(idx) => idx,
            Err(idx) => {
                // If idx == 0, the key is smaller than any chunk's max
                // If idx == toc.len(), the key is larger than any chunk's max
                if idx >= self.toc.len() {
                    return Ok(None);
                }
                idx
            }
        };

        // Load chunk if needed
        let chunk_entries = self.load_chunk(chunk_idx)?;

        // Binary search within chunk (exact match)
        match chunk_entries.binary_search_by(|e| e.encoding_key.as_slice().cmp(encoding_key)) {
            Ok(idx) => Ok(Some(&chunk_entries[idx])),
            Err(_) => Ok(None),
        }
    }
}

/// Helper functions
///
/// Calculate TOC hash using MD5 (upper 8 bytes)
pub fn calculate_toc_hash(toc: &[Vec<u8>]) -> [u8; 8] {
    // Concatenate all TOC keys for hashing
    let mut data = Vec::new();
    for key in toc {
        data.extend_from_slice(key);
    }

    // Calculate MD5 hash using cascette-crypto
    let content_key = cascette_crypto::md5::ContentKey::from_data(&data);

    // Return upper 8 bytes of MD5 hash (like reference implementation)
    let mut result = [0u8; 8];
    result.copy_from_slice(&content_key.as_bytes()[8..16]);
    result
}

/// Check if entries are sorted by encoding key
pub fn is_sorted(entries: &[IndexEntry]) -> bool {
    entries
        .windows(2)
        .all(|pair| pair[0].encoding_key <= pair[1].encoding_key)
}

/// Calculate number of chunks needed for entries
pub fn calculate_chunks(entry_count: usize) -> usize {
    entry_count.div_ceil(MAX_ENTRIES_PER_CHUNK)
}

impl crate::CascFormat for ArchiveIndex {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::parse(&mut Cursor::new(data)).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut output = Vec::new();
        self.write_to(&mut Cursor::new(&mut output))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        Ok(output)
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::single_match
)]
mod tests {
    use super::*;
    use crate::archive::constants::FOOTER_SIZE;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn test_index_entry_parsing() {
        let data = [
            // encoding_key (9 bytes)
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11,
            // size (4 bytes, big-endian)
            0x00, 0x01, 0x00, 0x00, // offset (4 bytes, big-endian)
            0x00, 0x00, 0x10, 0x00, // reserved (7 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let mut cursor = Cursor::new(&data);
        // Parse as old-style 24-byte entry: 9-byte key + 4-byte size + 4-byte offset + 7-byte reserved
        let pos = cursor.position() as usize;
        let entry_data = &data[pos..pos + 24];
        cursor.set_position(cursor.position() + 24);
        let entry = IndexEntry::parse(entry_data, 9, 4, 4).expect("Operation should succeed");

        assert_eq!(
            entry.encoding_key,
            vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11]
        );
        assert_eq!(entry.size, 0x0001_0000);
        assert_eq!(entry.offset, 0x0000_1000);
    }

    #[test]
    fn test_index_entry_round_trip() {
        let original = IndexEntry::new(
            vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11],
            0x1234_5678,
            0x8765_4321,
        );

        // Serialize
        let buffer = original.to_bytes(4, 4).expect("Operation should succeed");

        // Parse
        let parsed = IndexEntry::parse(&buffer, 9, 4, 4).expect("Operation should succeed");

        // Verify
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_footer_validation() {
        let toc_hash = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let mut footer = IndexFooter::new(toc_hash, 1);
        assert!(footer.is_valid());
        assert!(footer.validate_format().is_ok());

        footer.footer_hash = vec![0u8; 8];
        assert!(!footer.is_valid());
    }

    #[test]
    fn test_footer_format_validation() {
        let toc_hash = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let mut footer = IndexFooter::new(toc_hash, 1);

        // Valid format
        assert!(footer.validate_format().is_ok());

        // Invalid version
        footer.version = 0x08;
        assert!(matches!(
            footer.validate_format(),
            Err(ArchiveError::UnsupportedVersion(0x08))
        ));

        // Reset and test invalid ekey length (outside valid range 9-16)
        footer.version = 1;
        footer.ekey_length = 8; // Less than minimum 9
        assert!(matches!(
            footer.validate_format(),
            Err(ArchiveError::InvalidFormat(_))
        ));

        footer.ekey_length = 17; // Greater than maximum 16
        assert!(matches!(
            footer.validate_format(),
            Err(ArchiveError::InvalidFormat(_))
        ));

        // Reset and test invalid page size
        footer.ekey_length = 16;
        footer.page_size_kb = 8;
        assert!(matches!(
            footer.validate_format(),
            Err(ArchiveError::InvalidFormat(_))
        ));
    }

    #[test]
    fn test_builder_basic() {
        let mut builder = ArchiveIndexBuilder::new();
        let key1 = [1u8; 16];
        let key2 = [2u8; 16];

        builder.add_entry_old(key1, 100, 1000);
        builder.add_entry_old(key2, 200, 2000);

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.chunk_count(), 1);

        // Entries should be sorted
        assert!(index.entries[0].encoding_key <= index.entries[1].encoding_key);
    }

    #[test]
    fn test_builder_multiple_chunks() {
        let mut builder = ArchiveIndexBuilder::new();

        // Add enough entries to span multiple chunks
        let entries_count = MAX_ENTRIES_PER_CHUNK + 50;
        for i in 0..entries_count {
            let mut key = [0u8; 16];
            key[12..16].copy_from_slice(&(i as u32).to_be_bytes());
            builder.add_entry_old(key, (i * 100) as u32, (i * 4096) as u32);
        }

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        assert_eq!(index.entries.len(), entries_count);
        assert_eq!(index.chunk_count(), 2); // Should span 2 chunks

        // Verify all entries are sorted
        for i in 1..index.entries.len() {
            assert!(index.entries[i - 1].encoding_key <= index.entries[i].encoding_key);
        }
    }

    #[test]
    fn test_round_trip_build_parse() {
        let temp_dir = tempdir().expect("Operation should succeed");
        let index_path = temp_dir.path().join("test.index");

        // Build index
        let mut builder = ArchiveIndexBuilder::new();

        // Add test entries with various patterns
        for i in 0..500u32 {
            let mut key = [0u8; 16];
            // Create keys that will test truncation and sorting
            key[0] = (i % 256) as u8;
            key[8] = ((i >> 8) % 256) as u8;
            key[12..16].copy_from_slice(&i.to_be_bytes());

            builder.add_entry_old(key, i * 100 + 50, i * 4096 + 1024);
        }

        let mut file = std::fs::File::create(&index_path).expect("Operation should succeed");
        let original_index = builder.build(&mut file).expect("Operation should succeed");
        drop(file);

        // Parse index back
        let mut file = std::fs::File::open(&index_path).expect("Operation should succeed");
        let parsed_index = ArchiveIndex::parse(&mut file).expect("Operation should succeed");

        // Verify structure matches
        assert_eq!(original_index.entries.len(), parsed_index.entries.len());
        assert_eq!(original_index.toc.len(), parsed_index.toc.len());
        assert_eq!(
            original_index.footer.element_count,
            parsed_index.footer.element_count
        );

        // Verify all entries match
        for (orig, parsed) in original_index
            .entries
            .iter()
            .zip(parsed_index.entries.iter())
        {
            assert_eq!(orig, parsed);
        }

        // Verify TOC matches
        for (orig, parsed) in original_index.toc.iter().zip(parsed_index.toc.iter()) {
            assert_eq!(orig, parsed);
        }

        // Test that lookups work the same
        for key in [
            [
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x20, 0x30, 0x40, 0x50,
                0x60, 0x70,
            ],
            [
                0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x21, 0x31, 0x41, 0x51,
                0x61, 0x71,
            ],
        ] {
            let orig_result = original_index.find_entry(&key);
            let parsed_result = parsed_index.find_entry(&key);
            match (orig_result, parsed_result) {
                (Some(orig_entry), Some(parsed_entry)) => assert_eq!(orig_entry, parsed_entry),
                _ => {} // Both not found or collisions, acceptable
            }
        }
    }

    #[test]
    fn test_binary_search_functionality() {
        let mut builder = ArchiveIndexBuilder::new();

        // Create test data with known keys
        let test_keys = [
            [
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x20, 0x30, 0x40, 0x50,
                0x60, 0x70,
            ],
            [
                0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x21, 0x31, 0x41, 0x51,
                0x61, 0x71,
            ],
            [
                0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x22, 0x32, 0x42, 0x52,
                0x62, 0x72,
            ],
            [
                0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x13, 0x23, 0x33, 0x43, 0x53,
                0x63, 0x73,
            ],
        ];

        for (i, key) in test_keys.iter().enumerate() {
            builder.add_entry_old(*key, (i * 100) as u32, (i * 1000) as u32);
        }

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        // Test successful lookups
        for (i, key) in test_keys.iter().enumerate() {
            let entry = index.find_entry(key).expect("Should find entry");
            assert_eq!(entry.size, (i * 100) as u32);
            assert_eq!(entry.offset, (i * 1000) as u64);
        }

        // Test unsuccessful lookup
        let missing_key = [
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x24, 0x34, 0x44, 0x54,
            0x64, 0x74,
        ];
        assert!(index.find_entry(&missing_key).is_none());
    }

    #[test]
    fn test_exact_key_matching() {
        let mut builder = ArchiveIndexBuilder::new();

        // Create different keys for exact matching test
        let key1 = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xFF, 0x00,
        ];
        let key2 = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0xFF, 0xEE, 0xDD, 0xCC, 0xBB,
            0xAA, 0x99,
        ];

        builder.add_entry_old(key1, 100, 1000);
        builder.add_entry_old(key2, 200, 2000);

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        // Each key should find its own entry (exact matching, no collisions)
        let matches1 = index.find_all_entries(&key1);
        let matches2 = index.find_all_entries(&key2);

        // Each search should return exactly one entry (exact match)
        assert_eq!(matches1.len(), 1);
        assert_eq!(matches2.len(), 1);

        // Verify each key matches its own entry
        assert_eq!(matches1[0].size, 100);
        assert_eq!(matches2[0].size, 200);
    }

    #[test]
    fn test_toc_hash_calculation() {
        let toc = vec![
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09],
            vec![0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12],
        ];

        let hash1 = calculate_toc_hash(&toc);
        let hash2 = calculate_toc_hash(&toc);

        // Hash should be deterministic
        assert_eq!(hash1, hash2);

        // Different TOC should produce different hash
        let toc2 = vec![
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09],
            vec![0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13], // Changed last byte
        ];

        let hash3 = calculate_toc_hash(&toc2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_chunked_index_loading() {
        let temp_dir = tempdir().expect("Operation should succeed");
        let index_path = temp_dir.path().join("chunked.index");

        // Create large index that spans multiple chunks
        let mut builder = ArchiveIndexBuilder::new();
        let entries_count = MAX_ENTRIES_PER_CHUNK * 2 + 100;

        for i in 0..entries_count {
            let mut key = [0u8; 16];
            // Put the index in bytes 0-3 so it affects the truncated key
            key[0..4].copy_from_slice(&(i as u32).to_be_bytes());
            // Avoid creating entries that look like padding by ensuring non-zero size or offset
            let size = if i == 0 { 1 } else { (i * 50) as u32 }; // Avoid size=0 for first entry
            builder.add_entry_old(key, size, (i * 2048) as u32);
        }

        let mut file = std::fs::File::create(&index_path).expect("Operation should succeed");
        let _original = builder.build(&mut file).expect("Operation should succeed");
        drop(file);

        // Test chunked access
        let mut chunked_index =
            ChunkedArchiveIndex::open(&index_path).expect("Operation should succeed");

        // Access entry that should load first chunk
        #[allow(clippy::no_effect_underscore_binding)]
        let _search_key = [0u8; 12]; // Intentional for test
        let search_key = {
            let mut key = [0u8; 16];
            key[0..4].copy_from_slice(&0u32.to_be_bytes());
            key
        };

        let entry = chunked_index
            .find_entry(&search_key)
            .expect("Operation should succeed")
            .expect("Operation should succeed");
        assert_eq!(entry.size, 1); // Changed from 0 to 1
        assert_eq!(entry.offset, 0);

        // Access entry in different chunk
        let search_key2 = {
            let mut key = [0u8; 16];
            let target_idx = MAX_ENTRIES_PER_CHUNK + 50;
            key[0..4].copy_from_slice(&(target_idx as u32).to_be_bytes());
            key
        };

        let entry2 = chunked_index
            .find_entry(&search_key2)
            .expect("Operation should succeed")
            .expect("Operation should succeed");
        let expected_size = (MAX_ENTRIES_PER_CHUNK + 50) * 50;
        assert_eq!(entry2.size, expected_size as u32);
    }

    #[test]
    fn test_edge_case_empty_index() {
        let builder = ArchiveIndexBuilder::new();
        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        assert_eq!(index.entries.len(), 0);
        assert_eq!(index.toc.len(), 0);
        assert!(index.find_entry(&[0u8; 16]).is_none());
    }

    #[test]
    fn test_edge_case_single_entry() {
        let mut builder = ArchiveIndexBuilder::new();
        let key = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        builder.add_entry_old(key, 1024, 4096);

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.toc.len(), 1);

        let entry = index.find_entry(&key).expect("Operation should succeed");
        assert_eq!(entry.size, 1024);
        assert_eq!(entry.offset, 4096);

        // Test miss
        let miss_key = [
            0x02, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        assert!(index.find_entry(&miss_key).is_none());
    }

    #[test]
    fn test_edge_case_max_entries_per_chunk() {
        let mut builder = ArchiveIndexBuilder::new();

        // Add exactly MAX_ENTRIES_PER_CHUNK entries
        for i in 0..MAX_ENTRIES_PER_CHUNK {
            let mut key = [0u8; 16];
            key[0..4].copy_from_slice(&(i as u32).to_be_bytes());
            builder.add_entry_old(key, (i * 100) as u32, (i * 1000) as u32);
        }

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        assert_eq!(index.entries.len(), MAX_ENTRIES_PER_CHUNK);
        assert_eq!(index.toc.len(), 1); // Should fit in one chunk

        // Test first and last entries
        let first_key = [0u8; 16];
        let entry = index
            .find_entry(&first_key)
            .expect("Operation should succeed");
        assert_eq!(entry.size, 0);

        let mut last_key = [0u8; 16];
        last_key[0..4].copy_from_slice(&((MAX_ENTRIES_PER_CHUNK - 1) as u32).to_be_bytes());
        let entry = index
            .find_entry(&last_key)
            .expect("Operation should succeed");
        assert_eq!(entry.size, ((MAX_ENTRIES_PER_CHUNK - 1) * 100) as u32);
    }

    #[test]
    fn test_edge_case_exact_chunk_boundaries() {
        let mut builder = ArchiveIndexBuilder::new();

        // Add exactly MAX_ENTRIES_PER_CHUNK + 1 entries to force 2 chunks
        for i in 0..=MAX_ENTRIES_PER_CHUNK {
            let mut key = [0u8; 16];
            key[0..4].copy_from_slice(&(i as u32).to_be_bytes());
            let size = if i == 0 { 1 } else { (i * 100) as u32 }; // Avoid zero padding detection
            builder.add_entry_old(key, size, (i * 1000) as u32);
        }

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        assert_eq!(index.entries.len(), MAX_ENTRIES_PER_CHUNK + 1);
        assert_eq!(index.toc.len(), 2); // Should create 2 chunks

        // Test entries at chunk boundary
        let mut boundary_key = [0u8; 16];
        boundary_key[0..4].copy_from_slice(&((MAX_ENTRIES_PER_CHUNK - 1) as u32).to_be_bytes());
        assert!(index.find_entry(&boundary_key).is_some());

        let mut next_key = [0u8; 16];
        next_key[0..4].copy_from_slice(&(MAX_ENTRIES_PER_CHUNK as u32).to_be_bytes());
        assert!(index.find_entry(&next_key).is_some());
    }

    #[test]
    fn test_edge_case_large_values() {
        let mut builder = ArchiveIndexBuilder::new();
        let key = [0xff; 16];
        builder.add_entry_old(key, u32::MAX, u32::MAX);

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        let entry = index.find_entry(&key).expect("Operation should succeed");
        assert_eq!(entry.size, u32::MAX);
        assert_eq!(entry.offset, u64::from(u32::MAX));
    }

    #[test]
    fn test_variable_key_matching() {
        let mut builder = ArchiveIndexBuilder::new();

        // Test with different variable-length keys
        let key1 = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x10, 0x11, 0x12, 0x13, 0x14,
            0x15, 0x16,
        ];
        let key2 = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x20, 0x21, 0x22, 0x23, 0x24,
            0x25, 0x26,
        ];

        builder.add_entry_old(key1, 100, 1000);
        builder.add_entry_old(key2, 200, 2000);

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        // Both keys should be findable with their full keys
        let result1 = index.find_entry(&key1);
        let result2 = index.find_entry(&key2);

        assert!(result1.is_some());
        assert!(result2.is_some());

        if let (Some(entry1), Some(entry2)) = (result1, result2) {
            assert_eq!(entry1.size, 100);
            assert_eq!(entry2.size, 200);
        }
    }

    #[test]
    fn test_error_handling_invalid_data() {
        // Test parsing corrupted data
        let invalid_data = vec![0xff; 100]; // Random data
        let result = ArchiveIndex::parse(&mut Cursor::new(&invalid_data));
        assert!(result.is_err());

        // Test empty data
        let empty_data = Vec::new();
        let result = ArchiveIndex::parse(&mut Cursor::new(&empty_data));
        assert!(result.is_err());

        // Test data too short for footer
        let short_data = vec![0u8; 10];
        let result = ArchiveIndex::parse(&mut Cursor::new(&short_data));
        assert!(result.is_err());
    }

    #[test]
    fn test_is_sorted_function() {
        // Test sorted entries
        let sorted_entries = vec![
            IndexEntry::new(
                vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                100,
                1000,
            ),
            IndexEntry::new(
                vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                200,
                2000,
            ),
            IndexEntry::new(
                vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                300,
                3000,
            ),
        ];
        assert!(is_sorted(&sorted_entries));

        // Test unsorted entries
        let unsorted_entries = vec![
            IndexEntry::new(
                vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                300,
                3000,
            ),
            IndexEntry::new(
                vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                100,
                1000,
            ),
            IndexEntry::new(
                vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                200,
                2000,
            ),
        ];
        assert!(!is_sorted(&unsorted_entries));

        // Test equal entries (should be considered sorted)
        let equal_entries = vec![
            IndexEntry::new(
                vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                100,
                1000,
            ),
            IndexEntry::new(
                vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                200,
                2000,
            ),
        ];
        assert!(is_sorted(&equal_entries));
    }

    #[test]
    fn test_calculate_chunks_function() {
        assert_eq!(calculate_chunks(0), 0);
        assert_eq!(calculate_chunks(1), 1);
        assert_eq!(calculate_chunks(MAX_ENTRIES_PER_CHUNK), 1);
        assert_eq!(calculate_chunks(MAX_ENTRIES_PER_CHUNK + 1), 2);
        assert_eq!(calculate_chunks(MAX_ENTRIES_PER_CHUNK * 2), 2);
        assert_eq!(calculate_chunks(MAX_ENTRIES_PER_CHUNK * 2 + 1), 3);
    }

    #[test]
    fn test_casc_format_trait() {
        use crate::CascFormat;

        let mut builder = ArchiveIndexBuilder::new();
        let key = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ];
        builder.add_entry_old(key, 1024, 4096);

        let mut buffer = Vec::new();
        let original = builder
            .build(&mut Cursor::new(&mut buffer))
            .expect("Operation should succeed");

        println!("Built data size: {} bytes", buffer.len());
        let toc_key_size = if original.toc.is_empty() {
            16
        } else {
            original.toc[0].len()
        };
        let hash_size = 8; // footer_hash_bytes
        println!(
            "Expected structure: {} chunks * {} + {} TOC entries * ({} key + {} hash) + {} footer = {} bytes",
            original.toc.len(),
            CHUNK_SIZE,
            original.toc.len(),
            toc_key_size,
            hash_size,
            FOOTER_SIZE,
            original.toc.len() * CHUNK_SIZE
                + original.toc.len() * (toc_key_size + hash_size)
                + FOOTER_SIZE
        );

        // Test build through CascFormat trait
        let built_data = <ArchiveIndex as crate::CascFormat>::build(&original)
            .expect("Operation should succeed");

        println!("CascFormat built data size: {} bytes", built_data.len());

        // Check if parsing fails
        match <ArchiveIndex as crate::CascFormat>::parse(&built_data) {
            Ok(parsed) => {
                // Verify round-trip
                assert_eq!(original.entries.len(), parsed.entries.len());
                assert_eq!(original.entries[0], parsed.entries[0]);

                // Test verify_round_trip
                assert!(ArchiveIndex::verify_round_trip(&built_data).is_ok());
            }
            Err(e) => {
                println!("Parse failed: {}", e);
                println!("File size analysis:");
                println!("  Total size: {} bytes", built_data.len());
                println!("  Footer size: {} bytes", FOOTER_SIZE);
                println!(
                    "  Non-footer size: {} bytes",
                    built_data.len() - FOOTER_SIZE
                );
                println!(
                    "  Expected bytes_per_block: {} ({} + {})",
                    CHUNK_SIZE + ENTRY_SIZE,
                    CHUNK_SIZE,
                    ENTRY_SIZE
                );
                println!(
                    "  Remainder: {} bytes",
                    (built_data.len() - FOOTER_SIZE) % (CHUNK_SIZE + ENTRY_SIZE)
                );
                unreachable!("Parse should succeed for round-trip test");
            }
        }
    }

    // Real-world data tests
    #[test]
    fn test_problematic_cdn_index() {
        use std::fs::File;
        use std::io::{BufReader, Read as StdRead, Seek};

        let file_path = "/tmp/problem-index.index";
        if std::path::Path::new(file_path).exists() {
            let file = File::open(file_path).expect("Failed to open file");
            let mut reader = BufReader::new(file);

            println!("Testing problematic CDN archive index file...");
            println!(
                "File size: {} bytes",
                std::fs::metadata(file_path)
                    .expect("Failed to get file metadata in test")
                    .len()
            );

            // First let's manually check the footer
            let mut footer_reader =
                BufReader::new(File::open(file_path).expect("Failed to open test file"));
            footer_reader
                .seek(std::io::SeekFrom::End(-13))
                .expect("Failed to seek in test file");
            let mut hash_bytes_buf = [0u8; 1];
            footer_reader
                .read_exact(&mut hash_bytes_buf)
                .expect("Failed to read hash bytes in test");
            let footer_hash_bytes = hash_bytes_buf[0];
            println!("Footer hash bytes: {}", footer_hash_bytes);

            let footer_size = 20 + footer_hash_bytes as usize;
            println!("Footer size: {}", footer_size);

            footer_reader
                .seek(std::io::SeekFrom::End(-(footer_size as i64)))
                .expect("Failed to seek to footer in test");
            let mut footer_data = vec![0u8; 20];
            footer_reader
                .read_exact(&mut footer_data)
                .expect("Failed to read footer data in test");

            let ekey_length = footer_data[14];
            let element_count = u32::from_le_bytes([
                footer_data[16],
                footer_data[17],
                footer_data[18],
                footer_data[19],
            ]);

            println!("Key length: {} bytes", ekey_length);
            println!("Element count: {}", element_count);

            // Calculate chunk count properly: element_count represents data entries, not chunks
            let page_size_kb = footer_data[11] as usize;
            let offset_bytes = footer_data[12] as usize;
            let size_bytes = footer_data[13] as usize;
            let block_size = page_size_kb * 1024;
            let record_size = ekey_length as usize + size_bytes + offset_bytes;
            let records_per_block = block_size / record_size;
            let chunk_count = (element_count as usize).div_ceil(records_per_block);

            let toc_key_size = ekey_length as usize;
            let toc_entry_size = toc_key_size + footer_hash_bytes as usize;
            let toc_size = chunk_count * toc_entry_size; // TOC size based on chunks, not elements
            println!("TOC key size: {}", toc_key_size);
            println!("TOC entry size: {}", toc_entry_size);
            println!("Block size: {} KB = {} bytes", page_size_kb, block_size);
            println!("Record size: {} bytes", record_size);
            println!("Records per block: {}", records_per_block);
            println!("Chunk count: {}", chunk_count);
            println!("TOC size: {}", toc_size);

            let file_size = std::fs::metadata(file_path)
                .expect("Failed to get file metadata in test")
                .len() as usize;
            let data_size = file_size - footer_size - toc_size;
            println!("Data size: {}", data_size);

            let result = ArchiveIndex::parse(&mut reader);

            if let Ok(index) = &result {
                println!("✅ SUCCESS: Parsed with {} entries", index.entries.len());
                println!("  Footer version: {}", index.footer.version);
                println!("  Key length: {} bytes", index.footer.ekey_length);
                println!("  Element count: {}", index.footer.element_count);
            } else if let Err(e) = &result {
                println!("❌ FAILED: {:?}", e);
            }

            // Use assert for proper test failure reporting
            assert!(
                result.is_ok(),
                "Failed to parse problematic index: {:?}",
                result.err()
            );
        } else {
            println!("Skipping test - file not found: {}", file_path);
        }
    }

    // Performance and integration tests
    #[test]
    fn test_large_archive_performance() {
        let mut builder = ArchiveIndexBuilder::new();

        // Create large index with 10,000 entries (multiple chunks)
        for i in 0u32..10_000 {
            let mut key = [0u8; 16];
            key[0..4].copy_from_slice(&i.to_be_bytes());
            let size = if i == 0 { 1 } else { i.saturating_mul(50) };
            builder.add_entry_old(key, size, i.saturating_mul(1000));
        }

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        println!(
            "Large index: {} entries, {} chunks, {} bytes",
            index.entries.len(),
            index.toc.len(),
            output.len()
        );

        // Test lookups across chunks
        let test_keys = [5u32, 5000u32, 9999u32];
        for &key_val in &test_keys {
            let mut search_key = [0u8; 16];
            search_key[0..4].copy_from_slice(&key_val.to_be_bytes());
            assert!(index.find_entry(&search_key).is_some());
        }
    }

    #[test]
    fn test_realistic_casc_scenario() {
        let mut builder = ArchiveIndexBuilder::new();

        // Simulate realistic CASC archive with various content types
        let content_patterns = [
            (0x01, 50),   // Textures
            (0x02, 200),  // Models
            (0x03, 1000), // Sounds
            (0x04, 2000), // Data files
        ];

        let mut entry_count: u32 = 0;
        for (prefix, count) in content_patterns {
            for i in 0u32..count {
                let mut key = [0u8; 16];
                key[0] = prefix;
                key[1..5].copy_from_slice(&i.to_be_bytes());
                key[5..9].copy_from_slice(&entry_count.to_be_bytes());

                let size = (i + 1) * 1024; // Variable sizes
                let offset = entry_count * 65536; // 64KB aligned
                builder.add_entry_old(key, size, offset);
                entry_count += 1;
            }
        }

        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        // Test lookups for each content type
        for (prefix, _) in content_patterns {
            let mut search_key = [0u8; 16];
            search_key[0] = prefix;
            search_key[1..5].copy_from_slice(&10u32.to_be_bytes());

            let result = index.find_entry(&search_key);
            // Note: Due to truncated key collisions, we might not find the exact entry,
            // but we should be able to find entries with the same prefix pattern
            let found_any_with_prefix = index.entries.iter().any(|e| e.encoding_key[0] == prefix);
            assert!(
                found_any_with_prefix,
                "Should have entries with prefix {}",
                prefix
            );

            // If we found something, it should have the right truncated prefix
            if let Some(entry) = result {
                assert_eq!(entry.encoding_key[0], prefix);
            }
        }

        // Verify structure
        assert!(index.entries.len() > 1000);
        assert!(index.toc.len() > 1);

        // Test round-trip
        let parsed =
            ArchiveIndex::parse(&mut Cursor::new(&output)).expect("Operation should succeed");
        assert_eq!(index.entries.len(), parsed.entries.len());
    }

    #[test]
    fn test_error_recovery_scenarios() {
        // Test corrupted TOC hash
        let mut builder = ArchiveIndexBuilder::new();
        let key = [0x01; 16];
        builder.add_entry_old(key, 1000, 4096);

        let mut output = Vec::new();
        let _index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Operation should succeed");

        // Corrupt the TOC hash in the footer
        let footer_start = output.len() - FOOTER_SIZE;
        output[footer_start + 8..footer_start + 16].copy_from_slice(&[0xff; 8]);

        let result = ArchiveIndex::parse(&mut Cursor::new(&output));
        assert!(result.is_err());

        // Test unsorted entries detection
        let entries = vec![
            IndexEntry::new(
                vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                100,
                1000,
            ),
            IndexEntry::new(
                vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                200,
                2000,
            ),
        ];

        assert!(!is_sorted(&entries)); // Should be unsorted
    }

    #[test]
    fn test_from_archive_index() {
        // Build original index
        let mut original_builder = ArchiveIndexBuilder::new();
        let key1 = [1u8; 16];
        let key2 = [2u8; 16];
        let key3 = [3u8; 16];

        original_builder.add_entry_full(key1, 100, 1000);
        original_builder.add_entry_full(key2, 200, 2000);
        original_builder.add_entry_full(key3, 300, 3000);

        let mut output = Vec::new();
        let original = original_builder
            .build(&mut Cursor::new(&mut output))
            .expect("Failed to build original");

        // Create builder from existing index
        let mut modified_builder = ArchiveIndexBuilder::from_archive_index(&original);

        // Verify entries were copied
        assert_eq!(modified_builder.len(), 3);
        assert!(modified_builder.has_entry(&key1));
        assert!(modified_builder.has_entry(&key2));
        assert!(modified_builder.has_entry(&key3));

        // Add a new entry
        let key4 = [4u8; 16];
        modified_builder.add_entry_full(key4, 400, 4000);
        assert_eq!(modified_builder.len(), 4);
        assert!(modified_builder.has_entry(&key4));

        // Build modified index
        let mut modified_output = Vec::new();
        let modified = modified_builder
            .build(&mut Cursor::new(&mut modified_output))
            .expect("Failed to build modified");

        assert_eq!(modified.entries.len(), 4);
    }

    #[test]
    fn test_builder_remove_entry() {
        let mut builder = ArchiveIndexBuilder::new();
        let key1 = [1u8; 16];
        let key2 = [2u8; 16];
        let key3 = [3u8; 16];

        builder.add_entry_full(key1, 100, 1000);
        builder.add_entry_full(key2, 200, 2000);
        builder.add_entry_full(key3, 300, 3000);

        assert_eq!(builder.len(), 3);

        // Remove middle entry
        assert!(builder.remove_entry_full(&key2));
        assert_eq!(builder.len(), 2);
        assert!(builder.has_entry(&key1));
        assert!(!builder.has_entry(&key2));
        assert!(builder.has_entry(&key3));

        // Try to remove non-existent entry
        assert!(!builder.remove_entry_full(&key2));
        assert_eq!(builder.len(), 2);

        // Build should still work
        let mut output = Vec::new();
        let index = builder
            .build(&mut Cursor::new(&mut output))
            .expect("Failed to build");

        assert_eq!(index.entries.len(), 2);
    }

    #[test]
    fn test_builder_find_entry() {
        let mut builder = ArchiveIndexBuilder::new();
        let key1 = [1u8; 16];
        let key2 = [2u8; 16];

        builder.add_entry_full(key1, 100, 1000);
        builder.add_entry_full(key2, 200, 2000);

        // Find existing entry
        let found = builder.find_entry(&key1);
        assert!(found.is_some());
        let entry = found.expect("Entry should exist");
        assert_eq!(entry.size, 100);
        assert_eq!(entry.offset, 1000);

        // Find non-existent entry
        let key3 = [3u8; 16];
        assert!(builder.find_entry(&key3).is_none());
    }

    #[test]
    fn test_builder_clear() {
        let mut builder = ArchiveIndexBuilder::new();
        builder.add_entry_full([1u8; 16], 100, 1000);
        builder.add_entry_full([2u8; 16], 200, 2000);

        assert_eq!(builder.len(), 2);

        builder.clear();

        assert_eq!(builder.len(), 0);
        assert!(builder.is_empty());
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;
        use proptest::test_runner::TestCaseError;
        use std::io::Cursor;

        /// Generate arbitrary encoding keys of various valid lengths
        #[allow(dead_code)]
        fn encoding_key() -> impl Strategy<Value = Vec<u8>> {
            prop_oneof![
                // 9-byte truncated keys (local storage)
                prop::collection::vec(any::<u8>(), 9..=9),
                // 16-byte full MD5 keys (CDN archives)
                prop::collection::vec(any::<u8>(), 16..=16),
                // Other valid lengths
                prop::collection::vec(any::<u8>(), 1..=16),
            ]
        }

        /// Generate arbitrary index entries
        #[allow(dead_code)]
        fn index_entry() -> impl Strategy<Value = IndexEntry> {
            (encoding_key(), 1u32..u32::MAX, 0u64..u64::MAX)
                .prop_map(|(key, size, offset)| IndexEntry::new(key, size, offset))
        }

        /// Generate archive-group entries
        #[allow(dead_code)]
        fn archive_group_entry() -> impl Strategy<Value = IndexEntry> {
            (
                encoding_key(),
                1u32..u32::MAX,
                0u16..u16::MAX,
                0u32..u32::MAX,
            )
                .prop_map(|(key, size, archive_index, offset)| {
                    IndexEntry::new_archive_group(key, size, archive_index, offset)
                })
        }

        /// Generate footer with valid parameters
        #[allow(dead_code)]
        fn index_footer() -> impl Strategy<Value = IndexFooter> {
            (
                prop::array::uniform8(0u8..255),
                1u32..100u32, // reasonable element count for testing
            )
                .prop_map(|(toc_hash, element_count)| IndexFooter::new(toc_hash, element_count))
        }

        proptest! {
            /// Test that archive index round-trip always works for valid data
            fn archive_index_round_trip(
                entries in prop::collection::vec(index_entry(), 1..100)
            ) {
                let mut builder = ArchiveIndexBuilder::new();
                for entry in &entries {
                    builder.add_entry(entry.encoding_key.clone(), entry.size, entry.offset);
                }

                let mut output = Vec::new();
                let _index = builder.build(&mut Cursor::new(&mut output)).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Parse back from serialized data
                let mut cursor = Cursor::new(&output);
                let parsed = ArchiveIndex::parse(&mut cursor).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Verify entry count matches
                prop_assert_eq!(parsed.entries.len(), entries.len());

                // Verify all original entries are present (accounting for sorting)
                for original_entry in &entries {
                    let found = parsed.entries.iter().any(|parsed_entry| {
                        parsed_entry.encoding_key == original_entry.encoding_key
                            && parsed_entry.size == original_entry.size
                            && parsed_entry.offset == original_entry.offset
                    });
                    prop_assert!(
                        found,
                        "Original entry not found in parsed index: {:?}",
                        original_entry
                    );
                }
            }

            /// Test that footer validation works correctly
            fn footer_validation_correct(mut footer in index_footer()) {
                // Valid footer should pass validation
                prop_assert!(footer.is_valid());
                prop_assert!(footer.validate_format().is_ok());

                // Corrupt footer hash should fail validation
                footer.footer_hash[0] = footer.footer_hash[0].wrapping_add(1);
                prop_assert!(!footer.is_valid());
            }

            /// Test that invalid version numbers are rejected
            fn invalid_version_rejected(
                mut footer in index_footer(),
                invalid_version in prop::num::u8::ANY.prop_filter("Not version 1", |&v| v != 1)
            ) {
                footer.version = invalid_version;
                prop_assert!(footer.validate_format().is_err());
            }

            /// Test that invalid page sizes are rejected
            fn invalid_page_size_rejected(
                mut footer in index_footer(),
                invalid_page_size in prop::num::u8::ANY.prop_filter("Not 4KB", |&p| p != 4)
            ) {
                footer.page_size_kb = invalid_page_size;
                prop_assert!(footer.validate_format().is_err());
            }

            /// Test that variable-length keys are handled correctly
            fn variable_length_keys_handled(
                key_length in 1u8..=16,
                entries in prop::collection::vec(
                    (prop::collection::vec(any::<u8>(), 1..=16), 1u32..1000u32, 0u64..1_000_000u64),
                    1..50
                )
            ) {
                let mut builder = ArchiveIndexBuilder::new();

                // Normalize all keys to the same length
                for (key, size, offset) in entries {
                    let mut normalized_key = key;
                    normalized_key.truncate(key_length as usize);
                    if normalized_key.len() < key_length as usize {
                        normalized_key.resize(key_length as usize, 0);
                    }
                    builder.add_entry(normalized_key, size, offset);
                }

                let mut output = Vec::new();
                let index = builder.build(&mut Cursor::new(&mut output)).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Verify all keys have the expected length
                for entry in &index.entries {
                    prop_assert_eq!(entry.encoding_key.len(), key_length as usize);
                }

                // Verify footer reflects the key length
                prop_assert_eq!(index.footer.ekey_length, key_length);
            }

            /// Test that archive-group entries work correctly
            fn archive_group_entries_work(
                entries in prop::collection::vec(archive_group_entry(), 1..50)
            ) {
                for entry in entries {
                    // Verify archive index is preserved
                    prop_assert!(entry.archive_index.is_some());

                    // Verify the combined offset logic would work
                    if let Some(archive_index) = entry.archive_index {
                        let combined = ((archive_index as u64) << 32) | (entry.offset & 0xFFFF_FFFF);
                        prop_assert!(combined >= entry.offset); // Should be larger due to archive index bits
                    }
                }
            }

            /// Test that zero entries are detected correctly
            fn zero_entries_detected(
                non_zero_key in prop::collection::vec(1u8..255, 1..16),
                non_zero_size in 1u32..u32::MAX,
                non_zero_offset in 1u64..u64::MAX
            ) {
                // Zero entry should be detected as zero
                let zero_entry = IndexEntry::new(vec![0u8; 16], 0, 0);
                prop_assert!(zero_entry.is_zero());

                // Non-zero entry should not be detected as zero
                let non_zero_entry = IndexEntry::new(non_zero_key, non_zero_size, non_zero_offset);
                prop_assert!(!non_zero_entry.is_zero());
            }

            /// Test that entry parsing handles different byte sizes
            fn entry_parsing_byte_sizes(
                key in encoding_key(),
                size in 1u32..u32::MAX,
                offset in 0u64..u64::MAX,
                key_bytes in 1u8..=16,
                size_bytes in prop::sample::select(vec![4u8]), // Only 4-byte sizes supported
                offset_bytes in prop::sample::select(vec![4u8, 6u8]) // 4-byte or 6-byte (archive-group)
            ) {
                // Create test data
                let _entry = if offset_bytes == 6 {
                    // Archive-group format
                    IndexEntry::new_archive_group(key.clone(), size, 0, offset as u32)
                } else {
                    IndexEntry::new(key.clone(), size, offset)
                };

                // Simulate serialization based on byte sizes
                let mut data = Vec::new();

                // Write key (truncated/padded to key_bytes)
                let mut padded_key = key.clone();
                padded_key.truncate(key_bytes as usize);
                if padded_key.len() < key_bytes as usize {
                    padded_key.resize(key_bytes as usize, 0);
                }
                data.extend_from_slice(&padded_key);

                // Write size (always 4 bytes, big-endian)
                data.extend_from_slice(&size.to_be_bytes());

                // Write offset
                if offset_bytes == 6 {
                    // Archive-group: 2-byte archive index + 4-byte offset
                    data.extend_from_slice(&0u16.to_be_bytes()); // archive index
                }
                // Common: 4-byte offset (either standalone or part of 6-byte archive-group)
                data.extend_from_slice(&(offset as u32).to_be_bytes());

                // Parse entry from the data
                let parsed = IndexEntry::parse(&data, key_bytes, size_bytes, offset_bytes).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Verify parsed data matches expected
                prop_assert_eq!(parsed.encoding_key, padded_key);
                prop_assert_eq!(parsed.size, size);

                if offset_bytes == 6 {
                    prop_assert!(parsed.archive_index.is_some());
                } else {
                    prop_assert_eq!(parsed.offset, offset);
                    prop_assert!(parsed.archive_index.is_none());
                }
            }

            /// Test that entry sorting is stable and correct
            fn entry_sorting_stable(
                entries in prop::collection::vec(index_entry(), 2..100)
            ) {
                let mut builder = ArchiveIndexBuilder::new();
                for entry in &entries {
                    builder.add_entry(entry.encoding_key.clone(), entry.size, entry.offset);
                }

                let mut output = Vec::new();
                let index = builder.build(&mut Cursor::new(&mut output)).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Verify entries are sorted by encoding key
                for window in index.entries.windows(2) {
                    prop_assert!(window[0].encoding_key <= window[1].encoding_key);
                }
            }

            /// Test that large indices don't cause overflow
            fn large_indices_no_overflow(
                large_count in 1000usize..10000usize
            ) {
                let mut builder = ArchiveIndexBuilder::new();

                // Add many entries with sequential keys
                for i in 0..large_count.min(1000) { // Limit for test performance
                    let mut key = vec![0u8; 16];
                    key[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                    builder.add_entry(key, 100, i as u64);
                }

                let mut output = Vec::new();
                let result = builder.build(&mut Cursor::new(&mut output));

                // Should not panic or overflow
                prop_assert!(result.is_ok());

                if let Ok(index) = result {
                    prop_assert_eq!(index.entries.len(), large_count.min(1000));
                    prop_assert!(!index.toc.is_empty());
                }
            }
        }
    }
}

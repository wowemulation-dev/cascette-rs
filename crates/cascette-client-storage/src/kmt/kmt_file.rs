//! KMT V8 standalone on-disk format parser.
//!
//! The KMT V8 format stores key-to-location mappings in a two-level structure:
//! - A sorted section of 32-byte entries for binary search
//! - An update section of 1024-byte pages (25 entries of 40 bytes each)
//!
//! The file layout:
//! ```text
//! [OuterHeader: 8 bytes]            -- data_length + Jenkins hash
//! [Padding to 16-byte boundary]
//! [InnerHeader: 16 bytes]           -- revision, bucket, field sizes, etag
//! [Sorted entries: N * 32 bytes]
//! [Padding to 4096-byte boundary]
//! [Update pages: M * 1024 bytes]
//! ```

use cascette_crypto::jenkins::{hashlittle, hashlittle2};

use crate::{Result, StorageError};

/// Outer header size in bytes.
pub const KMT_V8_OUTER_HEADER_SIZE: usize = 8;

/// Inner header size in bytes.
pub const KMT_V8_INNER_HEADER_SIZE: usize = 0x10;

/// Sorted entry size in bytes.
pub const KMT_V8_SORTED_ENTRY_SIZE: usize = 32;

/// Update entry size in bytes.
pub const KMT_V8_UPDATE_ENTRY_SIZE: usize = 40;

/// Update page size in bytes.
pub const KMT_V8_UPDATE_PAGE_SIZE: usize = 1024;

/// Maximum entries per update page (1024 / 40 = 25).
pub const KMT_V8_ENTRIES_PER_PAGE: usize = KMT_V8_UPDATE_PAGE_SIZE / KMT_V8_UPDATE_ENTRY_SIZE;

/// Minimum update section size.
pub const KMT_V8_MIN_UPDATE_SIZE: usize = 0x7800;

/// Inner header alignment.
pub const KMT_V8_INNER_ALIGNMENT: usize = 16;

/// Update section alignment.
pub const KMT_V8_UPDATE_ALIGNMENT: usize = 4096;

// ---------------------------------------------------------------------------
// Outer header
// ---------------------------------------------------------------------------

/// Outer header wrapping the inner data with a Jenkins integrity hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KmtV8OuterHeader {
    /// Size of the inner data (everything after this header, before update section).
    pub data_length: u32,
    /// Jenkins `hashlittle` hash of the inner data.
    pub hash: u32,
}

impl KmtV8OuterHeader {
    /// Parse from the first 8 bytes of a buffer.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < KMT_V8_OUTER_HEADER_SIZE {
            return Err(StorageError::InvalidFormat(
                "KMT V8 outer header: buffer too small".into(),
            ));
        }
        let data_length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let hash = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        Ok(Self { data_length, hash })
    }

    /// Serialize to 8 bytes.
    pub fn to_bytes(&self) -> [u8; KMT_V8_OUTER_HEADER_SIZE] {
        let mut buf = [0u8; KMT_V8_OUTER_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.data_length.to_le_bytes());
        buf[4..8].copy_from_slice(&self.hash.to_le_bytes());
        buf
    }
}

// ---------------------------------------------------------------------------
// Inner header
// ---------------------------------------------------------------------------

/// Inner header describing the KMT V8 bucket parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KmtV8InnerHeader {
    /// Format revision (must be >= 8).
    pub revision: u16,
    /// Bucket index (0-15).
    pub bucket_index: u8,
    /// Flags (expected 0).
    pub flags: u8,
    /// Key size in bytes (must be 8).
    pub key_size: u8,
    /// Hash size in bytes (must be 8).
    pub hash_size: u8,
    /// Content key size in bytes (must be 0x10).
    pub content_key_size: u8,
    /// Padding byte.
    pub padding: u8,
    /// ETag data for CDN cache validation.
    pub etag_data: u64,
}

impl KmtV8InnerHeader {
    /// Parse from 16 bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < KMT_V8_INNER_HEADER_SIZE {
            return Err(StorageError::InvalidFormat(
                "KMT V8 inner header: buffer too small".into(),
            ));
        }
        let revision = u16::from_le_bytes([data[0], data[1]]);
        if revision < 8 {
            return Err(StorageError::InvalidFormat(format!(
                "KMT V8 inner header: revision {revision} < 8"
            )));
        }
        let bucket_index = data[2];
        let flags = data[3];
        let key_size = data[4];
        if key_size != 8 {
            return Err(StorageError::InvalidFormat(format!(
                "KMT V8 inner header: key_size {key_size} != 8"
            )));
        }
        let hash_size = data[5];
        if hash_size != 8 {
            return Err(StorageError::InvalidFormat(format!(
                "KMT V8 inner header: hash_size {hash_size} != 8"
            )));
        }
        let content_key_size = data[6];
        if content_key_size != 0x10 {
            return Err(StorageError::InvalidFormat(format!(
                "KMT V8 inner header: content_key_size {content_key_size:#x} != 0x10"
            )));
        }
        let padding = data[7];
        let etag_data = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        Ok(Self {
            revision,
            bucket_index,
            flags,
            key_size,
            hash_size,
            content_key_size,
            padding,
            etag_data,
        })
    }

    /// Serialize to 16 bytes.
    pub fn to_bytes(&self) -> [u8; KMT_V8_INNER_HEADER_SIZE] {
        let mut buf = [0u8; KMT_V8_INNER_HEADER_SIZE];
        buf[0..2].copy_from_slice(&self.revision.to_le_bytes());
        buf[2] = self.bucket_index;
        buf[3] = self.flags;
        buf[4] = self.key_size;
        buf[5] = self.hash_size;
        buf[6] = self.content_key_size;
        buf[7] = self.padding;
        buf[8..16].copy_from_slice(&self.etag_data.to_le_bytes());
        buf
    }
}

// ---------------------------------------------------------------------------
// Sorted entry (32 bytes)
// ---------------------------------------------------------------------------

/// A sorted section entry mapping an EKey to a storage location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KmtV8SortedEntry {
    /// Full 16-byte encoding key.
    pub ekey: [u8; 16],
    /// Storage offset stored as two LE u32s (low, high).
    storage_offset_low: u32,
    storage_offset_high: u32,
    /// Encoded (compressed) size.
    pub encoded_size: u32,
    /// Decoded (uncompressed) size.
    pub decoded_size: u32,
}

impl KmtV8SortedEntry {
    /// Create a new sorted entry.
    pub fn new(ekey: [u8; 16], storage_offset: u64, encoded_size: u32, decoded_size: u32) -> Self {
        Self {
            ekey,
            storage_offset_low: storage_offset as u32,
            storage_offset_high: (storage_offset >> 32) as u32,
            encoded_size,
            decoded_size,
        }
    }

    /// Combined 64-bit storage offset.
    pub fn storage_offset(&self) -> u64 {
        u64::from(self.storage_offset_high) << 32 | u64::from(self.storage_offset_low)
    }

    /// Parse from 32 bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < KMT_V8_SORTED_ENTRY_SIZE {
            return Err(StorageError::InvalidFormat(
                "KMT V8 sorted entry: buffer too small".into(),
            ));
        }
        let mut ekey = [0u8; 16];
        ekey.copy_from_slice(&data[0..16]);
        let storage_offset_low = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let storage_offset_high = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let encoded_size = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let decoded_size = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        Ok(Self {
            ekey,
            storage_offset_low,
            storage_offset_high,
            encoded_size,
            decoded_size,
        })
    }

    /// Serialize to 32 bytes.
    pub fn to_bytes(&self) -> [u8; KMT_V8_SORTED_ENTRY_SIZE] {
        let mut buf = [0u8; KMT_V8_SORTED_ENTRY_SIZE];
        buf[0..16].copy_from_slice(&self.ekey);
        buf[16..20].copy_from_slice(&self.storage_offset_low.to_le_bytes());
        buf[20..24].copy_from_slice(&self.storage_offset_high.to_le_bytes());
        buf[24..28].copy_from_slice(&self.encoded_size.to_le_bytes());
        buf[28..32].copy_from_slice(&self.decoded_size.to_le_bytes());
        buf
    }
}

// ---------------------------------------------------------------------------
// Update entry (40 bytes)
// ---------------------------------------------------------------------------

/// An update section entry with a Jenkins hash guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KmtV8UpdateEntry {
    /// Jenkins hash guard: `hashlittle2(entry[4..37], pc=0, pb=0).pc | 0x80000000`.
    pub hash_guard: u32,
    /// Full 16-byte encoding key.
    pub ekey: [u8; 16],
    /// Storage offset stored as two LE u32s (low, high).
    storage_offset_low: u32,
    storage_offset_high: u32,
    /// Encoded (compressed) size.
    pub encoded_size: u32,
    /// Decoded (uncompressed) size.
    pub decoded_size: u32,
    /// Entry status (1-7).
    pub status: u8,
    /// Padding bytes.
    pub padding: [u8; 3],
}

impl KmtV8UpdateEntry {
    /// Create a new update entry, computing the hash guard automatically.
    pub fn new(
        ekey: [u8; 16],
        storage_offset: u64,
        encoded_size: u32,
        decoded_size: u32,
        status: u8,
    ) -> Self {
        let mut entry = Self {
            hash_guard: 0,
            ekey,
            storage_offset_low: storage_offset as u32,
            storage_offset_high: (storage_offset >> 32) as u32,
            encoded_size,
            decoded_size,
            status,
            padding: [0; 3],
        };
        let bytes = entry.to_bytes();
        entry.hash_guard = Self::compute_hash_guard(&bytes);
        entry
    }

    /// Combined 64-bit storage offset.
    pub fn storage_offset(&self) -> u64 {
        u64::from(self.storage_offset_high) << 32 | u64::from(self.storage_offset_low)
    }

    /// Compute the hash guard for a serialized 40-byte entry.
    ///
    /// Hashes bytes [4..37] with `hashlittle2(pc=0, pb=0)` and returns `pc | 0x80000000`.
    pub fn compute_hash_guard(entry_bytes: &[u8; KMT_V8_UPDATE_ENTRY_SIZE]) -> u32 {
        let mut pc: u32 = 0;
        let mut pb: u32 = 0;
        hashlittle2(&entry_bytes[4..37], &mut pc, &mut pb);
        pc | 0x8000_0000
    }

    /// Check if the hash guard matches the entry contents.
    pub fn validate_hash_guard(&self) -> bool {
        let bytes = self.to_bytes();
        self.hash_guard == Self::compute_hash_guard(&bytes)
    }

    /// Parse from 40 bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < KMT_V8_UPDATE_ENTRY_SIZE {
            return Err(StorageError::InvalidFormat(
                "KMT V8 update entry: buffer too small".into(),
            ));
        }
        let hash_guard = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let mut ekey = [0u8; 16];
        ekey.copy_from_slice(&data[4..20]);
        let storage_offset_low = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let storage_offset_high = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let encoded_size = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        let decoded_size = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
        let status = data[36];
        let mut padding = [0u8; 3];
        padding.copy_from_slice(&data[37..40]);
        Ok(Self {
            hash_guard,
            ekey,
            storage_offset_low,
            storage_offset_high,
            encoded_size,
            decoded_size,
            status,
            padding,
        })
    }

    /// Serialize to 40 bytes.
    pub fn to_bytes(&self) -> [u8; KMT_V8_UPDATE_ENTRY_SIZE] {
        let mut buf = [0u8; KMT_V8_UPDATE_ENTRY_SIZE];
        buf[0..4].copy_from_slice(&self.hash_guard.to_le_bytes());
        buf[4..20].copy_from_slice(&self.ekey);
        buf[20..24].copy_from_slice(&self.storage_offset_low.to_le_bytes());
        buf[24..28].copy_from_slice(&self.storage_offset_high.to_le_bytes());
        buf[28..32].copy_from_slice(&self.encoded_size.to_le_bytes());
        buf[32..36].copy_from_slice(&self.decoded_size.to_le_bytes());
        buf[36] = self.status;
        buf[37..40].copy_from_slice(&self.padding);
        buf
    }

    /// Check if the entry slot is occupied (high bit set in hash_guard).
    pub fn is_valid(&self) -> bool {
        self.hash_guard & 0x8000_0000 != 0
    }
}

// ---------------------------------------------------------------------------
// Update page (1024 bytes)
// ---------------------------------------------------------------------------

/// A fixed-size page of update entries.
#[derive(Debug, Clone)]
pub struct KmtV8UpdatePage {
    entries: Vec<KmtV8UpdateEntry>,
}

impl KmtV8UpdatePage {
    /// Create an empty page.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the page is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get entries.
    pub fn entries(&self) -> &[KmtV8UpdateEntry] {
        &self.entries
    }

    /// Add an entry. Returns false if the page is full.
    pub fn push(&mut self, entry: KmtV8UpdateEntry) -> bool {
        if self.entries.len() >= KMT_V8_ENTRIES_PER_PAGE {
            return false;
        }
        self.entries.push(entry);
        true
    }

    /// Parse from a 1024-byte buffer. Returns `None` for empty pages.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < KMT_V8_UPDATE_PAGE_SIZE {
            return None;
        }
        // First 4 bytes zero means empty page
        if data[0..4] == [0, 0, 0, 0] {
            return None;
        }

        let mut entries = Vec::new();
        let mut offset = 0;
        while offset + KMT_V8_UPDATE_ENTRY_SIZE <= KMT_V8_UPDATE_PAGE_SIZE {
            let hash_guard = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            if hash_guard == 0 {
                break;
            }
            if let Ok(entry) = KmtV8UpdateEntry::parse(&data[offset..]) {
                entries.push(entry);
            }
            offset += KMT_V8_UPDATE_ENTRY_SIZE;
        }

        if entries.is_empty() {
            None
        } else {
            Some(Self { entries })
        }
    }

    /// Serialize to 1024 bytes.
    pub fn to_bytes(&self) -> [u8; KMT_V8_UPDATE_PAGE_SIZE] {
        let mut buf = [0u8; KMT_V8_UPDATE_PAGE_SIZE];
        for (i, entry) in self.entries.iter().enumerate() {
            let start = i * KMT_V8_UPDATE_ENTRY_SIZE;
            buf[start..start + KMT_V8_UPDATE_ENTRY_SIZE].copy_from_slice(&entry.to_bytes());
        }
        buf
    }
}

impl Default for KmtV8UpdatePage {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KMT V8 file
// ---------------------------------------------------------------------------

/// Parsed KMT V8 file.
#[derive(Debug, Clone)]
pub struct KmtV8File {
    /// Outer header (data length + integrity hash).
    pub outer_header: KmtV8OuterHeader,
    /// Inner header (format parameters).
    pub inner_header: KmtV8InnerHeader,
    /// Sorted entries (binary-searchable by ekey).
    pub sorted_entries: Vec<KmtV8SortedEntry>,
    /// Update pages (append-only log, newest last).
    pub update_pages: Vec<KmtV8UpdatePage>,
}

impl KmtV8File {
    /// Parse a KMT V8 file from raw bytes.
    ///
    /// Validates the outer header Jenkins hash against the inner data.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < KMT_V8_OUTER_HEADER_SIZE {
            return Err(StorageError::InvalidFormat(
                "KMT V8: file too small for outer header".into(),
            ));
        }

        // Parse outer header
        let outer_header = KmtV8OuterHeader::parse(data)?;
        let inner_start = align_up(KMT_V8_OUTER_HEADER_SIZE, KMT_V8_INNER_ALIGNMENT);

        let inner_end = inner_start + outer_header.data_length as usize;
        if inner_end > data.len() {
            return Err(StorageError::InvalidFormat(format!(
                "KMT V8: inner data extends past EOF ({inner_end} > {})",
                data.len()
            )));
        }

        // Validate Jenkins hash of inner data
        let inner_data = &data[inner_start..inner_end];
        let computed_hash = hashlittle(inner_data, 0);
        if computed_hash != outer_header.hash {
            return Err(StorageError::InvalidFormat(format!(
                "KMT V8: hash mismatch (computed {computed_hash:#010x}, expected {:#010x})",
                outer_header.hash
            )));
        }

        // Parse inner header
        let inner_header = KmtV8InnerHeader::parse(inner_data)?;

        // Parse sorted entries (rest of inner data after inner header)
        let sorted_data = &inner_data[KMT_V8_INNER_HEADER_SIZE..];
        let sorted_count = sorted_data.len() / KMT_V8_SORTED_ENTRY_SIZE;
        let mut sorted_entries = Vec::with_capacity(sorted_count);
        for i in 0..sorted_count {
            let offset = i * KMT_V8_SORTED_ENTRY_SIZE;
            sorted_entries.push(KmtV8SortedEntry::parse(
                &sorted_data[offset..offset + KMT_V8_SORTED_ENTRY_SIZE],
            )?);
        }

        // Parse update pages (aligned to 4096 after inner data)
        let update_start = align_up(inner_end, KMT_V8_UPDATE_ALIGNMENT);
        let mut update_pages = Vec::new();
        if update_start < data.len() {
            let mut offset = update_start;
            while offset + KMT_V8_UPDATE_PAGE_SIZE <= data.len() {
                if let Some(page) = KmtV8UpdatePage::from_bytes(&data[offset..]) {
                    update_pages.push(page);
                }
                offset += KMT_V8_UPDATE_PAGE_SIZE;
            }
        }

        Ok(Self {
            outer_header,
            inner_header,
            sorted_entries,
            update_pages,
        })
    }

    /// Serialize the file to bytes with correct alignment.
    pub fn to_bytes(&self) -> Vec<u8> {
        // Build inner data: inner header + sorted entries
        let sorted_size = self.sorted_entries.len() * KMT_V8_SORTED_ENTRY_SIZE;
        let inner_size = KMT_V8_INNER_HEADER_SIZE + sorted_size;
        let mut inner_data = Vec::with_capacity(inner_size);
        inner_data.extend_from_slice(&self.inner_header.to_bytes());
        for entry in &self.sorted_entries {
            inner_data.extend_from_slice(&entry.to_bytes());
        }

        // Compute outer header
        let hash = hashlittle(&inner_data, 0);
        let outer_header = KmtV8OuterHeader {
            data_length: inner_data.len() as u32,
            hash,
        };

        // Assemble output
        let inner_start = align_up(KMT_V8_OUTER_HEADER_SIZE, KMT_V8_INNER_ALIGNMENT);
        let inner_end = inner_start + inner_data.len();
        let update_start = align_up(inner_end, KMT_V8_UPDATE_ALIGNMENT);
        let update_size = self.update_pages.len() * KMT_V8_UPDATE_PAGE_SIZE;
        let total_size = update_start + update_size;

        let mut out = vec![0u8; total_size];
        out[0..KMT_V8_OUTER_HEADER_SIZE].copy_from_slice(&outer_header.to_bytes());
        out[inner_start..inner_end].copy_from_slice(&inner_data);
        for (i, page) in self.update_pages.iter().enumerate() {
            let page_offset = update_start + i * KMT_V8_UPDATE_PAGE_SIZE;
            out[page_offset..page_offset + KMT_V8_UPDATE_PAGE_SIZE]
                .copy_from_slice(&page.to_bytes());
        }

        out
    }

    /// Lookup an entry by ekey.
    ///
    /// Searches update pages newest-first (linear scan), then falls back to
    /// binary search in the sorted section.
    pub fn lookup(&self, ekey: &[u8; 16]) -> Option<KmtV8SortedEntry> {
        // Check update pages newest first
        for page in self.update_pages.iter().rev() {
            for entry in page.entries() {
                if entry.ekey == *ekey {
                    return Some(KmtV8SortedEntry::new(
                        entry.ekey,
                        entry.storage_offset(),
                        entry.encoded_size,
                        entry.decoded_size,
                    ));
                }
            }
        }

        // Binary search in sorted entries
        self.sorted_entries
            .binary_search_by(|e| e.ekey.cmp(ekey))
            .ok()
            .map(|idx| self.sorted_entries[idx])
    }

    /// Total number of entries (sorted + update).
    pub fn entry_count(&self) -> usize {
        let update_count: usize = self.update_pages.iter().map(KmtV8UpdatePage::len).sum();
        self.sorted_entries.len() + update_count
    }
}

/// Align `value` up to the next multiple of `alignment`.
fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn make_inner_header() -> KmtV8InnerHeader {
        KmtV8InnerHeader {
            revision: 8,
            bucket_index: 0,
            flags: 0,
            key_size: 8,
            hash_size: 8,
            content_key_size: 0x10,
            padding: 0,
            etag_data: 0x1234_5678_9ABC_DEF0,
        }
    }

    fn make_sorted_entry(seed: u8) -> KmtV8SortedEntry {
        KmtV8SortedEntry::new([seed; 16], 0x0001_0000_0000 | u64::from(seed), 1024, 2048)
    }

    #[test]
    fn test_kmt_v8_outer_header_parse() {
        let header = KmtV8OuterHeader {
            data_length: 42,
            hash: 0xDEAD_BEEF,
        };
        let bytes = header.to_bytes();
        let parsed = KmtV8OuterHeader::parse(&bytes).expect("parse");
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_kmt_v8_inner_header_validation() {
        let good = make_inner_header();
        let bytes = good.to_bytes();
        assert!(KmtV8InnerHeader::parse(&bytes).is_ok());

        // Reject revision < 8
        let mut bad = bytes;
        bad[0] = 7;
        bad[1] = 0;
        assert!(KmtV8InnerHeader::parse(&bad).is_err());

        // Reject wrong key_size
        let mut bad = bytes;
        bad[4] = 9;
        assert!(KmtV8InnerHeader::parse(&bad).is_err());

        // Reject wrong hash_size
        let mut bad = bytes;
        bad[5] = 4;
        assert!(KmtV8InnerHeader::parse(&bad).is_err());

        // Reject wrong content_key_size
        let mut bad = bytes;
        bad[6] = 0x08;
        assert!(KmtV8InnerHeader::parse(&bad).is_err());
    }

    #[test]
    fn test_kmt_v8_sorted_entry_roundtrip() {
        let entry = KmtV8SortedEntry::new([0xAA; 16], 0x0001_0000_CAFE, 4096, 8192);
        assert_eq!(entry.storage_offset(), 0x0001_0000_CAFE);

        let bytes = entry.to_bytes();
        let parsed = KmtV8SortedEntry::parse(&bytes).expect("parse");
        assert_eq!(parsed, entry);
        assert_eq!(parsed.storage_offset(), 0x0001_0000_CAFE);
    }

    #[test]
    fn test_kmt_v8_update_entry_hash_guard() {
        let entry = KmtV8UpdateEntry::new([0x11; 16], 0x1000, 512, 1024, 1);
        // Hash guard must have high bit set
        assert!(entry.hash_guard & 0x8000_0000 != 0);
        // Must validate
        assert!(entry.validate_hash_guard());

        // Different key produces different guard
        let entry2 = KmtV8UpdateEntry::new([0x22; 16], 0x1000, 512, 1024, 1);
        assert_ne!(
            entry.hash_guard & 0x7FFF_FFFF,
            entry2.hash_guard & 0x7FFF_FFFF
        );

        // Verify against manual computation
        let bytes = entry.to_bytes();
        let mut pc: u32 = 0;
        let mut pb: u32 = 0;
        hashlittle2(&bytes[4..37], &mut pc, &mut pb);
        assert_eq!(entry.hash_guard, pc | 0x8000_0000);
    }

    #[test]
    fn test_kmt_v8_file_roundtrip() {
        let inner_header = make_inner_header();
        let sorted_entries = vec![
            make_sorted_entry(0x10),
            make_sorted_entry(0x20),
            make_sorted_entry(0x30),
        ];
        let mut page = KmtV8UpdatePage::new();
        page.push(KmtV8UpdateEntry::new([0x40; 16], 0x2000, 256, 512, 2));

        let file = KmtV8File {
            outer_header: KmtV8OuterHeader {
                data_length: 0,
                hash: 0,
            },
            inner_header,
            sorted_entries,
            update_pages: vec![page],
        };

        let bytes = file.to_bytes();
        let parsed = KmtV8File::parse(&bytes).expect("roundtrip parse");

        assert_eq!(parsed.inner_header, file.inner_header);
        assert_eq!(parsed.sorted_entries.len(), 3);
        assert_eq!(parsed.update_pages.len(), 1);
        assert_eq!(parsed.update_pages[0].len(), 1);
        assert_eq!(parsed.update_pages[0].entries()[0].ekey, [0x40; 16]);
    }

    #[test]
    fn test_kmt_v8_lookup_sorted() {
        let inner_header = make_inner_header();
        // Sorted entries must be in ekey order
        let sorted_entries = vec![
            make_sorted_entry(0x10),
            make_sorted_entry(0x20),
            make_sorted_entry(0x30),
        ];

        let file = KmtV8File {
            outer_header: KmtV8OuterHeader {
                data_length: 0,
                hash: 0,
            },
            inner_header,
            sorted_entries,
            update_pages: vec![],
        };

        let bytes = file.to_bytes();
        let file = KmtV8File::parse(&bytes).expect("parse");

        // Found
        let result = file.lookup(&[0x20; 16]);
        assert!(result.is_some());
        let entry = result.expect("found");
        assert_eq!(entry.ekey, [0x20; 16]);
        assert_eq!(entry.encoded_size, 1024);

        // Not found
        assert!(file.lookup(&[0xFF; 16]).is_none());
    }

    #[test]
    fn test_kmt_v8_lookup_update_wins() {
        let inner_header = make_inner_header();
        let sorted_entries = vec![make_sorted_entry(0x20)];

        // Update page has same ekey with different sizes
        let mut page = KmtV8UpdatePage::new();
        page.push(KmtV8UpdateEntry::new([0x20; 16], 0x9000, 9999, 8888, 1));

        let file = KmtV8File {
            outer_header: KmtV8OuterHeader {
                data_length: 0,
                hash: 0,
            },
            inner_header,
            sorted_entries,
            update_pages: vec![page],
        };

        let bytes = file.to_bytes();
        let file = KmtV8File::parse(&bytes).expect("parse");

        let result = file.lookup(&[0x20; 16]).expect("found");
        // Update entry should win over sorted entry
        assert_eq!(result.encoded_size, 9999);
        assert_eq!(result.decoded_size, 8888);
        assert_eq!(result.storage_offset(), 0x9000);
    }

    #[test]
    fn test_kmt_v8_alignment() {
        let inner_header = make_inner_header();
        let sorted_entries = vec![make_sorted_entry(0x10)];
        let mut page = KmtV8UpdatePage::new();
        page.push(KmtV8UpdateEntry::new([0x50; 16], 0x3000, 100, 200, 1));

        let file = KmtV8File {
            outer_header: KmtV8OuterHeader {
                data_length: 0,
                hash: 0,
            },
            inner_header,
            sorted_entries,
            update_pages: vec![page],
        };

        let bytes = file.to_bytes();

        // Inner header starts at 16-byte boundary
        let inner_start = align_up(KMT_V8_OUTER_HEADER_SIZE, KMT_V8_INNER_ALIGNMENT);
        assert_eq!(inner_start % KMT_V8_INNER_ALIGNMENT, 0);

        // Outer header says where inner data is
        let outer = KmtV8OuterHeader::parse(&bytes).expect("outer");
        let inner_end = inner_start + outer.data_length as usize;

        // Update section starts at 4096-byte boundary
        let update_start = align_up(inner_end, KMT_V8_UPDATE_ALIGNMENT);
        assert_eq!(update_start % KMT_V8_UPDATE_ALIGNMENT, 0);

        // Verify update data is at that offset
        assert!(update_start + KMT_V8_UPDATE_PAGE_SIZE <= bytes.len());
        let page = KmtV8UpdatePage::from_bytes(&bytes[update_start..]).expect("update page");
        assert_eq!(page.entries()[0].ekey, [0x50; 16]);
    }
}

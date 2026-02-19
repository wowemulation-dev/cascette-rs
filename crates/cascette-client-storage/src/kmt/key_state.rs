//! Per-key resident/non-resident state tracking (KMT V8 format).
//!
//! The "V8" in Agent's source paths (`key_state_v8.cpp`) refers to the
//! key state entry format, NOT the KMT file version. Key state tracks
//! per-key resident/non-resident status for partial download support.
//!
//! KMT V8 entries are 40 bytes with full 16-byte EKeys and residency
//! span data. Pages are 1024 bytes holding 25 entries each.
//!
//! Telemetry counters:
//! - `dynamic_container.key_state.mark_fully_resident`
//! - `dynamic_container.key_state.mark_fully_nonresident`
//! - `dynamic_container.key_state.grew_update_buffer`

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use tracing::debug;

use crate::{Result, StorageError};

/// Per-key residency state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Key's data is fully downloaded and available.
    Resident,
    /// Key's data has been evicted or not yet downloaded.
    NonResident,
}

/// Key state tracker for a container.
///
/// Tracks how many keys have been marked resident/non-resident
/// for telemetry and maintains per-key state.
#[derive(Debug, Default)]
pub struct KeyStateTracker {
    /// Number of times `mark_fully_resident` was called.
    pub resident_count: u64,
    /// Number of times `mark_fully_nonresident` was called.
    pub non_resident_count: u64,
    /// Number of times the update buffer grew.
    pub grew_update_buffer_count: u64,
}

impl KeyStateTracker {
    /// Create a new key state tracker.
    pub const fn new() -> Self {
        Self {
            resident_count: 0,
            non_resident_count: 0,
            grew_update_buffer_count: 0,
        }
    }

    /// Mark a key as fully resident.
    pub fn mark_fully_resident(&mut self) {
        self.resident_count += 1;
    }

    /// Mark a key as fully non-resident.
    pub fn mark_fully_non_resident(&mut self) {
        self.non_resident_count += 1;
    }
}

// =========================================================================
// KMT V8 Residency Format
// =========================================================================

/// Size of a single residency entry (KMT V8 format).
pub const RESIDENCY_ENTRY_SIZE: usize = 40;

/// Size of a single residency page.
pub const RESIDENCY_PAGE_SIZE: usize = 1024;

/// Maximum entries per residency page (1024 / 40 = 25).
pub const RESIDENCY_ENTRIES_PER_PAGE: usize = RESIDENCY_PAGE_SIZE / RESIDENCY_ENTRY_SIZE;

/// Number of buckets for residency hashing.
pub const RESIDENCY_BUCKET_COUNT: usize = 16;

/// Flush interval: every 4th bucket page.
pub const RESIDENCY_FLUSH_INTERVAL: usize = 4;

/// Batch delete threshold: switches to batch path above 10,000 keys.
pub const BATCH_DELETE_THRESHOLD: usize = 10_000;

/// Update type byte values for residency entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidencyUpdateType {
    /// Invalid/empty entry.
    Invalid = 0,
    /// Set residency (existing key).
    Set = 1,
    /// Create residency (new key).
    Create = 2,
    /// Delete/tombstone.
    Delete = 3,
    /// Mark as resident.
    MarkResident = 6,
    /// Mark as non-resident.
    MarkNonResident = 7,
}

impl ResidencyUpdateType {
    /// Convert from raw byte.
    pub fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Set,
            2 => Self::Create,
            3 => Self::Delete,
            6 => Self::MarkResident,
            7 => Self::MarkNonResident,
            _ => Self::Invalid,
        }
    }

    /// Whether this update type represents a live (non-deleted) entry.
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Set | Self::Create | Self::MarkResident | Self::MarkNonResident
        )
    }
}

/// Residency span (4 x i32 = 16 bytes).
///
/// Represents a byte range within an archive entry that is either
/// resident or non-resident. The span is used for partial download
/// tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResidencySpan {
    /// Span offset (archive-relative).
    pub offset: i32,
    /// Span length.
    pub length: i32,
    /// Reserved field 1.
    pub reserved1: i32,
    /// Reserved field 2.
    pub reserved2: i32,
}

impl ResidencySpan {
    /// Create a span covering the full entry.
    pub const fn full() -> Self {
        Self {
            offset: 0,
            length: i32::MAX,
            reserved1: 0,
            reserved2: 0,
        }
    }

    /// Create a span for a specific range.
    pub const fn range(offset: i32, length: i32) -> Self {
        Self {
            offset,
            length,
            reserved1: 0,
            reserved2: 0,
        }
    }

    /// Serialize to 16 bytes (big-endian).
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.offset.to_be_bytes());
        buf[4..8].copy_from_slice(&self.length.to_be_bytes());
        buf[8..12].copy_from_slice(&self.reserved1.to_be_bytes());
        buf[12..16].copy_from_slice(&self.reserved2.to_be_bytes());
        buf
    }

    /// Deserialize from 16 bytes (big-endian).
    pub fn from_bytes(data: &[u8; 16]) -> Self {
        Self {
            offset: i32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            length: i32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            reserved1: i32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            reserved2: i32::from_be_bytes([data[12], data[13], data[14], data[15]]),
        }
    }
}

/// A single residency entry (KMT V8, 40 bytes on disk).
///
/// Layout:
/// ```text
/// [0x00] hash_flags  (4 bytes) = XOR hash with bit 31 set for valid entries
/// [0x04] ekey        (16 bytes, full encoding key)
/// [0x14] span        (16 bytes, 4 x i32 big-endian)
/// [0x24] update_type (1 byte)
/// [0x25] padding     (3 bytes)
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ResidencyEntry {
    /// Hash/flags: XOR hash of ekey with bit 31 set for valid entries.
    pub hash_flags: u32,
    /// Full 16-byte encoding key.
    pub ekey: [u8; 16],
    /// Residency span.
    pub span: ResidencySpan,
    /// Update type byte.
    pub update_type: ResidencyUpdateType,
}

impl ResidencyEntry {
    /// Create a new residency entry.
    pub fn new(ekey: [u8; 16], span: ResidencySpan, update_type: ResidencyUpdateType) -> Self {
        let hash_flags = Self::compute_hash(&ekey);
        Self {
            hash_flags,
            ekey,
            span,
            update_type,
        }
    }

    /// Compute the V8 bucket hash for a 16-byte key.
    ///
    /// XOR all 16 bytes, fold to 32 bits, then `(result >> 4 ^ result) & 0xF`.
    pub fn bucket_hash(ekey: &[u8; 16]) -> u8 {
        let mut xor: u8 = 0;
        for &b in ekey {
            xor ^= b;
        }
        ((xor >> 4) ^ xor) & 0x0F
    }

    /// Compute hash/flags for a key (XOR with high bit set).
    fn compute_hash(ekey: &[u8; 16]) -> u32 {
        let mut hash: u32 = 0;
        // XOR 16 bytes into a u32 (4 bytes at a time)
        for chunk in ekey.chunks(4) {
            let val = u32::from_le_bytes([
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
                chunk.get(3).copied().unwrap_or(0),
            ]);
            hash ^= val;
        }
        hash | 0x8000_0000 // Set high bit for "valid entry"
    }

    /// Serialize to 40 bytes.
    pub fn to_bytes(&self) -> [u8; RESIDENCY_ENTRY_SIZE] {
        let mut buf = [0u8; RESIDENCY_ENTRY_SIZE];
        buf[0..4].copy_from_slice(&self.hash_flags.to_le_bytes());
        buf[4..20].copy_from_slice(&self.ekey);
        buf[20..36].copy_from_slice(&self.span.to_bytes());
        buf[36] = self.update_type as u8;
        // buf[37..40] = padding (zeros)
        buf
    }

    /// Deserialize from 40 bytes.
    pub fn from_bytes(data: &[u8; RESIDENCY_ENTRY_SIZE]) -> Self {
        let hash_flags = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let mut ekey = [0u8; 16];
        ekey.copy_from_slice(&data[4..20]);
        let mut span_bytes = [0u8; 16];
        span_bytes.copy_from_slice(&data[20..36]);
        let span = ResidencySpan::from_bytes(&span_bytes);
        let update_type = ResidencyUpdateType::from_byte(data[36]);

        Self {
            hash_flags,
            ekey,
            span,
            update_type,
        }
    }

    /// Check if the entry is valid (high bit set in hash_flags).
    pub fn is_valid(&self) -> bool {
        self.hash_flags & 0x8000_0000 != 0
    }
}

/// MurmurHash3 64-bit finalizer for fast residency checks.
///
/// Uses the two MurmurHash3 constants from Agent.exe:
/// - `0xff51afd7_ed558ccd`
/// - `0xc4ceb9fe_1a85ec53`
pub fn murmurhash3_finalize(mut k: u64) -> u64 {
    k ^= k >> 33;
    k = k.wrapping_mul(0xff51_afd7_ed55_8ccd);
    k ^= k >> 33;
    k = k.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    k ^= k >> 33;
    k
}

/// A page of residency entries (1024 bytes, 25 entries max).
#[derive(Debug, Clone)]
pub struct ResidencyPage {
    entries: Vec<ResidencyEntry>,
}

impl ResidencyPage {
    /// Create a new empty page.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if full.
    pub fn is_full(&self) -> bool {
        self.entries.len() >= RESIDENCY_ENTRIES_PER_PAGE
    }

    /// Get entries.
    pub fn entries(&self) -> &[ResidencyEntry] {
        &self.entries
    }

    /// Add an entry. Returns false if full.
    pub fn push(&mut self, entry: ResidencyEntry) -> bool {
        if self.is_full() {
            return false;
        }
        self.entries.push(entry);
        true
    }

    /// Serialize to 1024 bytes.
    pub fn to_bytes(&self) -> [u8; RESIDENCY_PAGE_SIZE] {
        let mut buf = [0u8; RESIDENCY_PAGE_SIZE];
        for (i, entry) in self.entries.iter().enumerate() {
            let start = i * RESIDENCY_ENTRY_SIZE;
            buf[start..start + RESIDENCY_ENTRY_SIZE].copy_from_slice(&entry.to_bytes());
        }
        buf
    }

    /// Deserialize from 1024 bytes. Returns None for empty pages.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < RESIDENCY_PAGE_SIZE {
            return None;
        }
        if data[0..4] == [0, 0, 0, 0] {
            return None;
        }

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset + RESIDENCY_ENTRY_SIZE <= RESIDENCY_PAGE_SIZE {
            let hash_flags = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            if hash_flags == 0 {
                break;
            }
            let mut arr = [0u8; RESIDENCY_ENTRY_SIZE];
            arr.copy_from_slice(&data[offset..offset + RESIDENCY_ENTRY_SIZE]);
            entries.push(ResidencyEntry::from_bytes(&arr));
            offset += RESIDENCY_ENTRY_SIZE;
        }

        if entries.is_empty() {
            None
        } else {
            Some(Self { entries })
        }
    }
}

impl Default for ResidencyPage {
    fn default() -> Self {
        Self::new()
    }
}

/// File-backed residency database using KMT V8 format.
///
/// Stores per-key residency state in pages organized by bucket hash.
/// Provides MurmurHash3 fast-path for `is_resident()` and two-pass
/// `scan_keys()` matching Agent.exe behavior.
pub struct ResidencyDb {
    /// Per-bucket pages.
    buckets: [Vec<ResidencyPage>; RESIDENCY_BUCKET_COUNT],
    /// MurmurHash3 index for fast lookup: maps hash -> set of bucket page indices.
    hash_index: BTreeMap<u64, Vec<usize>>,
    /// File path for persistence.
    path: PathBuf,
    /// Whether the database has been modified since last save.
    dirty: bool,
}

impl ResidencyDb {
    /// Create a new empty database.
    pub fn new(path: PathBuf) -> Self {
        Self {
            buckets: std::array::from_fn(|_| Vec::new()),
            hash_index: BTreeMap::new(),
            path,
            dirty: false,
        }
    }

    /// Load from disk.
    pub fn load(path: &Path) -> Result<Self> {
        let mut db = Self::new(path.to_path_buf());

        if !path.exists() {
            return Ok(db);
        }

        let file = File::open(path)
            .map_err(|e| StorageError::Archive(format!("failed to open residency db: {e}")))?;
        let mut reader = BufReader::new(file);

        // Read all data
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|e| StorageError::Archive(format!("failed to read residency db: {e}")))?;

        // Parse bucket structure: 16 buckets, each with variable pages
        // Simple format: [bucket_id: u8][page_count: u32][pages...]
        let mut offset = 0;
        while offset + 5 <= data.len() {
            let bucket_id = data[offset] as usize;
            if bucket_id >= RESIDENCY_BUCKET_COUNT {
                break;
            }
            let page_count = u32::from_le_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
            ]) as usize;
            offset += 5;

            for _ in 0..page_count {
                if offset + RESIDENCY_PAGE_SIZE > data.len() {
                    break;
                }
                if let Some(page) = ResidencyPage::from_bytes(&data[offset..]) {
                    db.buckets[bucket_id].push(page);
                }
                offset += RESIDENCY_PAGE_SIZE;
            }
        }

        // Build hash index
        db.rebuild_hash_index();

        debug!(
            "loaded residency db from {}: {} entries",
            path.display(),
            db.entry_count()
        );
        Ok(db)
    }

    /// Save to disk.
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Archive(format!(
                    "failed to create residency directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let temp_path = self.path.with_extension("tmp");
        let file = File::create(&temp_path)
            .map_err(|e| StorageError::Archive(format!("failed to create residency temp: {e}")))?;
        let mut writer = BufWriter::new(&file);

        for (bucket_id, pages) in self.buckets.iter().enumerate() {
            if pages.is_empty() {
                continue;
            }
            // Write bucket header
            writer
                .write_all(&[bucket_id as u8])
                .map_err(|e| StorageError::Archive(format!("write error: {e}")))?;
            writer
                .write_all(&(pages.len() as u32).to_le_bytes())
                .map_err(|e| StorageError::Archive(format!("write error: {e}")))?;
            // Write pages
            for page in pages {
                writer
                    .write_all(&page.to_bytes())
                    .map_err(|e| StorageError::Archive(format!("write error: {e}")))?;
            }
        }

        writer
            .flush()
            .map_err(|e| StorageError::Archive(format!("flush error: {e}")))?;
        file.sync_all()
            .map_err(|e| StorageError::Archive(format!("fsync error: {e}")))?;

        std::fs::rename(&temp_path, &self.path)
            .map_err(|e| StorageError::Archive(format!("rename error: {e}")))?;

        self.dirty = false;
        Ok(())
    }

    /// Check if a key is resident using MurmurHash3 fast-path.
    ///
    /// Agent's `CheckResidency`: computes MurmurHash3 finalizer of the
    /// first 8 key bytes, masks by `(bucket_count - 1)` for slot, then
    /// walks the chain looking for a match.
    pub fn is_resident(&self, ekey: &[u8; 16]) -> bool {
        let bucket = ResidencyEntry::bucket_hash(ekey) as usize;
        if bucket >= RESIDENCY_BUCKET_COUNT {
            return false;
        }

        // Fast path: check MurmurHash3 index
        let key_u64 = u64::from_le_bytes([
            ekey[0], ekey[1], ekey[2], ekey[3], ekey[4], ekey[5], ekey[6], ekey[7],
        ]);
        let hash = murmurhash3_finalize(key_u64);

        // If the hash isn't in the index at all, the key isn't resident
        if !self.hash_index.contains_key(&hash) {
            return false;
        }

        // Full scan in the bucket for exact match
        for page in &self.buckets[bucket] {
            for entry in page.entries() {
                if entry.ekey == *ekey && entry.update_type.is_live() {
                    return entry.update_type != ResidencyUpdateType::MarkNonResident;
                }
            }
        }

        false
    }

    /// Mark a key as resident.
    pub fn mark_resident(&mut self, ekey: &[u8; 16]) {
        let entry = ResidencyEntry::new(*ekey, ResidencySpan::full(), ResidencyUpdateType::Set);
        self.insert_entry(entry);
        self.dirty = true;
    }

    /// Mark a key as non-resident.
    pub fn mark_non_resident(&mut self, ekey: &[u8; 16]) {
        let entry = ResidencyEntry::new(*ekey, ResidencySpan::full(), ResidencyUpdateType::Delete);
        self.insert_entry(entry);
        self.dirty = true;
    }

    /// Mark a specific span of a key as non-resident.
    ///
    /// Used by truncation tracking to record partial non-residency.
    pub fn mark_span_non_resident(&mut self, ekey: &[u8; 16], offset: i32, length: i32) {
        let span = ResidencySpan::range(offset, length);
        let entry = ResidencyEntry::new(*ekey, span, ResidencyUpdateType::MarkNonResident);
        self.insert_entry(entry);
        self.dirty = true;
    }

    /// Two-pass scan: count pass then populate pass across 16 buckets.
    ///
    /// Matches Agent's `ScanKeys` behavior.
    pub fn scan_keys(&self) -> Vec<[u8; 16]> {
        // Pass 1: count
        let mut count = 0;
        for bucket in &self.buckets {
            for page in bucket {
                for entry in page.entries() {
                    if entry.update_type.is_live()
                        && entry.update_type != ResidencyUpdateType::MarkNonResident
                    {
                        count += 1;
                    }
                }
            }
        }

        // Pass 2: populate
        let mut keys = Vec::with_capacity(count);
        for bucket in &self.buckets {
            for page in bucket {
                for entry in page.entries() {
                    if entry.update_type.is_live()
                        && entry.update_type != ResidencyUpdateType::MarkNonResident
                    {
                        keys.push(entry.ekey);
                    }
                }
            }
        }

        keys
    }

    /// Delete keys. Uses batch path if count exceeds threshold.
    pub fn delete_keys(&mut self, keys: &[[u8; 16]]) {
        if keys.len() > BATCH_DELETE_THRESHOLD {
            self.batch_delete(keys);
        } else {
            for key in keys {
                self.mark_non_resident(key);
            }
        }
    }

    /// Batch delete: mark all keys as deleted.
    fn batch_delete(&mut self, keys: &[[u8; 16]]) {
        // Build a set for fast lookup
        let key_set: std::collections::HashSet<[u8; 16]> = keys.iter().copied().collect();

        for bucket in &mut self.buckets {
            for page in bucket {
                for entry in &mut page.entries {
                    if key_set.contains(&entry.ekey) {
                        entry.update_type = ResidencyUpdateType::Delete;
                    }
                }
            }
        }
        self.dirty = true;
        self.rebuild_hash_index();
    }

    /// Total number of live entries across all buckets.
    pub fn entry_count(&self) -> usize {
        let mut count = 0;
        for bucket in &self.buckets {
            for page in bucket {
                for entry in page.entries() {
                    if entry.update_type.is_live() {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Insert or update an entry in the appropriate bucket.
    fn insert_entry(&mut self, entry: ResidencyEntry) {
        let bucket = ResidencyEntry::bucket_hash(&entry.ekey) as usize;
        if bucket >= RESIDENCY_BUCKET_COUNT {
            return;
        }

        let ekey = entry.ekey;

        // Check for existing entry with the same key
        let mut found = false;
        for page in &mut self.buckets[bucket] {
            for existing in &mut page.entries {
                if existing.ekey == ekey {
                    *existing = entry;
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }

        if !found {
            // Add to the last page if it has room, or create a new page
            let added = self.buckets[bucket]
                .last_mut()
                .is_some_and(|last_page| last_page.push(entry));
            if !added {
                let mut page = ResidencyPage::new();
                page.push(entry);
                self.buckets[bucket].push(page);
            }
        }

        self.update_hash_index_for_key(&ekey);
    }

    /// Rebuild the MurmurHash3 index from scratch.
    fn rebuild_hash_index(&mut self) {
        self.hash_index.clear();
        for (bucket_idx, bucket) in self.buckets.iter().enumerate() {
            for (page_idx, page) in bucket.iter().enumerate() {
                for entry in page.entries() {
                    if entry.update_type.is_live() {
                        let key_u64 = u64::from_le_bytes([
                            entry.ekey[0],
                            entry.ekey[1],
                            entry.ekey[2],
                            entry.ekey[3],
                            entry.ekey[4],
                            entry.ekey[5],
                            entry.ekey[6],
                            entry.ekey[7],
                        ]);
                        let hash = murmurhash3_finalize(key_u64);
                        self.hash_index
                            .entry(hash)
                            .or_default()
                            .push(bucket_idx * 1000 + page_idx);
                    }
                }
            }
        }
    }

    /// Update hash index for a single key.
    fn update_hash_index_for_key(&mut self, ekey: &[u8; 16]) {
        let key_u64 = u64::from_le_bytes([
            ekey[0], ekey[1], ekey[2], ekey[3], ekey[4], ekey[5], ekey[6], ekey[7],
        ]);
        let hash = murmurhash3_finalize(key_u64);
        let bucket = ResidencyEntry::bucket_hash(ekey) as usize;
        // Just ensure the hash is present in the index
        self.hash_index.entry(hash).or_insert_with(|| vec![bucket]);
    }
}

impl std::fmt::Debug for ResidencyDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResidencyDb")
            .field("path", &self.path)
            .field("entry_count", &self.entry_count())
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_residency_entry_round_trip() {
        let ekey = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ];
        let span = ResidencySpan::range(100, 5000);
        let entry = ResidencyEntry::new(ekey, span, ResidencyUpdateType::Set);

        assert!(entry.is_valid());
        assert_eq!(entry.ekey, ekey);

        let bytes = entry.to_bytes();
        assert_eq!(bytes.len(), RESIDENCY_ENTRY_SIZE);

        let parsed = ResidencyEntry::from_bytes(&bytes);
        assert_eq!(parsed.hash_flags, entry.hash_flags);
        assert_eq!(parsed.ekey, entry.ekey);
        assert_eq!(parsed.span, entry.span);
        assert_eq!(parsed.update_type, entry.update_type);
    }

    #[test]
    fn test_residency_page_round_trip() {
        let mut page = ResidencyPage::new();
        for i in 0..5 {
            let ekey = [i as u8; 16];
            let entry = ResidencyEntry::new(ekey, ResidencySpan::full(), ResidencyUpdateType::Set);
            assert!(page.push(entry));
        }

        let bytes = page.to_bytes();
        let parsed = ResidencyPage::from_bytes(&bytes).expect("should parse");
        assert_eq!(parsed.len(), 5);
    }

    #[test]
    fn test_residency_page_capacity() {
        let mut page = ResidencyPage::new();
        for i in 0..RESIDENCY_ENTRIES_PER_PAGE {
            let ekey = [i as u8; 16];
            let entry = ResidencyEntry::new(ekey, ResidencySpan::full(), ResidencyUpdateType::Set);
            assert!(page.push(entry), "should accept entry {i}");
        }
        assert!(page.is_full());

        let overflow =
            ResidencyEntry::new([0xFF; 16], ResidencySpan::full(), ResidencyUpdateType::Set);
        assert!(!page.push(overflow));
    }

    #[test]
    fn test_murmurhash3_finalize() {
        // Test known values
        let h1 = murmurhash3_finalize(0);
        let h2 = murmurhash3_finalize(1);
        let h3 = murmurhash3_finalize(u64::MAX);

        // Each should produce a different result
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        assert_ne!(h1, h3);

        // Deterministic
        assert_eq!(murmurhash3_finalize(42), murmurhash3_finalize(42));
    }

    #[test]
    fn test_bucket_hash_v8() {
        // All zeros
        assert_eq!(ResidencyEntry::bucket_hash(&[0u8; 16]), 0);
        // All ones
        assert_eq!(ResidencyEntry::bucket_hash(&[0xFF; 16]), 0);
        // Sequential
        let key = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        let bucket = ResidencyEntry::bucket_hash(&key);
        assert!(bucket < 16);
    }

    #[test]
    fn test_residency_db_operations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("residency.db");

        let mut db = ResidencyDb::new(db_path);

        let key1 = [0x11; 16];
        let key2 = [0x22; 16];

        // Mark resident
        db.mark_resident(&key1);
        db.mark_resident(&key2);
        assert!(db.is_resident(&key1));
        assert!(db.is_resident(&key2));
        assert_eq!(db.entry_count(), 2);

        // Mark non-resident
        db.mark_non_resident(&key1);
        assert!(!db.is_resident(&key1));
        assert!(db.is_resident(&key2));

        // Scan keys
        let keys = db.scan_keys();
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&key2));
    }

    #[test]
    fn test_residency_db_persistence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("residency.db");

        // Create and save
        {
            let mut db = ResidencyDb::new(db_path.clone());
            db.mark_resident(&[0xAA; 16]);
            db.mark_resident(&[0xBB; 16]);
            db.save().expect("save");
        }

        // Load and verify
        {
            let db = ResidencyDb::load(&db_path).expect("load");
            assert!(db.is_resident(&[0xAA; 16]));
            assert!(db.is_resident(&[0xBB; 16]));
            assert!(!db.is_resident(&[0xCC; 16]));
            assert_eq!(db.entry_count(), 2);
        }
    }

    #[test]
    fn test_residency_db_span_tracking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("residency.db");

        let mut db = ResidencyDb::new(db_path);
        let key = [0x55; 16];

        db.mark_resident(&key);
        assert!(db.is_resident(&key));

        // Mark span as non-resident
        db.mark_span_non_resident(&key, 1000, 5000);
        // After marking span non-resident, the key shows as non-resident
        assert!(!db.is_resident(&key));
    }

    #[test]
    fn test_residency_batch_delete() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("residency.db");

        let mut db = ResidencyDb::new(db_path);

        // Add many keys
        let keys: Vec<[u8; 16]> = (0..100u8).map(|i| [i; 16]).collect();
        for key in &keys {
            db.mark_resident(key);
        }
        assert_eq!(db.entry_count(), 100);

        // Delete half
        db.delete_keys(&keys[..50]);
        // Deleted entries are tombstoned but still counted
        // Check that first 50 are not resident
        for key in &keys[..50] {
            assert!(!db.is_resident(key));
        }
        for key in &keys[50..] {
            assert!(db.is_resident(key));
        }
    }

    #[test]
    fn test_residency_span_types() {
        let full = ResidencySpan::full();
        assert_eq!(full.offset, 0);
        assert_eq!(full.length, i32::MAX);

        let range = ResidencySpan::range(100, 500);
        let bytes = range.to_bytes();
        let parsed = ResidencySpan::from_bytes(&bytes);
        assert_eq!(parsed, range);
    }

    #[test]
    fn test_update_type_conversions() {
        for &(ut, byte_val) in &[
            (ResidencyUpdateType::Invalid, 0u8),
            (ResidencyUpdateType::Set, 1),
            (ResidencyUpdateType::Create, 2),
            (ResidencyUpdateType::Delete, 3),
            (ResidencyUpdateType::MarkResident, 6),
            (ResidencyUpdateType::MarkNonResident, 7),
        ] {
            assert_eq!(ResidencyUpdateType::from_byte(byte_val), ut);
            assert_eq!(ut as u8, byte_val);
        }

        assert!(ResidencyUpdateType::Set.is_live());
        assert!(!ResidencyUpdateType::Delete.is_live());
        assert!(!ResidencyUpdateType::Invalid.is_live());
    }
}

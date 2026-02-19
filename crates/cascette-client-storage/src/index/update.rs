//! KMT update section (LSM-tree L0 layer).
//!
//! The update section is an append-only log of 24-byte entries stored in
//! 512-byte pages. It sits at a 64KB-aligned boundary after the sorted
//! section in `.idx` files.
//!
//! Entries have a hash guard computed via `hashlittle(bytes[4..23], 0) | 0x80000000`.
//! Empty pages have their first 4 bytes set to zero. Parsing stops at the
//! first empty page.
//!
//! When the update section fills, `flush_updates` merges all entries into
//! the sorted section via merge-sort with atomic file replacement.

use cascette_crypto::jenkins::hashlittle;

use super::ArchiveLocation;
use super::IndexEntry;

/// Size of a single update entry in bytes.
pub const UPDATE_ENTRY_SIZE: usize = 24;

/// Size of a single update page in bytes.
pub const UPDATE_PAGE_SIZE: usize = 512;

/// Maximum entries per update page (512 / 24 = 21).
pub const ENTRIES_PER_PAGE: usize = UPDATE_PAGE_SIZE / UPDATE_ENTRY_SIZE;

/// Minimum update section size in bytes (60 pages).
pub const MIN_UPDATE_SECTION_SIZE: usize = 0x7800;

/// Alignment boundary for the update section start (64 KB).
pub const UPDATE_SECTION_ALIGNMENT: usize = 0x1_0000;

/// Sync interval: every 8th page triggers a 4KB sync.
pub const SYNC_PAGE_INTERVAL: usize = 8;

/// Status byte values for update entries.
///
/// These match the Agent.exe status encoding:
/// `(is_header ^ 1) + 6` yields 7 for data, 6 for header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UpdateStatus {
    /// Normal entry (resident, valid).
    Normal = 0,
    /// Deleted entry (tombstone).
    Delete = 3,
    /// Header is non-resident.
    HeaderNonResident = 6,
    /// Data is non-resident.
    DataNonResident = 7,
}

impl UpdateStatus {
    /// Convert a raw byte to an `UpdateStatus`.
    pub fn from_byte(b: u8) -> Self {
        match b {
            3 => Self::Delete,
            6 => Self::HeaderNonResident,
            7 => Self::DataNonResident,
            _ => Self::Normal,
        }
    }
}

/// A single entry in the update section (24 bytes on disk).
///
/// Layout:
/// ```text
/// [0x00] hash_guard  (4 bytes, LE) = hashlittle(bytes[4..23], 0) | 0x80000000
/// [0x04] ekey        (9 bytes)
/// [0x0D] offset      (5 bytes, BE packed archive location)
/// [0x12] size        (4 bytes, LE)
/// [0x16] status      (1 byte)
/// [0x17] padding     (1 byte)
/// ```
#[derive(Debug, Clone)]
pub struct UpdateEntry {
    /// Hash guard: `hashlittle(bytes[4..23], 0) | 0x80000000`.
    pub hash_guard: u32,
    /// Truncated encoding key (first 9 bytes).
    pub ekey: [u8; 9],
    /// Archive location (5 bytes packed big-endian).
    pub archive_location: ArchiveLocation,
    /// Encoded size.
    pub encoded_size: u32,
    /// Status byte.
    pub status: UpdateStatus,
}

impl UpdateEntry {
    /// Create a new update entry with computed hash guard.
    pub fn new(
        ekey: [u8; 9],
        archive_location: ArchiveLocation,
        encoded_size: u32,
        status: UpdateStatus,
    ) -> Self {
        let mut entry = Self {
            hash_guard: 0,
            ekey,
            archive_location,
            encoded_size,
            status,
        };
        // Serialize with hash_guard=0, compute hash from bytes[4..23],
        // then set the correct hash_guard. Since hash_guard is at bytes[0..4],
        // it does not affect the hashed range.
        let bytes = entry.to_bytes();
        entry.hash_guard = Self::compute_hash_guard(&bytes);
        entry
    }

    /// Compute the hash guard from serialized entry bytes.
    ///
    /// Hashes bytes 4 through 22 inclusive (19 bytes: ekey + offset + size + status).
    pub fn compute_hash_guard(entry_bytes: &[u8; UPDATE_ENTRY_SIZE]) -> u32 {
        hashlittle(&entry_bytes[4..23], 0) | 0x8000_0000
    }

    /// Serialize to 24 bytes.
    pub fn to_bytes(&self) -> [u8; UPDATE_ENTRY_SIZE] {
        let mut buf = [0u8; UPDATE_ENTRY_SIZE];

        buf[0..4].copy_from_slice(&self.hash_guard.to_le_bytes());
        buf[4..13].copy_from_slice(&self.ekey);

        // Pack archive location: 1 byte high + 4 bytes packed (big-endian)
        let index_high = (self.archive_location.archive_id >> 2) as u8;
        let archive_low = u32::from(self.archive_location.archive_id & 0x03);
        let packed = (archive_low << 30) | (self.archive_location.archive_offset & 0x3FFF_FFFF);
        buf[13] = index_high;
        buf[14..18].copy_from_slice(&packed.to_be_bytes());

        buf[18..22].copy_from_slice(&self.encoded_size.to_le_bytes());
        buf[22] = self.status as u8;
        buf[23] = 0; // padding

        buf
    }

    /// Deserialize from 24 bytes.
    pub fn from_bytes(data: &[u8; UPDATE_ENTRY_SIZE]) -> Self {
        let hash_guard = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&data[4..13]);

        // Parse archive location: 1 byte high + 4 bytes packed (big-endian)
        let index_high = u16::from(data[13]);
        let packed = u32::from_be_bytes([data[14], data[15], data[16], data[17]]);
        let archive_id = (index_high << 2) | u16::try_from(packed >> 30).unwrap_or(0);
        let archive_offset = packed & 0x3FFF_FFFF;

        let encoded_size = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
        let status = UpdateStatus::from_byte(data[22]);

        Self {
            hash_guard,
            ekey,
            archive_location: ArchiveLocation {
                archive_id,
                archive_offset,
            },
            encoded_size,
            status,
        }
    }

    /// Convert to an `IndexEntry` (for merge into sorted section).
    pub fn to_index_entry(&self) -> IndexEntry {
        IndexEntry::new(
            self.ekey,
            self.archive_location.archive_id,
            self.archive_location.archive_offset,
            self.encoded_size,
        )
    }

    /// Check if the hash guard matches the entry contents.
    pub fn validate_hash_guard(&self) -> bool {
        let bytes = self.to_bytes();
        let expected = Self::compute_hash_guard(&bytes);
        self.hash_guard == expected
    }
}

/// A page of update entries (512 bytes on disk, up to 21 entries).
#[derive(Debug, Clone)]
pub struct UpdatePage {
    entries: Vec<UpdateEntry>,
}

impl UpdatePage {
    /// Create a new empty page.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Check if the page has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if the page is full (21 entries).
    pub fn is_full(&self) -> bool {
        self.entries.len() >= ENTRIES_PER_PAGE
    }

    /// Number of entries in this page.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Get entries.
    pub fn entries(&self) -> &[UpdateEntry] {
        &self.entries
    }

    /// Add an entry to this page. Returns false if full.
    pub fn push(&mut self, entry: UpdateEntry) -> bool {
        if self.is_full() {
            return false;
        }
        self.entries.push(entry);
        true
    }

    /// Serialize to 512 bytes.
    pub fn to_bytes(&self) -> [u8; UPDATE_PAGE_SIZE] {
        let mut buf = [0u8; UPDATE_PAGE_SIZE];
        for (i, entry) in self.entries.iter().enumerate() {
            let start = i * UPDATE_ENTRY_SIZE;
            buf[start..start + UPDATE_ENTRY_SIZE].copy_from_slice(&entry.to_bytes());
        }
        buf
    }

    /// Deserialize from a 512-byte slice. Returns `None` for empty pages.
    ///
    /// An empty page is detected by the first 4 bytes being zero
    /// (hash_guard of the first entry is zero).
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < UPDATE_PAGE_SIZE {
            return None;
        }

        // Empty page check: first 4 bytes are zero
        if data[0..4] == [0, 0, 0, 0] {
            return None;
        }

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset + UPDATE_ENTRY_SIZE <= UPDATE_PAGE_SIZE {
            let entry_slice = &data[offset..offset + UPDATE_ENTRY_SIZE];

            // Check for empty entry slot (hash_guard == 0)
            let hash_guard = u32::from_le_bytes([
                entry_slice[0],
                entry_slice[1],
                entry_slice[2],
                entry_slice[3],
            ]);
            if hash_guard == 0 {
                break;
            }

            let mut arr = [0u8; UPDATE_ENTRY_SIZE];
            arr.copy_from_slice(entry_slice);
            entries.push(UpdateEntry::from_bytes(&arr));
            offset += UPDATE_ENTRY_SIZE;
        }

        if entries.is_empty() {
            None
        } else {
            Some(Self { entries })
        }
    }
}

impl Default for UpdatePage {
    fn default() -> Self {
        Self::new()
    }
}

/// The update section: an append-only log of pages (LSM-tree L0).
///
/// Minimum size is 60 pages (0x7800 bytes). The section starts at a
/// 64KB-aligned boundary after the sorted section.
#[derive(Debug, Clone)]
pub struct UpdateSection {
    /// Pages of update entries.
    pages: Vec<UpdatePage>,
    /// Total capacity in pages.
    capacity_pages: usize,
}

impl UpdateSection {
    /// Create a new empty update section with minimum capacity (60 pages).
    pub fn new() -> Self {
        let capacity_pages = MIN_UPDATE_SECTION_SIZE / UPDATE_PAGE_SIZE;
        Self {
            pages: Vec::new(),
            capacity_pages,
        }
    }

    /// Create with specified byte capacity (at least `MIN_UPDATE_SECTION_SIZE`).
    pub fn with_capacity(size_bytes: usize) -> Self {
        let capacity_pages = size_bytes.max(MIN_UPDATE_SECTION_SIZE) / UPDATE_PAGE_SIZE;
        Self {
            pages: Vec::new(),
            capacity_pages,
        }
    }

    /// Check if the update section is full.
    pub fn is_full(&self) -> bool {
        if self.pages.is_empty() {
            return false;
        }
        let last = &self.pages[self.pages.len() - 1];
        self.pages.len() >= self.capacity_pages && last.is_full()
    }

    /// Total entry count across all pages.
    pub fn entry_count(&self) -> usize {
        self.pages.iter().map(UpdatePage::len).sum()
    }

    /// Number of pages used.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Capacity in pages.
    pub fn capacity_pages(&self) -> usize {
        self.capacity_pages
    }

    /// Append an entry. Returns `true` on success, `false` if full.
    pub fn append(&mut self, entry: UpdateEntry) -> bool {
        // Try to append to the last page if it has room
        if let Some(last) = self.pages.last_mut()
            && !last.is_full()
        {
            last.entries.push(entry);
            return true;
        }

        // Need a new page
        if self.pages.len() >= self.capacity_pages {
            return false;
        }

        let mut page = UpdatePage::new();
        page.entries.push(entry);
        self.pages.push(page);
        true
    }

    /// Whether the most recently added page should trigger a sync.
    ///
    /// Agent syncs every 8th page (`page_index & 7 == 7`).
    pub fn should_sync(&self) -> bool {
        !self.pages.is_empty()
            && (self.pages.len() - 1) % SYNC_PAGE_INTERVAL == (SYNC_PAGE_INTERVAL - 1)
    }

    /// Search for a key in the update section (linear scan, newest first).
    ///
    /// Returns the most recent entry for the key, scanning pages and
    /// entries in reverse order.
    pub fn search(&self, ekey: &[u8; 9]) -> Option<&UpdateEntry> {
        for page in self.pages.iter().rev() {
            for entry in page.entries().iter().rev() {
                if entry.ekey == *ekey {
                    return Some(entry);
                }
            }
        }
        None
    }

    /// Iterate all entries across all pages (oldest first).
    pub fn all_entries(&self) -> impl Iterator<Item = &UpdateEntry> {
        self.pages.iter().flat_map(UpdatePage::entries)
    }

    /// Clear all pages.
    pub fn clear(&mut self) {
        self.pages.clear();
    }

    /// Serialize the entire update section to bytes.
    ///
    /// Output size is `capacity_pages * UPDATE_PAGE_SIZE`, padded with
    /// zeros for unused pages.
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_pages = self.capacity_pages.max(self.pages.len());
        let mut buf = vec![0u8; total_pages * UPDATE_PAGE_SIZE];

        for (i, page) in self.pages.iter().enumerate() {
            let start = i * UPDATE_PAGE_SIZE;
            buf[start..start + UPDATE_PAGE_SIZE].copy_from_slice(&page.to_bytes());
        }

        buf
    }

    /// Deserialize from raw bytes.
    ///
    /// Stops at the first empty page (first 4 bytes zero).
    pub fn from_bytes(data: &[u8]) -> Self {
        let capacity_pages = data.len() / UPDATE_PAGE_SIZE;
        let mut pages = Vec::new();

        let mut offset = 0;
        while offset + UPDATE_PAGE_SIZE <= data.len() {
            let page_data = &data[offset..offset + UPDATE_PAGE_SIZE];
            match UpdatePage::from_bytes(page_data) {
                Some(page) => pages.push(page),
                None => break,
            }
            offset += UPDATE_PAGE_SIZE;
        }

        Self {
            pages,
            capacity_pages: capacity_pages.max(MIN_UPDATE_SECTION_SIZE / UPDATE_PAGE_SIZE),
        }
    }
}

impl Default for UpdateSection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_update_entry_round_trip() {
        let entry = UpdateEntry::new(
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x00],
            ArchiveLocation {
                archive_id: 42,
                archive_offset: 0x1000,
            },
            4096,
            UpdateStatus::Normal,
        );

        // Verify hash guard has high bit set
        assert!(entry.hash_guard & 0x8000_0000 != 0);
        assert!(entry.validate_hash_guard());

        // Round-trip
        let bytes = entry.to_bytes();
        assert_eq!(bytes.len(), UPDATE_ENTRY_SIZE);

        let parsed = UpdateEntry::from_bytes(&bytes);
        assert_eq!(parsed.hash_guard, entry.hash_guard);
        assert_eq!(parsed.ekey, entry.ekey);
        assert_eq!(
            parsed.archive_location.archive_id,
            entry.archive_location.archive_id
        );
        assert_eq!(
            parsed.archive_location.archive_offset,
            entry.archive_location.archive_offset
        );
        assert_eq!(parsed.encoded_size, entry.encoded_size);
        assert_eq!(parsed.status, entry.status);
        assert!(parsed.validate_hash_guard());
    }

    #[test]
    fn test_update_entry_status_values() {
        for &(status, expected_byte) in &[
            (UpdateStatus::Normal, 0u8),
            (UpdateStatus::Delete, 3),
            (UpdateStatus::HeaderNonResident, 6),
            (UpdateStatus::DataNonResident, 7),
        ] {
            let entry = UpdateEntry::new(
                [1; 9],
                ArchiveLocation {
                    archive_id: 1,
                    archive_offset: 0,
                },
                100,
                status,
            );
            let bytes = entry.to_bytes();
            assert_eq!(bytes[22], expected_byte);
        }
    }

    #[test]
    fn test_update_page_fill_and_overflow() {
        let mut page = UpdatePage::new();
        assert!(page.is_empty());
        assert!(!page.is_full());

        // Fill the page (21 entries)
        for i in 0..ENTRIES_PER_PAGE {
            let entry = UpdateEntry::new(
                [i as u8; 9],
                ArchiveLocation {
                    archive_id: 0,
                    archive_offset: 0,
                },
                0,
                UpdateStatus::Normal,
            );
            assert!(page.push(entry), "should accept entry {i}");
        }

        assert!(page.is_full());
        assert_eq!(page.len(), ENTRIES_PER_PAGE);

        // Overflow should fail
        let overflow = UpdateEntry::new(
            [0xFF; 9],
            ArchiveLocation {
                archive_id: 0,
                archive_offset: 0,
            },
            0,
            UpdateStatus::Normal,
        );
        assert!(!page.push(overflow));
    }

    #[test]
    fn test_update_page_round_trip() {
        let mut page = UpdatePage::new();
        for i in 0..5 {
            let entry = UpdateEntry::new(
                [i as u8; 9],
                ArchiveLocation {
                    archive_id: i as u16,
                    archive_offset: i * 0x100,
                },
                i * 64,
                UpdateStatus::Normal,
            );
            page.push(entry);
        }

        let bytes = page.to_bytes();
        assert_eq!(bytes.len(), UPDATE_PAGE_SIZE);

        let parsed = UpdatePage::from_bytes(&bytes).expect("should parse non-empty page");
        assert_eq!(parsed.len(), 5);

        for (original, parsed_entry) in page.entries().iter().zip(parsed.entries()) {
            assert_eq!(original.ekey, parsed_entry.ekey);
            assert_eq!(
                original.archive_location.archive_id,
                parsed_entry.archive_location.archive_id
            );
            assert_eq!(original.encoded_size, parsed_entry.encoded_size);
        }
    }

    #[test]
    fn test_empty_page_returns_none() {
        let empty = [0u8; UPDATE_PAGE_SIZE];
        assert!(UpdatePage::from_bytes(&empty).is_none());
    }

    #[test]
    fn test_update_section_append_and_search() {
        let mut section = UpdateSection::new();
        assert_eq!(section.entry_count(), 0);

        let ekey1 = [0x11; 9];
        let ekey2 = [0x22; 9];

        // Add entries
        let e1 = UpdateEntry::new(
            ekey1,
            ArchiveLocation {
                archive_id: 1,
                archive_offset: 0x100,
            },
            1000,
            UpdateStatus::Normal,
        );
        assert!(section.append(e1));

        let e2 = UpdateEntry::new(
            ekey2,
            ArchiveLocation {
                archive_id: 2,
                archive_offset: 0x200,
            },
            2000,
            UpdateStatus::Normal,
        );
        assert!(section.append(e2));

        assert_eq!(section.entry_count(), 2);

        // Search finds entries
        let found = section.search(&ekey1).expect("should find ekey1");
        assert_eq!(found.encoded_size, 1000);

        let found = section.search(&ekey2).expect("should find ekey2");
        assert_eq!(found.encoded_size, 2000);

        // Missing key returns None
        assert!(section.search(&[0xFF; 9]).is_none());
    }

    #[test]
    fn test_update_section_newest_wins() {
        let mut section = UpdateSection::new();
        let ekey = [0xAA; 9];

        // Add entry with size 1000
        let e1 = UpdateEntry::new(
            ekey,
            ArchiveLocation {
                archive_id: 1,
                archive_offset: 0x100,
            },
            1000,
            UpdateStatus::Normal,
        );
        section.append(e1);

        // Add entry with same key but size 2000
        let e2 = UpdateEntry::new(
            ekey,
            ArchiveLocation {
                archive_id: 2,
                archive_offset: 0x200,
            },
            2000,
            UpdateStatus::Normal,
        );
        section.append(e2);

        // Search returns newest (size 2000)
        let found = section.search(&ekey).expect("should find");
        assert_eq!(found.encoded_size, 2000);
        assert_eq!(found.archive_location.archive_id, 2);
    }

    #[test]
    fn test_update_section_round_trip() {
        let mut section = UpdateSection::new();

        for i in 0..10u32 {
            let entry = UpdateEntry::new(
                [i as u8; 9],
                ArchiveLocation {
                    archive_id: i as u16,
                    archive_offset: i * 0x1000,
                },
                i * 100,
                UpdateStatus::Normal,
            );
            section.append(entry);
        }

        let bytes = section.to_bytes();
        let parsed = UpdateSection::from_bytes(&bytes);

        assert_eq!(parsed.entry_count(), 10);

        // Verify each entry survived
        for i in 0..10u32 {
            let ekey = [i as u8; 9];
            let found = parsed.search(&ekey).expect("should find entry");
            assert_eq!(found.encoded_size, i * 100);
        }
    }

    #[test]
    fn test_update_section_capacity() {
        let section = UpdateSection::with_capacity(UPDATE_PAGE_SIZE * 2);
        assert_eq!(section.capacity_pages(), 60); // min 60 pages enforced

        let mut section = UpdateSection::with_capacity(UPDATE_PAGE_SIZE * 100);
        assert_eq!(section.capacity_pages(), 100);

        // Fill all pages
        let entries_needed = 100 * ENTRIES_PER_PAGE;
        for i in 0..entries_needed {
            let mut ekey = [0u8; 9];
            ekey[0..4].copy_from_slice(&(i as u32).to_be_bytes());
            let entry = UpdateEntry::new(
                ekey,
                ArchiveLocation {
                    archive_id: 0,
                    archive_offset: 0,
                },
                0,
                UpdateStatus::Normal,
            );
            assert!(section.append(entry), "append should succeed for entry {i}");
        }

        assert!(section.is_full());

        // One more should fail
        let overflow = UpdateEntry::new(
            [0xFF; 9],
            ArchiveLocation {
                archive_id: 0,
                archive_offset: 0,
            },
            0,
            UpdateStatus::Normal,
        );
        assert!(!section.append(overflow));
    }

    #[test]
    fn test_update_section_sync_timing() {
        let mut section = UpdateSection::new();

        // Fill 8 pages (page indices 0-7)
        for page_idx in 0..8 {
            for slot in 0..ENTRIES_PER_PAGE {
                let i = page_idx * ENTRIES_PER_PAGE + slot;
                let mut ekey = [0u8; 9];
                ekey[0..4].copy_from_slice(&(i as u32).to_be_bytes());
                let entry = UpdateEntry::new(
                    ekey,
                    ArchiveLocation {
                        archive_id: 0,
                        archive_offset: 0,
                    },
                    0,
                    UpdateStatus::Normal,
                );
                section.append(entry);
            }
        }

        // After 8 pages (index 7), should_sync returns true
        assert_eq!(section.page_count(), 8);
        assert!(section.should_sync());
    }

    #[test]
    fn test_to_index_entry_conversion() {
        let update = UpdateEntry::new(
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x00],
            ArchiveLocation {
                archive_id: 42,
                archive_offset: 0x1234,
            },
            9999,
            UpdateStatus::Normal,
        );

        let index_entry = update.to_index_entry();
        assert_eq!(index_entry.key, update.ekey);
        assert_eq!(index_entry.archive_id(), 42);
        assert_eq!(index_entry.archive_offset(), 0x1234);
        assert_eq!(index_entry.size, 9999);
    }
}

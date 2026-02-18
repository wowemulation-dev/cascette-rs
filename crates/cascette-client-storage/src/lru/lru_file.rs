//! `.lru` file format with generation-based filenames.
//!
//! CASC persists LRU state to disk as a flat-file doubly-linked list.
//! The filename encodes the 64-bit generation counter as 16 big-endian
//! hex characters plus `.lru` extension (20 chars total).
//!
//! File layout:
//! - 28-byte header: version(2) + reserved(2) + MD5(16) + MRU head(4) + LRU tail(4)
//! - N × 20-byte entries: prev(4) + next(4) + ekey(9) + flags(1) + pad(2)
//!
use std::path::{Path, PathBuf};

/// LRU file header size in bytes.
pub const LRU_HEADER_SIZE: usize = 0x1C;

/// LRU entry size in bytes.
pub const LRU_ENTRY_SIZE: usize = 0x14;

/// Sentinel value for empty prev/next pointers and head/tail.
pub const LRU_SENTINEL: u32 = 0xFFFF_FFFF;

/// Maximum accepted LRU file version.
pub const LRU_MAX_VERSION: u16 = 1;

/// Current LRU file version written by cascette-rs.
pub const LRU_CURRENT_VERSION: u16 = 1;

/// LRU file extension.
pub const LRU_EXTENSION: &str = ".lru";

/// LRU filename length (16 hex chars + ".lru").
pub const LRU_FILENAME_LEN: usize = 20;

/// LRU file header.
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0x00   | 2    | Version (<= 1) |
/// | 0x02   | 2    | Reserved (zeroed for checksum) |
/// | 0x04   | 16   | MD5 hash of file (with hash field zeroed) |
/// | 0x14   | 4    | MRU head index |
/// | 0x18   | 4    | LRU tail index |
#[derive(Debug, Clone)]
pub struct LruFileHeader {
    /// File version (0 or 1).
    pub version: u16,
    /// MD5 hash of the file with this field zeroed during computation.
    pub hash: [u8; 16],
    /// Index of the most recently used entry (head of list).
    pub mru_head: u32,
    /// Index of the least recently used entry (tail of list).
    pub lru_tail: u32,
}

impl Default for LruFileHeader {
    fn default() -> Self {
        Self {
            version: LRU_CURRENT_VERSION,
            hash: [0; 16],
            mru_head: LRU_SENTINEL,
            lru_tail: LRU_SENTINEL,
        }
    }
}

impl LruFileHeader {
    /// Serialize the header to bytes.
    pub fn to_bytes(&self) -> [u8; LRU_HEADER_SIZE] {
        let mut buf = [0u8; LRU_HEADER_SIZE];
        buf[0..2].copy_from_slice(&self.version.to_le_bytes());
        // bytes 2..4 reserved (zero)
        buf[4..20].copy_from_slice(&self.hash);
        buf[20..24].copy_from_slice(&self.mru_head.to_le_bytes());
        buf[24..28].copy_from_slice(&self.lru_tail.to_le_bytes());
        buf
    }

    /// Parse a header from bytes.
    pub fn from_bytes(data: &[u8; LRU_HEADER_SIZE]) -> Option<Self> {
        let version = u16::from_le_bytes([data[0], data[1]]);
        if version > LRU_MAX_VERSION {
            return None;
        }

        let mut hash = [0u8; 16];
        hash.copy_from_slice(&data[4..20]);
        let mru_head = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let lru_tail = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

        Some(Self {
            version,
            hash,
            mru_head,
            lru_tail,
        })
    }
}

/// Single LRU file entry (20 bytes).
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0x00   | 4    | prev index (toward LRU tail) |
/// | 0x04   | 4    | next index (toward MRU head) |
/// | 0x08   | 9    | Encoding key (truncated) |
/// | 0x11   | 1    | Flags |
/// | 0x12   | 2    | Padding |
#[derive(Debug, Clone, Copy)]
pub struct LruFileEntry {
    /// Index of previous entry (toward LRU tail), or `LRU_SENTINEL`.
    pub prev: u32,
    /// Index of next entry (toward MRU head), or `LRU_SENTINEL`.
    pub next: u32,
    /// 9-byte truncated encoding key.
    pub ekey: [u8; 9],
    /// Entry flags.
    pub flags: u8,
}

impl LruFileEntry {
    /// Create an empty (unlinked) entry.
    pub const fn empty() -> Self {
        Self {
            prev: LRU_SENTINEL,
            next: LRU_SENTINEL,
            ekey: [0; 9],
            flags: 0,
        }
    }

    /// Serialize the entry to bytes.
    pub fn to_bytes(&self) -> [u8; LRU_ENTRY_SIZE] {
        let mut buf = [0u8; LRU_ENTRY_SIZE];
        buf[0..4].copy_from_slice(&self.prev.to_le_bytes());
        buf[4..8].copy_from_slice(&self.next.to_le_bytes());
        buf[8..17].copy_from_slice(&self.ekey);
        buf[17] = self.flags;
        // bytes 18..20 padding (zero)
        buf
    }

    /// Parse an entry from bytes.
    pub fn from_bytes(data: &[u8; LRU_ENTRY_SIZE]) -> Self {
        let prev = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let next = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&data[8..17]);
        let flags = data[17];

        Self {
            prev,
            next,
            ekey,
            flags,
        }
    }

    /// Check if this entry is in use (has a non-zero key).
    pub fn is_active(&self) -> bool {
        self.ekey != [0; 9]
    }
}

/// Build a `.lru` filename from a generation counter.
///
/// The generation is byte-swapped to big-endian, hex-encoded as 16 chars,
/// and `.lru` is appended. Result: `XXXXXXXXXXXXXXXX.lru`.
///
pub fn generation_to_filename(generation: u64) -> String {
    let be_bytes = generation.to_be_bytes();
    let mut name = String::with_capacity(LRU_FILENAME_LEN);
    for b in &be_bytes {
        let _ = std::fmt::Write::write_fmt(&mut name, format_args!("{b:02x}"));
    }
    name.push_str(LRU_EXTENSION);
    name
}

/// Parse a `.lru` filename back to a generation counter.
///
/// - Validates string length is exactly 20 chars
/// - Validates extension is `.lru`
/// - Parses 16 hex chars as big-endian 64-bit value
pub fn filename_to_generation(name: &str) -> Option<u64> {
    if name.len() != LRU_FILENAME_LEN {
        return None;
    }

    if !name.ends_with(LRU_EXTENSION) {
        return None;
    }

    let hex_part = &name[..16];
    let bytes = hex::decode(hex_part).ok()?;
    if bytes.len() != 8 {
        return None;
    }

    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes);
    Some(u64::from_be_bytes(arr))
}

/// Build the full path for a `.lru` file in the given directory.
pub fn lru_file_path(dir: &Path, generation: u64) -> PathBuf {
    dir.join(generation_to_filename(generation))
}

/// Validate an LRU file's size.
///
/// - Minimum size is 0x1C (header only)
/// - `(file_size - 0x1C) % 0x14 == 0` (entries are 20 bytes each)
pub const fn validate_file_size(size: usize) -> bool {
    size >= LRU_HEADER_SIZE && (size - LRU_HEADER_SIZE).is_multiple_of(LRU_ENTRY_SIZE)
}

/// Calculate the number of entries from a file size.
pub const fn entry_count_from_file_size(size: usize) -> usize {
    if size < LRU_HEADER_SIZE {
        0
    } else {
        (size - LRU_HEADER_SIZE) / LRU_ENTRY_SIZE
    }
}

/// Serialize a full LRU file (header + entries).
pub fn serialize(header: &LruFileHeader, entries: &[LruFileEntry]) -> Vec<u8> {
    let mut data = Vec::with_capacity(LRU_HEADER_SIZE + entries.len() * LRU_ENTRY_SIZE);

    // Write header with hash zeroed for computation
    let mut header_bytes = header.to_bytes();
    header_bytes[4..20].fill(0); // Zero hash field
    data.extend_from_slice(&header_bytes);

    // Write entries
    for entry in entries {
        data.extend_from_slice(&entry.to_bytes());
    }

    // Compute MD5 hash over file with hash field zeroed
    let hash = md5::compute(&data);
    data[4..20].copy_from_slice(&hash.0);

    data
}

/// Deserialize an LRU file from bytes.
///
/// Returns the header and entry array. Verifies the MD5 hash.
pub fn deserialize(data: &[u8]) -> Option<(LruFileHeader, Vec<LruFileEntry>)> {
    if !validate_file_size(data.len()) {
        return None;
    }

    // Parse header
    let header_bytes: &[u8; LRU_HEADER_SIZE] = data[..LRU_HEADER_SIZE].try_into().ok()?;
    let header = LruFileHeader::from_bytes(header_bytes)?;

    // Verify MD5 hash
    let mut check_data = data.to_vec();
    check_data[4..20].fill(0); // Zero hash field
    let computed = md5::compute(&check_data);
    if computed.0 != header.hash {
        return None;
    }

    // Parse entries
    let entry_count = entry_count_from_file_size(data.len());
    let mut entries = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let offset = LRU_HEADER_SIZE + i * LRU_ENTRY_SIZE;
        let entry_bytes: &[u8; LRU_ENTRY_SIZE] =
            data[offset..offset + LRU_ENTRY_SIZE].try_into().ok()?;
        entries.push(LruFileEntry::from_bytes(entry_bytes));
    }

    Some((header, entries))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_to_filename() {
        assert_eq!(generation_to_filename(1), "0000000000000001.lru");
        assert_eq!(
            generation_to_filename(0x0123_4567_89AB_CDEF),
            "0123456789abcdef.lru"
        );
        assert_eq!(generation_to_filename(u64::MAX), "ffffffffffffffff.lru");
    }

    #[test]
    fn test_filename_to_generation() {
        assert_eq!(filename_to_generation("0000000000000001.lru"), Some(1));
        assert_eq!(
            filename_to_generation("0123456789abcdef.lru"),
            Some(0x0123_4567_89AB_CDEF)
        );
        assert_eq!(
            filename_to_generation("ffffffffffffffff.lru"),
            Some(u64::MAX)
        );

        // Invalid
        assert_eq!(filename_to_generation("short.lru"), None);
        assert_eq!(filename_to_generation("0000000000000001.txt"), None);
        assert_eq!(filename_to_generation("zzzzzzzzzzzzzzzz.lru"), None);
    }

    #[test]
    fn test_filename_roundtrip() {
        for generation in [0, 1, 42, 0xDEAD_BEEF, u64::MAX] {
            let name = generation_to_filename(generation);
            assert_eq!(filename_to_generation(&name), Some(generation));
        }
    }

    #[test]
    fn test_validate_file_size() {
        assert!(!validate_file_size(0));
        assert!(!validate_file_size(27)); // less than header
        assert!(validate_file_size(28)); // header only
        assert!(!validate_file_size(29)); // misaligned
        assert!(validate_file_size(48)); // header + 1 entry
        assert!(validate_file_size(68)); // header + 2 entries
    }

    #[test]
    fn test_entry_count() {
        assert_eq!(entry_count_from_file_size(28), 0);
        assert_eq!(entry_count_from_file_size(48), 1);
        assert_eq!(entry_count_from_file_size(68), 2);
    }

    #[test]
    fn test_header_roundtrip() {
        let header = LruFileHeader {
            version: 1,
            hash: [0xAA; 16],
            mru_head: 0,
            lru_tail: 5,
        };
        let bytes = header.to_bytes();
        let parsed = LruFileHeader::from_bytes(&bytes).expect("parse");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.mru_head, 0);
        assert_eq!(parsed.lru_tail, 5);
    }

    #[test]
    fn test_entry_roundtrip() {
        let entry = LruFileEntry {
            prev: 3,
            next: 7,
            ekey: [1, 2, 3, 4, 5, 6, 7, 8, 9],
            flags: 0x42,
        };
        let bytes = entry.to_bytes();
        let parsed = LruFileEntry::from_bytes(&bytes);
        assert_eq!(parsed.prev, 3);
        assert_eq!(parsed.next, 7);
        assert_eq!(parsed.ekey, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(parsed.flags, 0x42);
    }

    #[test]
    fn test_serialize_deserialize() {
        let header = LruFileHeader::default();
        let entries = vec![
            LruFileEntry {
                prev: LRU_SENTINEL,
                next: 1,
                ekey: [0xAA; 9],
                flags: 0,
            },
            LruFileEntry {
                prev: 0,
                next: LRU_SENTINEL,
                ekey: [0xBB; 9],
                flags: 0,
            },
        ];

        let data = serialize(&header, &entries);
        assert_eq!(data.len(), LRU_HEADER_SIZE + 2 * LRU_ENTRY_SIZE);

        let (parsed_header, parsed_entries) = deserialize(&data).expect("deserialize");
        assert_eq!(parsed_header.version, LRU_CURRENT_VERSION);
        assert_eq!(parsed_header.mru_head, LRU_SENTINEL);
        assert_eq!(parsed_header.lru_tail, LRU_SENTINEL);
        assert_eq!(parsed_entries.len(), 2);
        assert_eq!(parsed_entries[0].ekey, [0xAA; 9]);
        assert_eq!(parsed_entries[1].ekey, [0xBB; 9]);
    }

    #[test]
    fn test_deserialize_rejects_bad_hash() {
        let header = LruFileHeader::default();
        let entries = vec![LruFileEntry::empty()];
        let mut data = serialize(&header, &entries);

        // Corrupt the hash
        data[4] ^= 0xFF;
        assert!(deserialize(&data).is_none());
    }

    #[test]
    fn test_deserialize_rejects_bad_version() {
        let mut header = LruFileHeader::default();
        header.version = 2; // Invalid
        let data = serialize(&header, &[]);
        // The serialize will compute a valid hash, but from_bytes rejects version > 1
        assert!(deserialize(&data).is_none());
    }

    #[test]
    fn test_empty_entry() {
        let entry = LruFileEntry::empty();
        assert!(!entry.is_active());
        assert_eq!(entry.prev, LRU_SENTINEL);
        assert_eq!(entry.next, LRU_SENTINEL);
    }

    #[test]
    fn test_lru_file_path() {
        let path = lru_file_path(Path::new("/data"), 42);
        assert_eq!(path, PathBuf::from("/data/000000000000002a.lru"));
    }
}

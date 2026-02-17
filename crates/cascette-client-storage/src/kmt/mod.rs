//! Key Mapping Table (KMT) v8.
//!
//! The KMT is the primary on-disk key-to-location structure.
//! It uses a two-tier LSM-tree design:
//!
//! - **Sorted sections**: 0x20-byte buckets, binary-searchable
//! - **Update sections**: 0x400-byte pages with 0x19 entries each,
//!   minimum 0x7800 bytes
//!
//! Jenkins lookup3 hash distributes keys across buckets.
//!
//! # KMT Entry Format (16 bytes)
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0x00   | 4    | packed_offset (bits 0-29: segment offset, bits 30-31: flags) |
//! | 0x04   | 4    | segment_id (archive segment index, shifted left 2) |
//! | 0x08   | 8    | key_hash (content key or key hash) |

pub mod key_state;
pub mod kmt_file;

/// KMT entry size in bytes.
pub const KMT_ENTRY_SIZE: usize = 0x10;

/// KMT entry representing a key-to-location mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KmtEntry {
    /// Packed offset: bits 0-29 are the segment offset, bits 30-31 are flags.
    pub packed_offset: u32,
    /// Segment ID (archive segment index).
    pub segment_id: u32,
    /// Key hash (content key or truncated hash).
    pub key_hash: u64,
}

impl KmtEntry {
    /// Get the segment offset (bits 0-29).
    pub const fn segment_offset(&self) -> u32 {
        self.packed_offset & 0x3FFF_FFFF
    }

    /// Get the flags (bits 30-31).
    pub const fn flags(&self) -> u8 {
        (self.packed_offset >> 30) as u8
    }

    /// Create a new KMT entry.
    pub const fn new(segment_offset: u32, flags: u8, segment_id: u32, key_hash: u64) -> Self {
        let packed_offset = (segment_offset & 0x3FFF_FFFF) | ((flags as u32) << 30);
        Self {
            packed_offset,
            segment_id,
            key_hash,
        }
    }

    /// Serialize to 16 bytes.
    pub fn to_bytes(&self) -> [u8; KMT_ENTRY_SIZE] {
        let mut buf = [0u8; KMT_ENTRY_SIZE];
        buf[0..4].copy_from_slice(&self.packed_offset.to_le_bytes());
        buf[4..8].copy_from_slice(&self.segment_id.to_le_bytes());
        buf[8..16].copy_from_slice(&self.key_hash.to_le_bytes());
        buf
    }

    /// Parse from 16 bytes.
    pub fn from_bytes(data: &[u8; KMT_ENTRY_SIZE]) -> Self {
        let packed_offset = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let segment_id = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let key_hash = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        Self {
            packed_offset,
            segment_id,
            key_hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmt_entry_round_trip() {
        let entry = KmtEntry::new(0x1234_5678, 2, 42, 0xDEAD_BEEF_CAFE_BABE);
        let bytes = entry.to_bytes();
        let parsed = KmtEntry::from_bytes(&bytes);
        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_kmt_entry_fields() {
        let entry = KmtEntry::new(0x0ABC_DEF0, 3, 7, 0x1122_3344_5566_7788);
        assert_eq!(entry.segment_offset(), 0x0ABC_DEF0);
        assert_eq!(entry.flags(), 3);
        assert_eq!(entry.segment_id, 7);
        assert_eq!(entry.key_hash, 0x1122_3344_5566_7788);
    }

    #[test]
    fn test_offset_mask() {
        // Offset is 30 bits, should be masked
        let entry = KmtEntry::new(0xFFFF_FFFF, 0, 0, 0);
        assert_eq!(entry.segment_offset(), 0x3FFF_FFFF);
    }
}

//! Patch Index entry types
//!
//! Each entry maps a patch blob (identified by its EKey) to the source and
//! target files it transforms. This is used by the TACT client to determine
//! which patches are available for a given file.

/// A patch index entry from block type 2
///
/// Maps a patch blob to its source and target file information.
///
/// Binary layout (with key_size=16, 61 bytes total):
/// ```text
/// source_ekey:     [u8; key_size]     Source file encoding key
/// source_size:     u32 LE             Source file decoded size
/// target_ekey:     [u8; key_size]     Target file encoding key
/// target_size:     u32 LE             Target file decoded size
/// encoded_size:    u32 LE             Encoded (compressed) size
/// suffix_offset:   u8                 EKey suffix table offset (unused when table is empty)
/// patch_ekey:      [u8; key_size]     Patch blob encoding key (map lookup key)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchIndexEntry {
    /// Source (old) file encoding key
    pub source_ekey: [u8; 16],

    /// Source file decoded size
    pub source_size: u32,

    /// Target (new) file encoding key
    pub target_ekey: [u8; 16],

    /// Target file decoded size
    pub target_size: u32,

    /// Encoded (compressed) size of the target
    pub encoded_size: u32,

    /// EKey suffix table offset (0 or 1 when table is empty)
    pub suffix_offset: u8,

    /// Patch blob encoding key — identifies the actual patch data on CDN
    pub patch_ekey: [u8; 16],
}

/// Entry size in bytes for a given key size
///
/// `3 * key_size + 4 + 4 + 4 + 1` = with key_size=16: 61 bytes
pub const fn entry_size(key_size: u8) -> usize {
    3 * key_size as usize + 13
}

impl PatchIndexEntry {
    /// Parse a single entry from a byte slice
    ///
    /// Returns the entry and the number of bytes consumed.
    pub fn parse(data: &[u8], key_size: u8) -> Option<Self> {
        let size = entry_size(key_size);
        if data.len() < size {
            return None;
        }

        let ks = key_size as usize;
        let mut pos = 0;

        let mut source_ekey = [0u8; 16];
        source_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let source_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let mut target_ekey = [0u8; 16];
        target_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let target_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let encoded_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let suffix_offset = data[pos];
        pos += 1;

        let mut patch_ekey = [0u8; 16];
        patch_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);

        Some(Self {
            source_ekey,
            source_size,
            target_ekey,
            target_size,
            encoded_size,
            suffix_offset,
            patch_ekey,
        })
    }

    /// Serialize this entry to bytes
    pub fn build(&self, key_size: u8) -> Vec<u8> {
        let ks = key_size as usize;
        let mut out = Vec::with_capacity(entry_size(key_size));

        out.extend_from_slice(&self.source_ekey[..ks]);
        out.extend_from_slice(&self.source_size.to_le_bytes());
        out.extend_from_slice(&self.target_ekey[..ks]);
        out.extend_from_slice(&self.target_size.to_le_bytes());
        out.extend_from_slice(&self.encoded_size.to_le_bytes());
        out.push(self.suffix_offset);
        out.extend_from_slice(&self.patch_ekey[..ks]);

        out
    }
}

/// A V2 patch index entry from block type 6
///
/// V2 entries include an ESpec string table offset and separate fields for
/// original file metadata. The ESpec table is stored once per block and
/// entries reference offsets into it.
///
/// Binary layout (with key_size=16):
/// ```text
/// target_ekey:          [u8; key_size]
/// espec_offset:         u32 LE         Offset into shared ESpec string
/// decoded_size:         u32 LE         Target decoded size
/// patch_ekey:           [u8; key_size]
/// patch_encoded_size:   u32 LE         Patch encoded size
/// patch_decoded_size:   u32 LE         Patch decoded size
/// original_ekey_offset: u32 LE         Offset for original EKey lookup
/// original_ekey:        [u8; key_size]
/// original_size:        u32 LE         Original file decoded size
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchIndexEntryV2 {
    /// Target file encoding key
    pub target_ekey: [u8; 16],
    /// Offset into the shared ESpec string table
    pub espec_offset: u32,
    /// Target file decoded size
    pub decoded_size: u32,
    /// Patch blob encoding key
    pub patch_ekey: [u8; 16],
    /// Patch encoded (compressed) size
    pub patch_encoded_size: u32,
    /// Patch decoded size
    pub patch_decoded_size: u32,
    /// Offset for original EKey lookup
    pub original_ekey_offset: u32,
    /// Original (source) file encoding key
    pub original_ekey: [u8; 16],
    /// Original file decoded size
    pub original_size: u32,
}

/// V2 entry size: 3 * key_size + 6 * 4 = with key_size=16: 72 bytes
pub const fn entry_v2_size(key_size: u8) -> usize {
    3 * key_size as usize + 24
}

impl PatchIndexEntryV2 {
    /// Parse a V2 entry from a byte slice
    pub fn parse(data: &[u8], key_size: u8) -> Option<Self> {
        let size = entry_v2_size(key_size);
        if data.len() < size {
            return None;
        }

        let ks = key_size as usize;
        let mut pos = 0;

        let mut target_ekey = [0u8; 16];
        target_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let espec_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let decoded_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let mut patch_ekey = [0u8; 16];
        patch_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let patch_encoded_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let patch_decoded_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let original_ekey_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let mut original_ekey = [0u8; 16];
        original_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let original_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);

        Some(Self {
            target_ekey,
            espec_offset,
            decoded_size,
            patch_ekey,
            patch_encoded_size,
            patch_decoded_size,
            original_ekey_offset,
            original_ekey,
            original_size,
        })
    }

    /// Serialize this V2 entry to bytes
    pub fn build(&self, key_size: u8) -> Vec<u8> {
        let ks = key_size as usize;
        let mut out = Vec::with_capacity(entry_v2_size(key_size));

        out.extend_from_slice(&self.target_ekey[..ks]);
        out.extend_from_slice(&self.espec_offset.to_le_bytes());
        out.extend_from_slice(&self.decoded_size.to_le_bytes());
        out.extend_from_slice(&self.patch_ekey[..ks]);
        out.extend_from_slice(&self.patch_encoded_size.to_le_bytes());
        out.extend_from_slice(&self.patch_decoded_size.to_le_bytes());
        out.extend_from_slice(&self.original_ekey_offset.to_le_bytes());
        out.extend_from_slice(&self.original_ekey[..ks]);
        out.extend_from_slice(&self.original_size.to_le_bytes());

        out
    }
}

/// A V3 patch index entry from block type 10
///
/// V3 extends V2 with a flags byte that controls which fields are present.
/// Fields controlled by flags default to zero when absent.
///
/// Flags:
/// - bit 0: `has_original_ekey_offset`
/// - bit 1: `has_original_ekey`
/// - bit 2: `has_decoded_size`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchIndexEntryV3 {
    /// The V2 base fields
    pub base: PatchIndexEntryV2,
    /// Flags byte controlling which conditional fields are present
    pub flags: u8,
}

/// V3 flags: original_ekey_offset field is present
pub const V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET: u8 = 0x01;
/// V3 flags: original_ekey field is present
pub const V3_FLAG_HAS_ORIGINAL_EKEY: u8 = 0x02;
/// V3 flags: decoded_size field is present
pub const V3_FLAG_HAS_DECODED_SIZE: u8 = 0x04;

impl PatchIndexEntryV3 {
    /// Compute the entry size for a given key_size and flags
    pub fn entry_size(key_size: u8, flags: u8) -> usize {
        let ks = key_size as usize;
        // Always present: target_ekey + espec_offset + patch_ekey + patch_encoded_size + patch_decoded_size
        let mut size = ks + 4 + ks + 4 + 4;

        if flags & V3_FLAG_HAS_DECODED_SIZE != 0 {
            size += 4;
        }
        if flags & V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET != 0 {
            size += 4;
        }
        if flags & V3_FLAG_HAS_ORIGINAL_EKEY != 0 {
            size += ks;
        }
        // original_size is always present when original_ekey is present
        if flags & V3_FLAG_HAS_ORIGINAL_EKEY != 0 {
            size += 4;
        }

        size
    }

    /// Parse a V3 entry from a byte slice with the given flags
    pub fn parse(data: &[u8], key_size: u8, flags: u8) -> Option<Self> {
        let size = Self::entry_size(key_size, flags);
        if data.len() < size {
            return None;
        }

        let ks = key_size as usize;
        let mut pos = 0;

        let mut target_ekey = [0u8; 16];
        target_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let espec_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let decoded_size = if flags & V3_FLAG_HAS_DECODED_SIZE != 0 {
            let v = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;
            v
        } else {
            0
        };

        let mut patch_ekey = [0u8; 16];
        patch_ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
        pos += ks;

        let patch_encoded_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let patch_decoded_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        let original_ekey_offset = if flags & V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET != 0 {
            let v = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;
            v
        } else {
            0
        };

        let (original_ekey, original_size) = if flags & V3_FLAG_HAS_ORIGINAL_EKEY != 0 {
            let mut ekey = [0u8; 16];
            ekey[..ks].copy_from_slice(&data[pos..pos + ks]);
            pos += ks;
            let size = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            (ekey, size)
        } else {
            ([0u8; 16], 0u32)
        };
        // Suppress unused variable warning — pos is logically consumed
        let _ = pos;

        Some(Self {
            base: PatchIndexEntryV2 {
                target_ekey,
                espec_offset,
                decoded_size,
                patch_ekey,
                patch_encoded_size,
                patch_decoded_size,
                original_ekey_offset,
                original_ekey,
                original_size,
            },
            flags,
        })
    }

    /// Serialize this V3 entry to bytes
    pub fn build(&self, key_size: u8) -> Vec<u8> {
        let ks = key_size as usize;
        let mut out = Vec::with_capacity(Self::entry_size(key_size, self.flags));

        out.extend_from_slice(&self.base.target_ekey[..ks]);
        out.extend_from_slice(&self.base.espec_offset.to_le_bytes());

        if self.flags & V3_FLAG_HAS_DECODED_SIZE != 0 {
            out.extend_from_slice(&self.base.decoded_size.to_le_bytes());
        }

        out.extend_from_slice(&self.base.patch_ekey[..ks]);
        out.extend_from_slice(&self.base.patch_encoded_size.to_le_bytes());
        out.extend_from_slice(&self.base.patch_decoded_size.to_le_bytes());

        if self.flags & V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET != 0 {
            out.extend_from_slice(&self.base.original_ekey_offset.to_le_bytes());
        }

        if self.flags & V3_FLAG_HAS_ORIGINAL_EKEY != 0 {
            out.extend_from_slice(&self.base.original_ekey[..ks]);
            out.extend_from_slice(&self.base.original_size.to_le_bytes());
        }

        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_size() {
        assert_eq!(entry_size(16), 61);
        assert_eq!(entry_size(9), 40);
    }

    #[test]
    fn test_entry_round_trip() {
        let entry = PatchIndexEntry {
            source_ekey: [0x01; 16],
            source_size: 1000,
            target_ekey: [0x02; 16],
            target_size: 2000,
            encoded_size: 1500,
            suffix_offset: 1,
            patch_ekey: [0x03; 16],
        };

        let built = entry.build(16);
        assert_eq!(built.len(), 61);

        let reparsed = PatchIndexEntry::parse(&built, 16).unwrap();
        assert_eq!(reparsed, entry);
    }

    #[test]
    fn test_entry_parse_too_short() {
        let data = [0u8; 60]; // 1 byte short
        assert!(PatchIndexEntry::parse(&data, 16).is_none());
    }

    #[test]
    fn test_v2_entry_size() {
        assert_eq!(entry_v2_size(16), 72);
        assert_eq!(entry_v2_size(9), 51);
    }

    #[test]
    fn test_v2_entry_round_trip() {
        let entry = PatchIndexEntryV2 {
            target_ekey: [0x01; 16],
            espec_offset: 42,
            decoded_size: 2000,
            patch_ekey: [0x02; 16],
            patch_encoded_size: 1500,
            patch_decoded_size: 1800,
            original_ekey_offset: 10,
            original_ekey: [0x03; 16],
            original_size: 1000,
        };

        let built = entry.build(16);
        assert_eq!(built.len(), 72);

        let reparsed = PatchIndexEntryV2::parse(&built, 16).unwrap();
        assert_eq!(reparsed, entry);
    }

    #[test]
    fn test_v3_entry_round_trip_all_flags() {
        let flags =
            V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET | V3_FLAG_HAS_ORIGINAL_EKEY | V3_FLAG_HAS_DECODED_SIZE;
        let entry = PatchIndexEntryV3 {
            base: PatchIndexEntryV2 {
                target_ekey: [0x11; 16],
                espec_offset: 5,
                decoded_size: 3000,
                patch_ekey: [0x22; 16],
                patch_encoded_size: 2500,
                patch_decoded_size: 2800,
                original_ekey_offset: 20,
                original_ekey: [0x33; 16],
                original_size: 1500,
            },
            flags,
        };

        let built = entry.build(16);
        let reparsed = PatchIndexEntryV3::parse(&built, 16, flags).unwrap();
        assert_eq!(reparsed, entry);
    }

    #[test]
    fn test_v3_entry_round_trip_no_flags() {
        let flags = 0u8;
        let entry = PatchIndexEntryV3 {
            base: PatchIndexEntryV2 {
                target_ekey: [0x44; 16],
                espec_offset: 0,
                decoded_size: 0,
                patch_ekey: [0x55; 16],
                patch_encoded_size: 100,
                patch_decoded_size: 200,
                original_ekey_offset: 0,
                original_ekey: [0u8; 16],
                original_size: 0,
            },
            flags,
        };

        let built = entry.build(16);
        // Minimal: target_ekey(16) + espec_offset(4) + patch_ekey(16) + enc(4) + dec(4) = 44
        assert_eq!(built.len(), 44);
        let reparsed = PatchIndexEntryV3::parse(&built, 16, flags).unwrap();
        assert_eq!(reparsed, entry);
    }
}

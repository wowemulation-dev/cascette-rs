//! Patch Index entry types
//!
//! Each entry maps a patch blob (identified by its EKey) to the source and
//! target files it transforms. This is used by the TACT client to determine
//! which patches are available for a given file.

/// A patch index entry from block type 2
///
/// Maps a patch blob to its source and target file information.
/// Agent.exe `ParseBlock2` at 0x6a4a51 reads these into `PatchEntryMap`.
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
}

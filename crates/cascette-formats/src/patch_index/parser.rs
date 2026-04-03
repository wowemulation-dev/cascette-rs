//! Patch Index parser
//!
//! Parses the block-based patch index format. The main entry data lives
//! in block type 2. Block type 8 contains a secondary copy with an extended
//! header.
//!
//! Block types 6 (V2) and 10 (V3) use a different entry format with
//! suffix tables and conditional fields. These are parsed if present but
//! all known CDN files use block types 1, 2, and 8.

use super::entry::{
    PatchIndexEntry, PatchIndexEntryV2, PatchIndexEntryV3, entry_size, entry_v2_size,
};
use super::error::{PatchIndexError, PatchIndexResult};
use super::header::PatchIndexHeader;

/// Block type 1: Configuration/skip block (processed but no entries extracted).
pub const BLOCK_TYPE_SKIP: u32 = 1;

/// Block type 2: Key-pair entries (main entry block)
pub const BLOCK_TYPE_ENTRIES: u32 = 2;

/// Block type 6: V2 entries with ESpec string table
pub const BLOCK_TYPE_V2: u32 = 6;

/// Block type 8: Extended entry block (secondary copy with larger header)
pub const BLOCK_TYPE_EXTENDED: u32 = 8;

/// Block type 10: V3 entries with flags and conditional fields
pub const BLOCK_TYPE_V3: u32 = 10;

/// Parse block type 2 entries from raw data
///
/// Block layout:
/// ```text
/// u32 LE  entry_count
/// u8      key_size
/// [PatchIndexEntry; entry_count]
/// ```
pub fn parse_block2(data: &[u8]) -> PatchIndexResult<(u8, Vec<PatchIndexEntry>)> {
    if data.len() < 5 {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_ENTRIES,
            needed: 5,
            available: data.len(),
        });
    }

    let entry_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let key_size = data[4];
    let esize = entry_size(key_size);

    let needed = 5 + entry_count * esize;
    if data.len() < needed {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_ENTRIES,
            needed,
            available: data.len(),
        });
    }

    let mut entries = Vec::with_capacity(entry_count);
    let mut pos = 5;
    for _ in 0..entry_count {
        let entry = PatchIndexEntry::parse(&data[pos..], key_size).ok_or_else(|| {
            PatchIndexError::EntryOverflow {
                block_type: BLOCK_TYPE_ENTRIES,
                needed: pos + esize,
                available: data.len(),
            }
        })?;
        entries.push(entry);
        pos += esize;
    }

    Ok((key_size, entries))
}

/// Parse block type 8 entries from raw data
///
/// Block 8 has a 14-byte header followed by entries in the same format
/// as block type 2. The header structure:
/// ```text
/// u8      version (3)
/// u8      key_size
/// u16 LE  data_offset (offset to entry data from block start)
/// u32 LE  entry_count
/// u32 LE  unknown (62 in all known files = entry_size + 1)
/// u8      unknown
/// u8      unknown
/// [PatchIndexEntry; entry_count]
/// ```
pub fn parse_block8(data: &[u8]) -> PatchIndexResult<(u8, Vec<PatchIndexEntry>)> {
    if data.len() < 14 {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_EXTENDED,
            needed: 14,
            available: data.len(),
        });
    }

    let version = data[0];
    if version != 3 {
        return Err(PatchIndexError::InvalidBlockVersion {
            block_type: BLOCK_TYPE_EXTENDED,
            expected: 3,
            actual: version,
        });
    }

    let key_size = data[1];
    let data_offset = u16::from_le_bytes([data[2], data[3]]) as usize;
    let entry_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let esize = entry_size(key_size);

    let needed = data_offset + entry_count * esize;
    if data.len() < needed {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_EXTENDED,
            needed,
            available: data.len(),
        });
    }

    let mut entries = Vec::with_capacity(entry_count);
    let mut pos = data_offset;
    for _ in 0..entry_count {
        let entry = PatchIndexEntry::parse(&data[pos..], key_size).ok_or_else(|| {
            PatchIndexError::EntryOverflow {
                block_type: BLOCK_TYPE_EXTENDED,
                needed: pos + esize,
                available: data.len(),
            }
        })?;
        entries.push(entry);
        pos += esize;
    }

    Ok((key_size, entries))
}

/// Parse block type 6 (V2) entries from raw data
///
/// Block layout:
/// ```text
/// u8      identifier (must be 2)
/// u8      key_size
/// u32 LE  entry_count
/// u32 LE  skip (unknown, 4 bytes)
/// u32 LE  espec_string_length
/// [u8; espec_string_length]  ESpec string table (UTF-8)
/// [PatchIndexEntryV2; entry_count]
/// ```
pub fn parse_block6(data: &[u8]) -> PatchIndexResult<(u8, Vec<PatchIndexEntryV2>, Option<String>)> {
    if data.len() < 14 {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_V2,
            needed: 14,
            available: data.len(),
        });
    }

    let identifier = data[0];
    if identifier != 2 {
        return Err(PatchIndexError::InvalidBlockVersion {
            block_type: BLOCK_TYPE_V2,
            expected: 2,
            actual: identifier,
        });
    }

    let key_size = data[1];
    let entry_count = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
    // Skip 4 bytes (unknown)
    let espec_len = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;

    let mut pos = 14;

    // Read ESpec string table
    let espec_table = if espec_len > 0 {
        if pos + espec_len > data.len() {
            return Err(PatchIndexError::EntryOverflow {
                block_type: BLOCK_TYPE_V2,
                needed: pos + espec_len,
                available: data.len(),
            });
        }
        let s = String::from_utf8_lossy(&data[pos..pos + espec_len]).into_owned();
        pos += espec_len;
        Some(s)
    } else {
        None
    };

    // Read entries
    let esize = entry_v2_size(key_size);
    let needed = pos + entry_count * esize;
    if data.len() < needed {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_V2,
            needed,
            available: data.len(),
        });
    }

    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let entry = PatchIndexEntryV2::parse(&data[pos..], key_size).ok_or_else(|| {
            PatchIndexError::EntryOverflow {
                block_type: BLOCK_TYPE_V2,
                needed: pos + esize,
                available: data.len(),
            }
        })?;
        entries.push(entry);
        pos += esize;
    }

    Ok((key_size, entries, espec_table))
}

/// Parse block type 10 (V3) entries from raw data
///
/// Same header as block 6 plus a flags byte controlling conditional fields.
///
/// Block layout:
/// ```text
/// u8      identifier (must be 2)
/// u8      key_size
/// u32 LE  entry_count
/// u32 LE  skip (unknown, 4 bytes)
/// u32 LE  espec_string_length
/// u8      flags
/// [u8; espec_string_length]  ESpec string table (UTF-8)
/// [PatchIndexEntryV3; entry_count]
/// ```
pub fn parse_block10(
    data: &[u8],
) -> PatchIndexResult<(u8, Vec<PatchIndexEntryV3>, Option<String>)> {
    if data.len() < 15 {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_V3,
            needed: 15,
            available: data.len(),
        });
    }

    let identifier = data[0];
    if identifier != 2 {
        return Err(PatchIndexError::InvalidBlockVersion {
            block_type: BLOCK_TYPE_V3,
            expected: 2,
            actual: identifier,
        });
    }

    let key_size = data[1];
    let entry_count = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
    // Skip 4 bytes (unknown)
    let espec_len = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
    let flags = data[14];

    let mut pos = 15;

    // Read ESpec string table
    let espec_table = if espec_len > 0 {
        if pos + espec_len > data.len() {
            return Err(PatchIndexError::EntryOverflow {
                block_type: BLOCK_TYPE_V3,
                needed: pos + espec_len,
                available: data.len(),
            });
        }
        let s = String::from_utf8_lossy(&data[pos..pos + espec_len]).into_owned();
        pos += espec_len;
        Some(s)
    } else {
        None
    };

    // Read entries
    let esize = PatchIndexEntryV3::entry_size(key_size, flags);
    let needed = pos + entry_count * esize;
    if data.len() < needed {
        return Err(PatchIndexError::EntryOverflow {
            block_type: BLOCK_TYPE_V3,
            needed,
            available: data.len(),
        });
    }

    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let entry = PatchIndexEntryV3::parse(&data[pos..], key_size, flags).ok_or_else(|| {
            PatchIndexError::EntryOverflow {
                block_type: BLOCK_TYPE_V3,
                needed: pos + esize,
                available: data.len(),
            }
        })?;
        entries.push(entry);
        pos += esize;
    }

    Ok((key_size, entries, espec_table))
}

/// Parsed patch index result including V2/V3 entries
pub struct ParsedPatchIndex {
    /// File header
    pub header: PatchIndexHeader,
    /// Entry key size
    pub key_size: u8,
    /// V1 entries from block type 2
    pub entries: Vec<PatchIndexEntry>,
    /// V2 entries from block type 6
    pub entries_v2: Vec<PatchIndexEntryV2>,
    /// V3 entries from block type 10
    pub entries_v3: Vec<PatchIndexEntryV3>,
    /// Shared ESpec string table from V2/V3 blocks
    pub espec_table: Option<String>,
}

/// Parse a complete patch index from raw data
///
/// Extracts entries from block type 2 (primary). If block type 8 is
/// present and block type 2 is not, falls back to block type 8.
/// Also parses V2 (block 6) and V3 (block 10) entries if present.
pub fn parse_patch_index(
    data: &[u8],
) -> PatchIndexResult<(PatchIndexHeader, u8, Vec<PatchIndexEntry>)> {
    let parsed = parse_patch_index_full(data)?;
    Ok((parsed.header, parsed.key_size, parsed.entries))
}

/// Parse a complete patch index including V2/V3 entries
pub fn parse_patch_index_full(data: &[u8]) -> PatchIndexResult<ParsedPatchIndex> {
    let header = PatchIndexHeader::parse(data)?;

    // Validate data_size matches actual length
    if header.data_size as usize != data.len() {
        return Err(PatchIndexError::DataSizeMismatch {
            declared: header.data_size,
            actual: data.len(),
        });
    }

    let mut key_size = 16u8;
    let mut entries = Vec::new();
    let mut entries_v2 = Vec::new();
    let mut entries_v3 = Vec::new();
    let mut espec_table = None;
    let mut found_block2 = false;

    for (i, desc) in header.blocks.iter().enumerate() {
        let offset = header.block_offset(i) as usize;
        let block_data = &data[offset..offset + desc.block_size as usize];

        match desc.block_type {
            BLOCK_TYPE_ENTRIES => {
                let (ks, ents) = parse_block2(block_data)?;
                key_size = ks;
                entries = ents;
                found_block2 = true;
            }
            BLOCK_TYPE_V2 => {
                let (ks, ents, espec) = parse_block6(block_data)?;
                key_size = ks;
                entries_v2 = ents;
                if espec.is_some() {
                    espec_table = espec;
                }
            }
            BLOCK_TYPE_EXTENDED => {
                // Only use block 8 if block 2 was not found
                if !found_block2 {
                    let (ks, ents) = parse_block8(block_data)?;
                    key_size = ks;
                    entries = ents;
                }
            }
            BLOCK_TYPE_V3 => {
                let (ks, ents, espec) = parse_block10(block_data)?;
                key_size = ks;
                entries_v3 = ents;
                if espec.is_some() {
                    espec_table = espec;
                }
            }
            // Block type 1 (config) and unknown types are skipped.
            // Agent.exe logs "Unknown Patch Index block. BlockId:%u"
            // for unrecognized types but does not fail.
            _ => {}
        }
    }

    Ok(ParsedPatchIndex {
        header,
        key_size,
        entries,
        entries_v2,
        entries_v3,
        espec_table,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_block2_empty() {
        // entry_count=0, key_size=16
        let data = [0, 0, 0, 0, 16];
        let (ks, entries) = parse_block2(&data).unwrap();
        assert_eq!(ks, 16);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_block2_single_entry() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes()); // entry_count
        data.push(16); // key_size

        let entry = PatchIndexEntry {
            source_ekey: [0xAA; 16],
            source_size: 100,
            target_ekey: [0xBB; 16],
            target_size: 200,
            encoded_size: 150,
            suffix_offset: 1,
            patch_ekey: [0xCC; 16],
        };
        data.extend_from_slice(&entry.build(16));

        let (ks, entries) = parse_block2(&data).unwrap();
        assert_eq!(ks, 16);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry);
    }

    #[test]
    fn test_parse_block6_empty() {
        let mut data = Vec::new();
        data.push(2); // identifier
        data.push(16); // key_size
        data.extend_from_slice(&0u32.to_le_bytes()); // entry_count
        data.extend_from_slice(&0u32.to_le_bytes()); // skip
        data.extend_from_slice(&0u32.to_le_bytes()); // espec_len

        let (ks, entries, espec) = parse_block6(&data).unwrap();
        assert_eq!(ks, 16);
        assert!(entries.is_empty());
        assert!(espec.is_none());
    }

    #[test]
    fn test_parse_block6_with_espec_and_entry() {
        let mut data = Vec::new();
        data.push(2); // identifier
        data.push(16); // key_size
        data.extend_from_slice(&1u32.to_le_bytes()); // entry_count
        data.extend_from_slice(&0u32.to_le_bytes()); // skip
        let espec_str = b"b:{*=z}";
        data.extend_from_slice(&(espec_str.len() as u32).to_le_bytes()); // espec_len
        data.extend_from_slice(espec_str);

        let entry = PatchIndexEntryV2 {
            target_ekey: [0x11; 16],
            espec_offset: 0,
            decoded_size: 2000,
            patch_ekey: [0x22; 16],
            patch_encoded_size: 1500,
            patch_decoded_size: 1800,
            original_ekey_offset: 0,
            original_ekey: [0x33; 16],
            original_size: 1000,
        };
        data.extend_from_slice(&entry.build(16));

        let (ks, entries, espec) = parse_block6(&data).unwrap();
        assert_eq!(ks, 16);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry);
        assert_eq!(espec.unwrap(), "b:{*=z}");
    }

    #[test]
    fn test_parse_block10_with_all_flags() {
        use crate::patch_index::entry::{
            V3_FLAG_HAS_DECODED_SIZE, V3_FLAG_HAS_ORIGINAL_EKEY, V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET,
        };

        let flags =
            V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET | V3_FLAG_HAS_ORIGINAL_EKEY | V3_FLAG_HAS_DECODED_SIZE;

        let mut data = Vec::new();
        data.push(2); // identifier
        data.push(16); // key_size
        data.extend_from_slice(&1u32.to_le_bytes()); // entry_count
        data.extend_from_slice(&0u32.to_le_bytes()); // skip
        data.extend_from_slice(&0u32.to_le_bytes()); // espec_len = 0
        data.push(flags);

        let entry = PatchIndexEntryV3 {
            base: PatchIndexEntryV2 {
                target_ekey: [0xAA; 16],
                espec_offset: 5,
                decoded_size: 3000,
                patch_ekey: [0xBB; 16],
                patch_encoded_size: 2500,
                patch_decoded_size: 2800,
                original_ekey_offset: 20,
                original_ekey: [0xCC; 16],
                original_size: 1500,
            },
            flags,
        };
        data.extend_from_slice(&entry.build(16));

        let (ks, entries, espec) = parse_block10(&data).unwrap();
        assert_eq!(ks, 16);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry);
        assert!(espec.is_none());
    }

    #[test]
    fn test_parse_block6_invalid_identifier() {
        let mut data = Vec::new();
        data.push(1); // wrong identifier (should be 2)
        data.push(16);
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let result = parse_block6(&data);
        assert!(result.is_err());
    }
}

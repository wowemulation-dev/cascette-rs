//! Patch Index parser
//!
//! Parses the block-based patch index format. The main entry data lives
//! in block type 2 (`ParseBlock2` at 0x6a4a51). Block type 8 contains
//! a secondary copy with an extended header.
//!
//! Block types 6 (V2) and 10 (V3) use a different entry format with
//! suffix tables and conditional fields. These are parsed if present but
//! all known CDN files use block types 1, 2, and 8.

use super::entry::{PatchIndexEntry, entry_size};
use super::error::{PatchIndexError, PatchIndexResult};
use super::header::PatchIndexHeader;

/// Block type 1: Configuration/skip block
///
/// Agent.exe `SkipBlock1` at 0x6a4ec6 processes this in the first pass
/// but does not extract entries from it.
pub const BLOCK_TYPE_SKIP: u32 = 1;

/// Block type 2: Key-pair entries (main entry block)
pub const BLOCK_TYPE_ENTRIES: u32 = 2;

/// Block type 8: Extended entry block (secondary copy with larger header)
pub const BLOCK_TYPE_EXTENDED: u32 = 8;

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

/// Parse a complete patch index from raw data
///
/// Extracts entries from block type 2 (primary). If block type 8 is
/// present and block type 2 is not, falls back to block type 8.
pub fn parse_patch_index(
    data: &[u8],
) -> PatchIndexResult<(PatchIndexHeader, u8, Vec<PatchIndexEntry>)> {
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
            BLOCK_TYPE_EXTENDED => {
                // Only use block 8 if block 2 was not found
                if !found_block2 {
                    let (ks, ents) = parse_block8(block_data)?;
                    key_size = ks;
                    entries = ents;
                }
            }
            // Block type 1 (config) and unknown types are skipped.
            // Agent.exe logs "Unknown Patch Index block. BlockId:%u"
            // for unrecognized types but does not fail.
            _ => {}
        }
    }

    Ok((header, key_size, entries))
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
}

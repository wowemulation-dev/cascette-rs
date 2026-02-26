//! Patch Index builder
//!
//! Builds a patch index binary blob from entries. Produces block types
//! 1 (config), 2 (entries), and 8 (extended copy) matching the layout
//! observed in real CDN files.

use super::entry::{PatchIndexEntry, entry_size};
use super::error::PatchIndexResult;
use super::header::{BlockDescriptor, PatchIndexHeader};

/// Standard block type 1 (configuration) content
///
/// All known CDN files contain identical 7-byte block 1 data:
/// `03 00 00 00 00 6E 00`
const BLOCK1_DATA: [u8; 7] = [0x03, 0x00, 0x00, 0x00, 0x00, 0x6E, 0x00];

/// Builder for Patch Index files
#[derive(Debug, Clone)]
pub struct PatchIndexBuilder {
    key_size: u8,
    entries: Vec<PatchIndexEntry>,
}

impl PatchIndexBuilder {
    /// Create a new builder with the default key size (16)
    pub fn new() -> Self {
        Self {
            key_size: 16,
            entries: Vec::new(),
        }
    }

    /// Set the key size (1-16 bytes)
    pub fn key_size(mut self, key_size: u8) -> Self {
        self.key_size = key_size;
        self
    }

    /// Add a patch index entry
    pub fn add_entry(&mut self, entry: PatchIndexEntry) {
        self.entries.push(entry);
    }

    /// Build the patch index binary data
    ///
    /// Produces three blocks matching real CDN layout:
    /// - Block 1 (type 1): 7-byte configuration
    /// - Block 2 (type 2): Main entry block
    /// - Block 3 (type 8): Extended entry block (copy of block 2 data)
    pub fn build(&self) -> PatchIndexResult<Vec<u8>> {
        let esize = entry_size(self.key_size);

        // Build block 2 data: entry_count(4) + key_size(1) + entries
        let block2_size = 5 + self.entries.len() * esize;
        let mut block2_data = Vec::with_capacity(block2_size);
        block2_data.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        block2_data.push(self.key_size);
        for entry in &self.entries {
            block2_data.extend_from_slice(&entry.build(self.key_size));
        }

        // Build block 8 data: 14-byte header + same entries
        let block8_header_size = 14usize;
        let block8_size = block8_header_size + self.entries.len() * esize;
        let mut block8_data = Vec::with_capacity(block8_size);
        block8_data.push(3); // version
        block8_data.push(self.key_size);
        block8_data.extend_from_slice(&(block8_header_size as u16).to_le_bytes()); // data_offset
        block8_data.extend_from_slice(&(self.entries.len() as u32).to_le_bytes()); // entry_count
        block8_data.extend_from_slice(&((esize + 1) as u32).to_le_bytes()); // unknown (entry_size + 1)
        block8_data.push(0x01); // unknown flag byte
        block8_data.push(0x02); // unknown flag byte
        for entry in &self.entries {
            block8_data.extend_from_slice(&entry.build(self.key_size));
        }

        // Build header
        let block1_size = BLOCK1_DATA.len() as u32;
        let blocks = vec![
            BlockDescriptor {
                block_type: 1,
                block_size: block1_size,
            },
            BlockDescriptor {
                block_type: 2,
                block_size: block2_data.len() as u32,
            },
            BlockDescriptor {
                block_type: 8,
                block_size: block8_data.len() as u32,
            },
        ];

        // Header size: 12 (preamble) + 2 (extra_header_len) + 1 (key_size byte)
        //   + 4 (block_count) + 3*8 (block descriptors) = 43
        let header_size = 12 + 2 + 1 + 4 + blocks.len() * 8;
        let total_size = header_size + BLOCK1_DATA.len() + block2_data.len() + block8_data.len();

        let header = PatchIndexHeader {
            header_size: header_size as u32,
            version: 1,
            data_size: total_size as u32,
            key_size: 0,
            key_data: [0; 16],
            extra_data: Vec::new(),
            blocks,
        };

        let mut out = header.build();
        out.extend_from_slice(&BLOCK1_DATA);
        out.extend_from_slice(&block2_data);
        out.extend_from_slice(&block8_data);

        debug_assert_eq!(out.len(), total_size);

        Ok(out)
    }
}

impl Default for PatchIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::CascFormat;
    use crate::patch_index::PatchIndex;

    #[test]
    fn test_build_empty() {
        let builder = PatchIndexBuilder::new();
        let data = builder.build().unwrap();

        let index = PatchIndex::parse(&data).unwrap();
        assert!(index.entries.is_empty());
        assert_eq!(index.header.version, 1);
    }

    #[test]
    fn test_build_round_trip() {
        let mut builder = PatchIndexBuilder::new();
        builder.add_entry(PatchIndexEntry {
            source_ekey: [0x01; 16],
            source_size: 1000,
            target_ekey: [0x02; 16],
            target_size: 2000,
            encoded_size: 1500,
            suffix_offset: 1,
            patch_ekey: [0x03; 16],
        });
        builder.add_entry(PatchIndexEntry {
            source_ekey: [0x04; 16],
            source_size: 3000,
            target_ekey: [0x05; 16],
            target_size: 4000,
            encoded_size: 3500,
            suffix_offset: 1,
            patch_ekey: [0x06; 16],
        });

        let data = builder.build().unwrap();
        let index = PatchIndex::parse(&data).unwrap();

        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.entries[0].source_ekey, [0x01; 16]);
        assert_eq!(index.entries[0].target_size, 2000);
        assert_eq!(index.entries[1].patch_ekey, [0x06; 16]);
    }
}

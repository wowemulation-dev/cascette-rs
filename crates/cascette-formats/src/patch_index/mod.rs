//! Patch Index format implementation
//!
//! The Patch Index is a block-based binary format that maps patch blobs to
//! source/target file pairs. It is referenced by the `patch-index` key in
//! build configs and served as a BLTE-encoded file from CDN `data/` path.
//!
//! This is distinct from **patch archive index files** (`.index` files on
//! CDN `patch/` path) which use the standard CDN archive index format.
//!
//! # Format Structure
//!
//! ```text
//! Patch Index:
//! ├── Header (variable size, little-endian)
//! │   ├── header_size (u32)
//! │   ├── version (u32, must be 1)
//! │   ├── data_size (u32, total file size)
//! │   ├── extra_header_length (u16)
//! │   ├── [extra header data]
//! │   ├── block_count (u32)
//! │   └── Block Descriptors (block_count × 8 bytes)
//! │       ├── block_type (u32)
//! │       └── block_size (u32)
//! ├── Block Data (at offset header_size)
//! │   ├── Block type 1: Configuration (7 bytes, skipped)
//! │   ├── Block type 2: Entry table
//! │   │   ├── entry_count (u32 LE)
//! │   │   ├── key_size (u8)
//! │   │   └── entries (entry_count × entry_size)
//! │   └── Block type 8: Extended entry table (14-byte header + entries)
//! └── [End of file]
//! ```
//!
//! # Entry Format (61 bytes with key_size=16)
//!
//! Each entry maps a patch blob to the files it transforms:
//! - `source_ekey` (16): Source file encoding key
//! - `source_size` (u32 LE): Source file decoded size
//! - `target_ekey` (16): Target file encoding key
//! - `target_size` (u32 LE): Target file decoded size
//! - `encoded_size` (u32 LE): Encoded size
//! - `suffix_offset` (u8): EKey suffix table offset
//! - `patch_ekey` (16): Patch blob encoding key
//!
//! # Usage
//!
//! ## Parsing
//!
//! ```rust,no_run
//! use cascette_formats::patch_index::PatchIndex;
//! use cascette_formats::CascFormat;
//!
//! let data = std::fs::read("patch_index.bin")?;
//! let index = PatchIndex::parse(&data)?;
//!
//! println!("{} entries, key_size={}", index.entries.len(), index.key_size);
//!
//! for entry in &index.entries {
//!     println!("patch {} transforms {} -> {}",
//!         hex::encode(entry.patch_ekey),
//!         hex::encode(entry.source_ekey),
//!         hex::encode(entry.target_ekey));
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Building
//!
//! ```rust
//! use cascette_formats::patch_index::{PatchIndexBuilder, PatchIndexEntry};
//!
//! let mut builder = PatchIndexBuilder::new();
//! builder.add_entry(PatchIndexEntry {
//!     source_ekey: [0x01; 16],
//!     source_size: 1000,
//!     target_ekey: [0x02; 16],
//!     target_size: 2000,
//!     encoded_size: 1500,
//!     suffix_offset: 1,
//!     patch_ekey: [0x03; 16],
//! });
//!
//! let data = builder.build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod builder;
mod entry;
/// Patch Index error types
pub mod error;
mod header;
/// Patch Index parser (block-level parsing functions)
pub mod parser;

pub use builder::PatchIndexBuilder;
pub use entry::{
    PatchIndexEntry, PatchIndexEntryV2, PatchIndexEntryV3, V3_FLAG_HAS_DECODED_SIZE,
    V3_FLAG_HAS_ORIGINAL_EKEY, V3_FLAG_HAS_ORIGINAL_EKEY_OFFSET,
};
pub use error::{PatchIndexError, PatchIndexResult};
pub use header::{BlockDescriptor, PatchIndexHeader};
pub use parser::{BLOCK_TYPE_V2, BLOCK_TYPE_V3};

/// Complete parsed Patch Index
///
/// Contains the file header with block descriptors and all parsed entries
/// from block types 2 (V1), 6 (V2), and 10 (V3).
#[derive(Debug, Clone)]
pub struct PatchIndex {
    /// File header including block descriptors
    pub header: PatchIndexHeader,

    /// Entry key size in bytes (typically 16 for MD5)
    pub key_size: u8,

    /// Parsed entries from block type 2 (V1)
    pub entries: Vec<PatchIndexEntry>,

    /// Parsed entries from block type 6 (V2)
    pub entries_v2: Vec<PatchIndexEntryV2>,

    /// Parsed entries from block type 10 (V3)
    pub entries_v3: Vec<PatchIndexEntryV3>,

    /// Shared ESpec string table from V2/V3 blocks
    pub espec_table: Option<String>,
}

impl PatchIndex {
    /// Find all entries for a given patch blob EKey
    pub fn find_by_patch_ekey(&self, patch_ekey: &[u8; 16]) -> Vec<&PatchIndexEntry> {
        self.entries
            .iter()
            .filter(|e| &e.patch_ekey == patch_ekey)
            .collect()
    }

    /// Find entries that transform a specific source file
    pub fn find_by_source_ekey(&self, source_ekey: &[u8; 16]) -> Vec<&PatchIndexEntry> {
        self.entries
            .iter()
            .filter(|e| &e.source_ekey == source_ekey)
            .collect()
    }

    /// Find entries that produce a specific target file
    pub fn find_by_target_ekey(&self, target_ekey: &[u8; 16]) -> Vec<&PatchIndexEntry> {
        self.entries
            .iter()
            .filter(|e| &e.target_ekey == target_ekey)
            .collect()
    }

    /// Get all unique patch EKeys
    pub fn unique_patch_ekeys(&self) -> Vec<[u8; 16]> {
        let mut keys: Vec<[u8; 16]> = self.entries.iter().map(|e| e.patch_ekey).collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// Find V2 entries by target EKey
    pub fn find_v2_by_target_ekey(&self, target_ekey: &[u8; 16]) -> Vec<&PatchIndexEntryV2> {
        self.entries_v2
            .iter()
            .filter(|e| &e.target_ekey == target_ekey)
            .collect()
    }

    /// Find V3 entries by target EKey
    pub fn find_v3_by_target_ekey(&self, target_ekey: &[u8; 16]) -> Vec<&PatchIndexEntryV3> {
        self.entries_v3
            .iter()
            .filter(|e| &e.base.target_ekey == target_ekey)
            .collect()
    }

    /// Whether this index contains any V2 or V3 entries
    pub fn has_extended_entries(&self) -> bool {
        !self.entries_v2.is_empty() || !self.entries_v3.is_empty()
    }

    /// Total entry count across all versions
    pub fn total_entry_count(&self) -> usize {
        self.entries.len() + self.entries_v2.len() + self.entries_v3.len()
    }
}

impl crate::CascFormat for PatchIndex {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let parsed = parser::parse_patch_index_full(data)?;
        Ok(Self {
            header: parsed.header,
            key_size: parsed.key_size,
            entries: parsed.entries,
            entries_v2: parsed.entries_v2,
            entries_v3: parsed.entries_v3,
            espec_table: parsed.espec_table,
        })
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut builder = PatchIndexBuilder::new().key_size(self.key_size);
        for entry in &self.entries {
            builder.add_entry(entry.clone());
        }
        builder
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::CascFormat;

    #[test]
    fn test_parse_and_query() {
        let mut builder = PatchIndexBuilder::new();
        let patch_key = [0xAA; 16];
        builder.add_entry(PatchIndexEntry {
            source_ekey: [0x01; 16],
            source_size: 1000,
            target_ekey: [0x02; 16],
            target_size: 2000,
            encoded_size: 1500,
            suffix_offset: 1,
            patch_ekey: patch_key,
        });
        builder.add_entry(PatchIndexEntry {
            source_ekey: [0x03; 16],
            source_size: 3000,
            target_ekey: [0x04; 16],
            target_size: 4000,
            encoded_size: 3500,
            suffix_offset: 1,
            patch_ekey: patch_key,
        });
        builder.add_entry(PatchIndexEntry {
            source_ekey: [0x05; 16],
            source_size: 5000,
            target_ekey: [0x06; 16],
            target_size: 6000,
            encoded_size: 5500,
            suffix_offset: 1,
            patch_ekey: [0xBB; 16],
        });

        let data = builder.build().unwrap();
        let index = PatchIndex::parse(&data).unwrap();

        // Find by patch key
        let results = index.find_by_patch_ekey(&patch_key);
        assert_eq!(results.len(), 2);

        // Find by source key
        let results = index.find_by_source_ekey(&[0x03; 16]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target_size, 4000);

        // Unique patch keys
        let uniq = index.unique_patch_ekeys();
        assert_eq!(uniq.len(), 2);

        // No extended entries in V1-only index
        assert!(!index.has_extended_entries());
        assert_eq!(index.total_entry_count(), 3);
    }

    #[test]
    fn test_casc_format_round_trip() {
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

        let original_data = builder.build().unwrap();
        let index = PatchIndex::parse(&original_data).unwrap();
        let rebuilt_data = index.build().unwrap();
        let reparsed = PatchIndex::parse(&rebuilt_data).unwrap();

        assert_eq!(index.entries.len(), reparsed.entries.len());
        assert_eq!(index.entries[0], reparsed.entries[0]);
    }
}

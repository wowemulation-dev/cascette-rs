//! Patch Archive (PA) format implementation
//!
//! Patch Archives are block-based manifests that describe differential patches
//! between content versions. They map target files to available patches from
//! various source versions.
//!
//! # Format Structure
//!
//! ```text
//! PA File:
//! ├── Header (10 bytes, big-endian)
//! │   ├── Magic: "PA" (2 bytes)
//! │   ├── Version (1 byte) - 1 or 2
//! │   ├── File Key Size (1 byte) - 1-16
//! │   ├── Old Key Size (1 byte) - 1-16
//! │   ├── Patch Key Size (1 byte) - 1-16
//! │   ├── Block Size Bits (1 byte) - 12-24
//! │   ├── Block Count (2 bytes, big-endian)
//! │   └── Flags (1 byte)
//! ├── Extended Header (optional, when flags & 0x02)
//! │   ├── Encoding CKey (file_key_size bytes)
//! │   ├── Encoding EKey (file_key_size bytes)
//! │   ├── Decoded Size (4 bytes, big-endian)
//! │   ├── Encoded Size (4 bytes, big-endian)
//! │   ├── ESpec Length (1 byte)
//! │   └── ESpec String (espec_length bytes, UTF-8)
//! ├── Block Table (block_count entries)
//! │   └── Per block:
//! │       ├── Last File CKey (file_key_size bytes)
//! │       ├── Block MD5 (16 bytes)
//! │       └── Block Offset (4 bytes, big-endian)
//! └── Block Data (at block_offset for each block)
//!     └── File Entries (terminated by 0x00 sentinel):
//!         ├── Num Patches (1 byte, 0 = end of block)
//!         ├── Target CKey (file_key_size bytes)
//!         ├── Decoded Size (5 bytes, uint40, big-endian)
//!         └── Per patch:
//!             ├── Source EKey (old_key_size bytes)
//!             ├── Source Decoded Size (5 bytes, uint40, big-endian)
//!             ├── Patch EKey (patch_key_size bytes)
//!             ├── Patch Size (4 bytes, big-endian)
//!             └── Patch Index (1 byte)
//! ```
//!
//! # Usage
//!
//! ## Parsing a Patch Archive
//!
//! ```rust,no_run
//! use cascette_formats::patch_archive::PatchArchive;
//! use cascette_formats::CascFormat;
//!
//! let data = std::fs::read("patch_manifest.pa")?;
//! let archive = PatchArchive::parse(&data)?;
//!
//! println!("PA v{}, {} blocks, {} file entries",
//!     archive.header.version,
//!     archive.blocks.len(),
//!     archive.total_file_entries());
//!
//! if let Some(info) = &archive.encoding_info {
//!     println!("Encoding: espec={}", info.espec);
//! }
//!
//! for file_entry in archive.all_file_entries() {
//!     println!("Target: {}, {} patches",
//!         hex::encode(file_entry.target_ckey),
//!         file_entry.patches.len());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Building a Patch Archive
//!
//! ```rust
//! use cascette_formats::patch_archive::PatchArchiveBuilder;
//!
//! let mut builder = PatchArchiveBuilder::new();
//! builder.add_file_entry(
//!     [0x02; 16], // target CKey
//!     1000,       // decoded size
//!     vec![
//!         (
//!             [0x01; 16], // source EKey
//!             500,        // source decoded size
//!             [0x03; 16], // patch EKey
//!             200,        // patch size
//!             0,          // patch index
//!         ),
//!     ],
//! );
//!
//! let archive_data = builder.build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub(crate) mod block;
mod builder;
mod compression;
mod entry;
mod error;
mod header;
pub(crate) mod parser;
mod utils;

pub use block::{FilePatch, PatchArchiveEncodingInfo, PatchBlock, PatchFileEntry};
pub use builder::PatchArchiveBuilder;
pub use compression::{
    decompress_patch_data, format_compression_spec, get_compression_at_offset,
    parse_compression_spec,
};
pub use entry::PatchEntry;
pub use error::{PatchArchiveError, PatchArchiveResult};
pub use header::PatchArchiveHeader;

/// Complete Patch Archive with block-based structure
///
/// Parsed from the real CDN patch manifest format. Contains a header,
/// optional encoding info (extended header), and blocks with file entries.
#[derive(Debug, Clone)]
pub struct PatchArchive {
    /// PA header containing format metadata
    pub header: PatchArchiveHeader,
    /// Encoding info from extended header (when flags & 0x02)
    pub encoding_info: Option<PatchArchiveEncodingInfo>,
    /// Blocks containing file entries and patches
    pub blocks: Vec<PatchBlock>,
}

impl PatchArchive {
    /// Total number of file entries across all blocks
    pub fn total_file_entries(&self) -> usize {
        self.blocks.iter().map(|b| b.file_entries.len()).sum()
    }

    /// Iterate over all file entries across all blocks
    pub fn all_file_entries(&self) -> impl Iterator<Item = &PatchFileEntry> {
        self.blocks.iter().flat_map(|b| &b.file_entries)
    }

    /// Find a patch for a specific target content key
    pub fn find_patches_for_target(&self, target_ckey: &[u8; 16]) -> Option<&PatchFileEntry> {
        self.all_file_entries()
            .find(|entry| &entry.target_ckey == target_ckey)
    }

    /// Find all patches available from a specific source
    pub fn find_patches_from_source(
        &self,
        source_ekey: &[u8; 16],
    ) -> Vec<(&PatchFileEntry, &FilePatch)> {
        let mut results = Vec::new();
        for entry in self.all_file_entries() {
            for patch in &entry.patches {
                if &patch.source_ekey == source_ekey {
                    results.push((entry, patch));
                }
            }
        }
        results
    }

    /// Validate that blocks are sorted by CKey
    ///
    /// Agent.exe validates this with `_memcmp` during parsing at 0x6a6487.
    pub fn validate_block_sort_order(&self) -> PatchArchiveResult<()> {
        for window in self.blocks.windows(2) {
            if window[0].last_file_ckey > window[1].last_file_ckey {
                return Err(PatchArchiveError::BlocksNotSorted {
                    index: 1, // Relative to window
                });
            }
        }
        Ok(())
    }

    /// Flatten all file entries into legacy PatchEntry format
    ///
    /// Each (file_entry, patch) pair becomes one PatchEntry.
    /// The compression_info field is populated from the archive-level espec
    /// if available.
    pub fn flatten_entries(&self) -> Vec<PatchEntry> {
        let compression_info = self
            .encoding_info
            .as_ref()
            .map(|info| info.espec.clone())
            .unwrap_or_default();

        let mut entries = Vec::new();
        for file_entry in self.all_file_entries() {
            for patch in &file_entry.patches {
                entries.push(PatchEntry {
                    old_content_key: patch.source_ekey,
                    new_content_key: file_entry.target_ckey,
                    patch_encoding_key: patch.patch_ekey,
                    compression_info: compression_info.clone(),
                    additional_data: Vec::new(),
                });
            }
        }
        entries
    }
}

impl crate::CascFormat for PatchArchive {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut cursor = std::io::Cursor::new(data);
        let (header, encoding_info, blocks) = parser::parse_patch_archive(&mut cursor)?;
        Ok(Self {
            header,
            encoding_info,
            blocks,
        })
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut builder = PatchArchiveBuilder::new()
            .version(self.header.version)
            .block_size_bits(self.header.block_size_bits);

        if let Some(ref info) = self.encoding_info {
            builder = builder.encoding_info(info.clone());
        }

        for entry in self.all_file_entries() {
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
    fn test_patch_archive_round_trip() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );

        let data = builder.build().expect("build should succeed");
        let parsed = PatchArchive::parse(&data).expect("parse should succeed");

        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(parsed.total_file_entries(), 1);
        let entry = &parsed.blocks[0].file_entries[0];
        assert_eq!(entry.target_ckey, [0x02; 16]);
        assert_eq!(entry.decoded_size, 1000);
        assert_eq!(entry.patches[0].source_ekey, [0x01; 16]);
        assert_eq!(entry.patches[0].patch_size, 200);
    }

    #[test]
    fn test_flatten_entries() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![
                ([0x01; 16], 500, [0x03; 16], 200, 0),
                ([0x04; 16], 800, [0x05; 16], 300, 1),
            ],
        );

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        let flat = archive.flatten_entries();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].old_content_key, [0x01; 16]);
        assert_eq!(flat[0].new_content_key, [0x02; 16]);
        assert_eq!(flat[1].old_content_key, [0x04; 16]);
    }

    #[test]
    fn test_block_sort_validation() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x11; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );
        builder.add_file_entry(
            [0x22; 16],
            2000,
            vec![([0x04; 16], 1000, [0x06; 16], 300, 0)],
        );

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        // Builder sorts entries, so blocks should be sorted
        assert!(archive.validate_block_sort_order().is_ok());
    }

    #[test]
    fn test_find_patches() {
        let mut builder = PatchArchiveBuilder::new();
        let target = [0x02; 16];
        let source = [0x01; 16];
        builder.add_file_entry(target, 1000, vec![(source, 500, [0x03; 16], 200, 0)]);

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        // Find by target
        let found = archive.find_patches_for_target(&target);
        assert!(found.is_some());
        assert_eq!(found.unwrap().decoded_size, 1000);

        // Find by source
        let found = archive.find_patches_from_source(&source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1.patch_size, 200);
    }

    #[test]
    fn test_casc_format_round_trip() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );

        let original_data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&original_data).expect("parse should succeed");
        let rebuilt_data = archive.build().expect("rebuild should succeed");
        let reparsed = PatchArchive::parse(&rebuilt_data).expect("reparse should succeed");

        assert_eq!(archive.total_file_entries(), reparsed.total_file_entries());
    }
}

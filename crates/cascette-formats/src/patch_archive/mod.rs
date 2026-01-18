//! Patch Archive (PA) format implementation
//!
//! Patch Archives contain manifest files that describe differential patches between
//! different versions of NGDP content. They enable incremental updates by providing
//! mappings between old content keys, new content keys, and patch data.
//!
//! # Features
//!
//! - Parser and builder for PA format
//! - Big-endian header with mixed-endianness entries
//! - Variable-length compression specifications
//! - Support for both compressed (BLTE) and uncompressed archives
//! - Content addressing through MD5 hashes
//! - Streaming operations for large patch files
//!
//! # Format Structure
//!
//! ```text
//! PA File:
//! ├── Header (10 bytes, big-endian)
//! │   ├── Magic: "PA" (2 bytes)
//! │   ├── Version (1 byte) - Typically 2
//! │   ├── File Key Size (1 byte) - 16 for MD5
//! │   ├── Old Key Size (1 byte) - 16 for MD5
//! │   ├── Patch Key Size (1 byte) - 16 for MD5
//! │   ├── Block Size Bits (1 byte) - 16 for 64KB blocks
//! │   ├── Block Count (2 bytes, big-endian)
//! │   └── Flags (1 byte)
//! └── Patch Entries (variable number)
//!     └── Per entry:
//!         ├── Old Content Key (16 bytes, MD5)
//!         ├── New Content Key (16 bytes, MD5)
//!         ├── Patch Encoding Key (16 bytes, MD5)
//!         ├── Compression Info (null-terminated string)
//!         └── Additional Patch Data (variable)
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
//! println!("PA v{}, {} entries", archive.header.version, archive.entries.len());
//!
//! // Find patch for specific content
//! let content_key = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
//!                    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
//! if let Some(patch) = archive.find_patch_for_content(&content_key) {
//!     println!("Found patch: {} -> {}",
//!         hex::encode(&patch.old_content_key),
//!         hex::encode(&patch.new_content_key));
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
//! builder.add_patch(
//!     [0x01; 16], // old content key
//!     [0x02; 16], // new content key
//!     [0x03; 16], // patch encoding key
//!     "{*=z}".to_string(), // compression info
//! );
//!
//! let archive_data = builder.build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod builder;
mod compression;
mod entry;
mod error;
mod header;
mod parser;
mod utils;

pub use builder::PatchArchiveBuilder;
pub use compression::{
    decompress_patch_data, format_compression_spec, get_compression_at_offset,
    parse_compression_spec,
};
pub use entry::PatchEntry;
pub use error::{PatchArchiveError, PatchArchiveResult};
pub use header::PatchArchiveHeader;
pub use parser::PatchArchiveParser;

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};
use std::collections::HashMap;

/// Complete Patch Archive structure
#[derive(Debug, Clone)]
pub struct PatchArchive {
    /// PA header containing format metadata
    pub header: PatchArchiveHeader,
    /// Patch entries with mappings and compression info
    pub entries: Vec<PatchEntry>,
}

impl BinRead for PatchArchive {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read header (big-endian)
        let header = PatchArchiveHeader::read_options(reader, binrw::Endian::Big, ())?;

        // Validate header
        header.validate().map_err(|e| binrw::Error::Custom {
            pos: reader.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        // Read entries
        let mut entries = Vec::with_capacity(header.block_count as usize);
        let args = (
            header.file_key_size,
            header.old_key_size,
            header.patch_key_size,
        );

        for _ in 0..header.block_count {
            let entry = PatchEntry::read_options(reader, endian, args)?;
            entries.push(entry);
        }

        Ok(Self { header, entries })
    }
}

impl BinWrite for PatchArchive {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write header (big-endian)
        self.header.write_options(writer, binrw::Endian::Big, ())?;

        // Write entries
        let args = (
            self.header.file_key_size,
            self.header.old_key_size,
            self.header.patch_key_size,
        );

        for entry in &self.entries {
            entry.write_options(writer, binrw::Endian::Little, args)?;
        }

        Ok(())
    }
}

impl PatchArchive {
    /// Create streaming parser for large archives
    pub fn parser<R: Read + Seek>(reader: R) -> PatchArchiveResult<PatchArchiveParser<R>> {
        PatchArchiveParser::new(reader)
    }

    /// Find patch entry for specific content key
    pub fn find_patch_for_content(&self, content_key: &[u8; 16]) -> Option<&PatchEntry> {
        self.entries
            .iter()
            .find(|entry| &entry.old_content_key == content_key)
    }

    /// Build lookup map for efficient patch queries
    pub fn build_lookup_map(&self) -> HashMap<[u8; 16], &PatchEntry> {
        self.entries
            .iter()
            .map(|entry| (entry.old_content_key, entry))
            .collect()
    }

    /// Build patch chain from start key to end key
    pub fn build_patch_chain(
        &self,
        start_key: &[u8; 16],
        end_key: &[u8; 16],
    ) -> Option<PatchChain> {
        let mut chain = Vec::new();
        let mut current_key = *start_key;
        let mut visited = std::collections::HashSet::new();

        while current_key != *end_key {
            if visited.contains(&current_key) {
                return None; // Cycle detected
            }
            visited.insert(current_key);

            let patch_entry = self.find_patch_for_content(&current_key)?;
            current_key = patch_entry.new_content_key;
            chain.push(patch_entry.clone());

            if chain.len() > 10 {
                return None; // Chain too long
            }
        }

        Some(PatchChain {
            steps: chain,
            start_key: *start_key,
            end_key: *end_key,
        })
    }
}

/// Chain of patches from one content key to another
#[derive(Debug, Clone)]
pub struct PatchChain {
    /// Sequence of patch steps
    pub steps: Vec<PatchEntry>,
    /// Starting content key
    pub start_key: [u8; 16],
    /// Final content key
    pub end_key: [u8; 16],
}

impl crate::CascFormat for PatchArchive {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::read_options(&mut std::io::Cursor::new(data), binrw::Endian::Big, ())
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut output = Vec::new();
        self.write_options(
            &mut std::io::Cursor::new(&mut output),
            binrw::Endian::Big,
            (),
        )
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        Ok(output)
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
        builder.add_patch([0x01; 16], [0x02; 16], [0x03; 16], "{*=z}".to_string());

        let built = builder.build().expect("Operation should succeed");
        let parsed = PatchArchive::parse(&built).expect("Operation should succeed");

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].old_content_key, [0x01; 16]);
        assert_eq!(parsed.entries[0].new_content_key, [0x02; 16]);
        assert_eq!(parsed.entries[0].patch_encoding_key, [0x03; 16]);
        assert_eq!(parsed.entries[0].compression_info, "{*=z}");
    }

    #[test]
    fn test_patch_lookup() {
        let mut builder = PatchArchiveBuilder::new();
        let old_key = [0x01; 16];
        builder.add_patch(old_key, [0x02; 16], [0x03; 16], "{*=z}".to_string());

        let built = builder.build().expect("Operation should succeed");
        let archive = PatchArchive::parse(&built).expect("Operation should succeed");

        let patch = archive.find_patch_for_content(&old_key);
        assert!(patch.is_some());
        assert_eq!(
            patch.expect("Patch should be found").new_content_key,
            [0x02; 16]
        );
    }
}

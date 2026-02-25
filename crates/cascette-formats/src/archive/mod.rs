//! Archive system for NGDP/CASC content storage and retrieval
//!
//! This module provides complete support for CDN archive files (.data) and their
//! corresponding index files (.index). Archive files are the primary storage
//! mechanism for game content in NGDP/CASC systems.
//!
//! # Key Features
//!
//! - **Archive Building**: Create CDN archive data files with BLTE-encoded content
//! - **Archive Index Parsing**: Binary format parsing with chunked structure
//! - **Archive Index Building**: Create index files for archive data
//! - **Variable-Length Key Support**: Full encoding key support based on footer specification
//! - **Binary Search Operations**: Fast content location with O(log n) lookups
//! - **HTTP Range Requests**: Efficient partial content downloads
//! - **BLTE Integration**: Seamless decompression and decryption support
//! - **CDN Client Operations**: Complete CDN interaction support
//! - **Memory Efficient**: Chunked loading for large indices
//!
//! # Architecture
//!
//! The archive system uses a two-tier lookup mechanism:
//! 1. **Archive Index (.index)**: Maps variable-length encoding keys to archive offsets
//! 2. **Archive Data (.data)**: Sequential BLTE-compressed content blocks
//!
//! ```text
//! Content Resolution Flow:
//! EncodingKey → ArchiveIndex → ArchiveOffset → ArchiveData → BLTE → Content
//! ```
//!
//! # Binary Format Support
//!
//! Archive indices use a specialized binary format:
//! - **24-byte entries**: Fixed-size entries for binary search
//! - **Chunked structure**: 4KB chunks for memory efficiency
//! - **Footer validation**: Jenkins96 hash verification
//! - **Variable-length keys**: Encoding keys as specified in footer (typically 16 bytes)
//!
//! # Usage Examples
//!
//! ## Parse Archive Index
//!
//! ```rust
//! use cascette_formats::archive::ArchiveIndex;
//! use std::io::Cursor;
//!
//! // Parse archive index from existing data
//! let index_data = vec![0u8; 100]; // Your index data
//! let mut cursor = Cursor::new(index_data);
//! // let index = ArchiveIndex::parse(&mut cursor)?;
//!
//! // Find content by encoding key
//! let encoding_key = [0u8; 16]; // Your 16-byte encoding key
//! // if let Some(entry) = index.find_entry(&encoding_key) {
//! //     println!("Found at offset: {}, size: {}", entry.offset, entry.size);
//! // }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Build Archive Index
//!
//! ```rust,no_run
//! use cascette_formats::archive::ArchiveIndexBuilder;
//! use std::fs::File;
//!
//! let mut builder = ArchiveIndexBuilder::new();
//!
//! // Add entries
//! let encoding_key = [1u8; 16];
//! builder.add_entry(encoding_key.to_vec(), 1024, 4096);
//!
//! // Build and write
//! let mut file = File::create("new_archive.index")?;
//! let index = builder.build(&mut file)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Build Archive Data File
//!
//! ```rust,no_run
//! use cascette_formats::archive::{ArchiveBuilder, ArchiveIndexBuilder};
//! use cascette_formats::blte::CompressionMode;
//! use std::fs::File;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create archive data file
//! let mut builder = ArchiveBuilder::new(File::create("archive")?);
//!
//! // Add content (will be BLTE-encoded)
//! let entry1 = builder.add_content(b"Hello, World!", CompressionMode::None)?;
//! let entry2 = builder.add_content(b"More content", CompressionMode::ZLib)?;
//!
//! // Finalize and get entries for index building
//! let (_, entries) = builder.finish()?;
//!
//! // Build corresponding index file
//! let mut index_builder = ArchiveIndexBuilder::new();
//! for entry in &entries {
//!     index_builder.add_entry(entry.encoding_key.to_vec(), entry.size, entry.offset);
//! }
//! index_builder.build(File::create("archive.index")?)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Read Archive Content
//!
//! ```rust
//! use cascette_formats::archive::{ArchiveReader, ArchiveLocation};
//! use std::io::Cursor;
//!
//! // Create reader from archive data
//! let archive_data = vec![0u8; 1000]; // Your archive data
//! let cursor = Cursor::new(archive_data);
//! let mut reader = ArchiveReader::new(cursor);
//!
//! // Read content using location
//! let location = ArchiveLocation::new("0123456789abcdef0123456789abcdef".to_string(), 0, 100);
//! // let content = reader.read_content(&location)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod archive_group;
mod builder;
mod error;
mod file;
mod index;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    mod archive_group_tests;
}

pub use archive_group::{ArchiveGroup, ArchiveGroupBuilder, ArchiveGroupEntry, build_merged};
pub use builder::{ArchiveBuilder, ArchiveEntry};
pub use error::{ArchiveError, ArchiveResult};
pub use file::{ArchiveFile, ArchiveLocation, ArchiveReader};
pub use index::{
    ArchiveIndex, ArchiveIndexBuilder, ChunkedArchiveIndex, IndexEntry, IndexFooter,
    calculate_block_hash, calculate_chunks, calculate_toc_hash, is_sorted,
};

/// Archive system constants
pub mod constants {
    /// Size of each archive index chunk in bytes
    pub const CHUNK_SIZE: usize = 4096;

    /// Size of each index entry in bytes
    pub const ENTRY_SIZE: usize = 24;

    /// Maximum entries that can fit in one chunk
    pub const MAX_ENTRIES_PER_CHUNK: usize = CHUNK_SIZE / ENTRY_SIZE; // 170

    /// Typical CDN archive index footer size in bytes (20 fixed + 8 hash = 28)
    ///
    /// The actual footer size is `MIN_FOOTER_SIZE + footer_hash_bytes` (see
    /// `index::MIN_FOOTER_SIZE`). This constant assumes the typical
    /// `footer_hash_bytes = 8`. Use `MIN_FOOTER_SIZE + footer.footer_hash_bytes`
    /// for the exact size.
    pub const FOOTER_SIZE: usize = 28;

    /// Local IDX/KMT file format version (v7)
    ///
    /// This refers to the local CASC storage index format (`{bucket}{version}.idx`),
    /// NOT the CDN archive index footer version. The CDN footer version field
    /// accepts values 0 or 1 (validated in `IndexFooter::validate_format`).
    pub const LOCAL_IDX_VERSION: u8 = 0x07;

    /// Typical truncated key size for local IDX entries (9 bytes)
    ///
    /// This is the common truncated `EKey` size in local CASC storage indices.
    /// CDN archive indices typically use full 16-byte keys. Agent.exe accepts
    /// any `hash_size` in `1..=16` (`tact::CdnIndexFooterValidator` at 0x6b8168).
    pub const TYPICAL_TRUNCATED_KEY_SIZE: u8 = 0x09;

    /// Expected segment size log2 (4KB chunks)
    pub const EXPECTED_SEGMENT_SIZE_LOG2: u8 = 0x0C;

    /// Expected max file size log2 (1GB archives)
    pub const EXPECTED_MAX_FILE_SIZE_LOG2: u8 = 0x1E;
}

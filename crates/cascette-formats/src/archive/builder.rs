//! CDN Archive data file builder
//!
//! This module provides the `ArchiveBuilder` for creating CDN archive data files.
//! Archive data files contain sequential BLTE-encoded content blocks with no
//! header, footer, or padding between entries.
//!
//! # CDN Archive Format
//!
//! CDN archive data files are simple concatenations of BLTE blocks:
//!
//! ```text
//! [BLTE block 0][BLTE block 1][BLTE block 2]...
//! ```
//!
//! - No file header or footer
//! - No alignment or padding between blocks
//! - Each block is a complete BLTE file (magic + header + chunks)
//! - The corresponding `.index` file maps encoding keys to (offset, size) pairs
//!
//! # Example
//!
//! ```rust,no_run
//! use cascette_formats::archive::{ArchiveBuilder, ArchiveIndexBuilder};
//! use cascette_formats::blte::CompressionMode;
//! use std::fs::File;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create archive data file
//! let mut archive_builder = ArchiveBuilder::new(File::create("archive")?);
//!
//! // Add content (will be BLTE-encoded)
//! let entry1 = archive_builder.add_content(b"Hello, World!", CompressionMode::None)?;
//! let entry2 = archive_builder.add_content(b"More content here", CompressionMode::ZLib)?;
//!
//! // Finalize archive and get entries
//! let (_, entries) = archive_builder.finish()?;
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

use std::io::{Seek, SeekFrom, Write};

use binrw::BinWrite;

use crate::archive::error::{ArchiveError, ArchiveResult};
use crate::blte::{BlteBuilder, BlteFile, CompressionMode};

/// Result of adding content to an archive
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// Encoding key (MD5 hash of the BLTE-encoded data)
    pub encoding_key: [u8; 16],
    /// Offset within the archive where this entry starts
    pub offset: u64,
    /// Size of the BLTE-encoded data in bytes
    pub size: u32,
}

impl ArchiveEntry {
    /// Create a new archive entry
    pub fn new(encoding_key: [u8; 16], offset: u64, size: u32) -> Self {
        Self {
            encoding_key,
            offset,
            size,
        }
    }

    /// Get the encoding key as a hex string
    pub fn encoding_key_hex(&self) -> String {
        use std::fmt::Write;
        self.encoding_key.iter().fold(String::new(), |mut acc, b| {
            // Writing to String cannot fail, so we discard the result
            let _ = write!(acc, "{b:02x}");
            acc
        })
    }
}

/// Builder for CDN archive data files
///
/// Creates archive files by sequentially writing BLTE-encoded content blocks.
/// After building, use the returned entries with `ArchiveIndexBuilder` to create
/// the corresponding `.index` file.
pub struct ArchiveBuilder<W: Write + Seek> {
    /// Underlying writer
    writer: W,
    /// Current write position
    position: u64,
    /// Entries added to the archive
    entries: Vec<ArchiveEntry>,
}

impl<W: Write + Seek> ArchiveBuilder<W> {
    /// Create a new archive builder
    ///
    /// # Arguments
    /// * `writer` - The writer to output archive data to
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            position: 0,
            entries: Vec::new(),
        }
    }

    /// Create a new archive builder, appending to existing data
    ///
    /// # Arguments
    /// * `writer` - The writer to output archive data to
    /// * `start_offset` - The starting offset (for appending to existing archives)
    pub fn new_at_offset(mut writer: W, start_offset: u64) -> ArchiveResult<Self> {
        writer.seek(SeekFrom::Start(start_offset))?;
        Ok(Self {
            writer,
            position: start_offset,
            entries: Vec::new(),
        })
    }

    /// Add pre-built BLTE data (already serialized bytes)
    ///
    /// The encoding key is computed as the MD5 hash of the BLTE data.
    ///
    /// # Arguments
    /// * `blte_data` - Raw BLTE-encoded bytes
    ///
    /// # Returns
    /// The `ArchiveEntry` describing the added content
    pub fn add_blte_data(&mut self, blte_data: &[u8]) -> ArchiveResult<ArchiveEntry> {
        // Compute encoding key (MD5 of BLTE data)
        let hash = md5::compute(blte_data);
        let encoding_key: [u8; 16] = hash.into();

        // Record offset before writing
        let offset = self.position;

        // Write BLTE data
        self.writer.write_all(blte_data)?;

        // Update position
        let size = blte_data.len() as u32;
        self.position += u64::from(size);

        // Create entry
        let entry = ArchiveEntry::new(encoding_key, offset, size);
        self.entries.push(entry.clone());

        Ok(entry)
    }

    /// Add a `BlteFile` (will be serialized to bytes)
    ///
    /// # Arguments
    /// * `blte` - The BLTE file to add
    ///
    /// # Returns
    /// The `ArchiveEntry` describing the added content
    pub fn add_blte_file(&mut self, blte: &BlteFile) -> ArchiveResult<ArchiveEntry> {
        // Serialize BLTE to bytes
        let mut blte_data = Vec::new();
        blte.write_options(
            &mut std::io::Cursor::new(&mut blte_data),
            binrw::Endian::Big,
            (),
        )
        .map_err(|e| ArchiveError::InvalidFormat(format!("Failed to serialize BLTE: {e}")))?;

        self.add_blte_data(&blte_data)
    }

    /// Add raw content (will be BLTE-encoded with given compression)
    ///
    /// # Arguments
    /// * `data` - Raw content bytes to encode and add
    /// * `mode` - Compression mode to use
    ///
    /// # Returns
    /// The `ArchiveEntry` describing the added content
    pub fn add_content(
        &mut self,
        data: &[u8],
        mode: CompressionMode,
    ) -> ArchiveResult<ArchiveEntry> {
        // Build BLTE file
        let blte = BlteBuilder::new()
            .with_compression(mode)
            .add_data(data)
            .map_err(|e| ArchiveError::InvalidFormat(format!("Failed to create BLTE: {e}")))?
            .build()
            .map_err(|e| ArchiveError::InvalidFormat(format!("Failed to build BLTE: {e}")))?;

        self.add_blte_file(&blte)
    }

    /// Add raw content with default compression (None)
    ///
    /// # Arguments
    /// * `data` - Raw content bytes to encode and add
    ///
    /// # Returns
    /// The `ArchiveEntry` describing the added content
    pub fn add_content_uncompressed(&mut self, data: &[u8]) -> ArchiveResult<ArchiveEntry> {
        self.add_content(data, CompressionMode::None)
    }

    /// Add raw content with ZLib compression
    ///
    /// # Arguments
    /// * `data` - Raw content bytes to encode and add
    ///
    /// # Returns
    /// The `ArchiveEntry` describing the added content
    pub fn add_content_zlib(&mut self, data: &[u8]) -> ArchiveResult<ArchiveEntry> {
        self.add_content(data, CompressionMode::ZLib)
    }

    /// Finalize the archive and return entries for index building
    ///
    /// # Returns
    /// A tuple of (writer, entries) where entries can be used with `ArchiveIndexBuilder`
    pub fn finish(mut self) -> ArchiveResult<(W, Vec<ArchiveEntry>)> {
        self.writer.flush()?;
        Ok((self.writer, self.entries))
    }

    /// Get the current write position (total bytes written)
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get entries added so far
    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    /// Get the number of entries added
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the builder is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get mutable access to the underlying writer
    ///
    /// Use with caution - writing directly may corrupt the archive
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl ArchiveBuilder<std::fs::File> {
    /// Create a new archive builder writing to a file
    ///
    /// # Arguments
    /// * `path` - Path to create the archive file at
    pub fn create<P: AsRef<std::path::Path>>(path: P) -> ArchiveResult<Self> {
        let file = std::fs::File::create(path)?;
        Ok(Self::new(file))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::archive::{ArchiveIndexBuilder, ArchiveReader};
    use std::io::Cursor;

    #[test]
    fn test_archive_builder_empty() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let builder = ArchiveBuilder::new(cursor);

        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.position(), 0);

        let (cursor, entries) = builder.finish().expect("finish should succeed");
        assert!(entries.is_empty());
        assert!(cursor.into_inner().is_empty());
    }

    #[test]
    fn test_archive_builder_single_entry() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let test_data = b"Hello, Archive!";
        let entry = builder
            .add_content_uncompressed(test_data)
            .expect("add_content should succeed");

        assert_eq!(builder.len(), 1);
        assert!(builder.position() > 0);
        assert_eq!(entry.offset, 0);
        assert!(entry.size > 0);

        let (cursor, entries) = builder.finish().expect("finish should succeed");
        assert_eq!(entries.len(), 1);

        // Verify the data can be read back
        let archive_data = cursor.into_inner();
        assert_eq!(archive_data.len(), entry.size as usize);
    }

    #[test]
    fn test_archive_builder_multiple_entries() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let data1 = b"First entry";
        let data2 = b"Second entry with more content";
        let data3 = b"Third";

        let entry1 = builder
            .add_content_uncompressed(data1)
            .expect("add should succeed");
        let entry2 = builder
            .add_content_uncompressed(data2)
            .expect("add should succeed");
        let entry3 = builder
            .add_content_uncompressed(data3)
            .expect("add should succeed");

        assert_eq!(builder.len(), 3);

        // Verify offsets are sequential
        assert_eq!(entry1.offset, 0);
        assert_eq!(entry2.offset, u64::from(entry1.size));
        assert_eq!(
            entry3.offset,
            u64::from(entry1.size) + u64::from(entry2.size)
        );

        let (cursor, entries) = builder.finish().expect("finish should succeed");
        assert_eq!(entries.len(), 3);

        // Verify total size
        let archive_data = cursor.into_inner();
        let expected_size = entry1.size + entry2.size + entry3.size;
        assert_eq!(archive_data.len(), expected_size as usize);
    }

    #[test]
    fn test_archive_builder_with_compression() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        // Compressible data (repeated pattern)
        let test_data = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

        let entry = builder
            .add_content_zlib(test_data)
            .expect("add should succeed");

        assert!(entry.size > 0);

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Verify BLTE magic at the start
        assert!(archive_data.len() >= 4);
        assert_eq!(&archive_data[0..4], b"BLTE");
    }

    #[test]
    fn test_archive_builder_round_trip_single() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let test_data = b"Round trip test data!";
        let entry = builder
            .add_content_uncompressed(test_data)
            .expect("add should succeed");

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Read back using ArchiveReader
        let mut reader = ArchiveReader::new(Cursor::new(archive_data));
        let location = crate::archive::ArchiveLocation::new(
            entry.encoding_key_hex(),
            entry.offset,
            u64::from(entry.size),
        );

        let content = reader.read_content(&location).expect("read should succeed");
        assert_eq!(content, test_data);
    }

    #[test]
    fn test_archive_builder_round_trip_multiple() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let data1 = b"First piece of content";
        let data2 = b"Second piece with different data";
        let data3 = b"Third and final piece";

        let entry1 = builder
            .add_content_uncompressed(data1)
            .expect("add should succeed");
        let entry2 = builder
            .add_content_uncompressed(data2)
            .expect("add should succeed");
        let entry3 = builder
            .add_content_uncompressed(data3)
            .expect("add should succeed");

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Read back all entries
        let mut reader = ArchiveReader::new(Cursor::new(archive_data));

        let loc1 = crate::archive::ArchiveLocation::new(
            entry1.encoding_key_hex(),
            entry1.offset,
            u64::from(entry1.size),
        );
        let loc2 = crate::archive::ArchiveLocation::new(
            entry2.encoding_key_hex(),
            entry2.offset,
            u64::from(entry2.size),
        );
        let loc3 = crate::archive::ArchiveLocation::new(
            entry3.encoding_key_hex(),
            entry3.offset,
            u64::from(entry3.size),
        );

        assert_eq!(
            reader.read_content(&loc1).expect("read should succeed"),
            data1
        );
        assert_eq!(
            reader.read_content(&loc2).expect("read should succeed"),
            data2
        );
        assert_eq!(
            reader.read_content(&loc3).expect("read should succeed"),
            data3
        );
    }

    #[test]
    fn test_archive_builder_round_trip_with_compression() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let test_data = b"This data will be compressed with ZLib compression algorithm";
        let entry = builder
            .add_content_zlib(test_data)
            .expect("add should succeed");

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Read back and verify decompression works
        let mut reader = ArchiveReader::new(Cursor::new(archive_data));
        let location = crate::archive::ArchiveLocation::new(
            entry.encoding_key_hex(),
            entry.offset,
            u64::from(entry.size),
        );

        let content = reader.read_content(&location).expect("read should succeed");
        assert_eq!(content, test_data);
    }

    #[test]
    fn test_archive_builder_with_index() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let data1 = b"Content for index test 1";
        let data2 = b"Content for index test 2";

        let entry1 = builder
            .add_content_uncompressed(data1)
            .expect("add should succeed");
        let entry2 = builder
            .add_content_uncompressed(data2)
            .expect("add should succeed");

        let (cursor, entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Build index
        let mut index_builder = ArchiveIndexBuilder::new();
        for entry in &entries {
            index_builder.add_entry(entry.encoding_key.to_vec(), entry.size, entry.offset);
        }

        let index_buffer = Vec::new();
        let index_cursor = Cursor::new(index_buffer);
        let index = index_builder
            .build(index_cursor)
            .expect("index build should succeed");

        // Verify index has correct entries
        assert_eq!(index.entry_count(), 2);

        // Find entries in index
        let found1 = index
            .find_entry(&entry1.encoding_key)
            .expect("entry1 should be found in index");
        let found2 = index
            .find_entry(&entry2.encoding_key)
            .expect("entry2 should be found in index");

        assert_eq!(found1.offset, entry1.offset);
        assert_eq!(found1.size, entry1.size);
        assert_eq!(found2.offset, entry2.offset);
        assert_eq!(found2.size, entry2.size);

        // Verify we can read content using index lookup
        let mut reader = ArchiveReader::new(Cursor::new(archive_data));
        let location = crate::archive::ArchiveLocation::new(
            entry1.encoding_key_hex(),
            found1.offset,
            u64::from(found1.size),
        );
        let content = reader.read_content(&location).expect("read should succeed");
        assert_eq!(content, data1);
    }

    #[test]
    fn test_archive_builder_add_blte_file() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        // Create a BLTE file manually
        let test_data = b"BLTE file test";
        let blte = BlteBuilder::new()
            .with_compression(CompressionMode::None)
            .add_data(test_data)
            .expect("add_data should succeed")
            .build()
            .expect("build should succeed");

        let entry = builder
            .add_blte_file(&blte)
            .expect("add_blte_file should succeed");

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Read back
        let mut reader = ArchiveReader::new(Cursor::new(archive_data));
        let location = crate::archive::ArchiveLocation::new(
            entry.encoding_key_hex(),
            entry.offset,
            u64::from(entry.size),
        );
        let content = reader.read_content(&location).expect("read should succeed");
        assert_eq!(content, test_data);
    }

    #[test]
    fn test_archive_builder_add_blte_data() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        // Create raw BLTE data
        let test_data = b"Raw BLTE data test";
        let blte = BlteBuilder::new()
            .with_compression(CompressionMode::None)
            .add_data(test_data)
            .expect("add_data should succeed")
            .build()
            .expect("build should succeed");

        let mut blte_bytes = Vec::new();
        blte.write_options(&mut Cursor::new(&mut blte_bytes), binrw::Endian::Big, ())
            .expect("serialize should succeed");

        let entry = builder
            .add_blte_data(&blte_bytes)
            .expect("add_blte_data should succeed");

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Read back
        let mut reader = ArchiveReader::new(Cursor::new(archive_data));
        let location = crate::archive::ArchiveLocation::new(
            entry.encoding_key_hex(),
            entry.offset,
            u64::from(entry.size),
        );
        let content = reader.read_content(&location).expect("read should succeed");
        assert_eq!(content, test_data);
    }

    #[test]
    fn test_encoding_key_computation() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut builder = ArchiveBuilder::new(cursor);

        let test_data = b"Test data for encoding key";
        let entry = builder
            .add_content_uncompressed(test_data)
            .expect("add should succeed");

        let (cursor, _entries) = builder.finish().expect("finish should succeed");
        let archive_data = cursor.into_inner();

        // Manually compute MD5 of the BLTE data
        let blte_slice =
            &archive_data[entry.offset as usize..(entry.offset as usize + entry.size as usize)];
        let expected_hash = md5::compute(blte_slice);

        assert_eq!(entry.encoding_key, *expected_hash);
    }

    #[test]
    fn test_archive_entry_hex() {
        let entry = ArchiveEntry::new(
            [
                0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC,
                0xDE, 0xF0,
            ],
            0,
            100,
        );
        assert_eq!(entry.encoding_key_hex(), "deadbeefcafebabe123456789abcdef0");
    }
}

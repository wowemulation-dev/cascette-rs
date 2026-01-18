//! Archive file operations and content extraction
//!
//! Archive files (.data) contain sequential BLTE-compressed content blocks.
//! This module provides operations for reading content from archives using
//! offset and size information from archive indices.

use crate::archive::error::{ArchiveError, ArchiveResult};
use crate::blte::BlteFile;
use binrw::BinRead;
use cascette_crypto::TactKeyStore;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Location of content within an archive
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveLocation {
    /// Archive hash (MD5 of archive file)
    pub archive_hash: String,
    /// Offset within archive file
    pub offset: u64,
    /// Size of compressed content
    pub size: u64,
}

impl ArchiveLocation {
    /// Create new archive location
    pub fn new(archive_hash: String, offset: u64, size: u64) -> Self {
        Self {
            archive_hash,
            offset,
            size,
        }
    }

    /// Validate hash format (should be 32 hex characters)
    pub fn validate_hash(&self) -> ArchiveResult<()> {
        if self.archive_hash.len() != 32 {
            return Err(ArchiveError::InvalidHashLength(self.archive_hash.len()));
        }

        // Verify all characters are hex
        if !self.archive_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ArchiveError::InvalidFormat(format!(
                "Invalid hex characters in hash: {}",
                self.archive_hash
            )));
        }

        Ok(())
    }
}

/// Archive file reader for local file access
pub struct ArchiveFile<R: Read + Seek> {
    /// Underlying reader
    reader: R,
    /// Current position in file
    position: u64,
}

impl<R: Read + Seek> ArchiveFile<R> {
    /// Create new archive file reader
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            position: 0,
        }
    }

    /// Read content at specific offset and size
    pub fn read_at_offset(&mut self, offset: u64, size: u64) -> ArchiveResult<Vec<u8>> {
        // Seek to offset
        self.reader.seek(SeekFrom::Start(offset))?;
        self.position = offset;

        // Read data
        let mut data = vec![
            0u8;
            size.try_into().map_err(|_| ArchiveError::InvalidFormat(
                "Size too large for platform".to_string()
            ))?
        ];
        self.reader.read_exact(&mut data)?;
        self.position += size;

        Ok(data)
    }

    /// Read and decompress BLTE content at offset
    pub fn read_blte_at_offset(&mut self, offset: u64, size: u64) -> ArchiveResult<Vec<u8>> {
        let blte_data = self.read_at_offset(offset, size)?;

        // Parse BLTE
        let blte = BlteFile::read_options(
            &mut std::io::Cursor::new(&blte_data),
            binrw::Endian::Big,
            (),
        )?;

        // Decompress
        let decompressed = blte.decompress()?;
        Ok(decompressed)
    }

    /// Read and decompress BLTE content with decryption support
    pub fn read_blte_at_offset_with_keys(
        &mut self,
        offset: u64,
        size: u64,
        key_store: &TactKeyStore,
    ) -> ArchiveResult<Vec<u8>> {
        let blte_data = self.read_at_offset(offset, size)?;

        // Parse BLTE
        let blte = BlteFile::read_options(
            &mut std::io::Cursor::new(&blte_data),
            binrw::Endian::Big,
            (),
        )?;

        // Decompress with decryption
        let decompressed = blte.decompress_with_keys(key_store)?;
        Ok(decompressed)
    }

    /// Get current position in file
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Seek to position
    pub fn seek(&mut self, pos: SeekFrom) -> ArchiveResult<u64> {
        let new_pos = self.reader.seek(pos)?;
        self.position = new_pos;
        Ok(new_pos)
    }
}

impl ArchiveFile<File> {
    /// Open archive file from path
    pub fn open<P: AsRef<Path>>(path: P) -> ArchiveResult<Self> {
        let file = File::open(path)?;
        Ok(Self::new(file))
    }
}

/// Archive reader with multiple access patterns
pub struct ArchiveReader<R: Read + Seek> {
    /// Archive file
    archive: ArchiveFile<R>,
    /// Optional key store for decryption
    key_store: Option<TactKeyStore>,
}

impl<R: Read + Seek> ArchiveReader<R> {
    /// Create new archive reader
    pub fn new(reader: R) -> Self {
        Self {
            archive: ArchiveFile::new(reader),
            key_store: None,
        }
    }

    /// Create archive reader with key store
    pub fn with_keys(reader: R, key_store: TactKeyStore) -> Self {
        Self {
            archive: ArchiveFile::new(reader),
            key_store: Some(key_store),
        }
    }

    /// Read raw content at location
    pub fn read_raw(&mut self, location: &ArchiveLocation) -> ArchiveResult<Vec<u8>> {
        location.validate_hash()?;
        self.archive.read_at_offset(location.offset, location.size)
    }

    /// Read and decompress BLTE content at location
    pub fn read_content(&mut self, location: &ArchiveLocation) -> ArchiveResult<Vec<u8>> {
        location.validate_hash()?;

        if let Some(ref key_store) = self.key_store {
            self.archive
                .read_blte_at_offset_with_keys(location.offset, location.size, key_store)
        } else {
            self.archive
                .read_blte_at_offset(location.offset, location.size)
        }
    }

    /// Read multiple locations in sequence
    pub fn read_multiple_content(
        &mut self,
        locations: &[ArchiveLocation],
    ) -> ArchiveResult<Vec<Vec<u8>>> {
        let mut results = Vec::with_capacity(locations.len());

        for location in locations {
            let content = self.read_content(location)?;
            results.push(content);
        }

        Ok(results)
    }

    /// Stream content in chunks for large files
    pub fn stream_content(
        &mut self,
        location: &ArchiveLocation,
        chunk_size: usize,
    ) -> ArchiveResult<ContentStream> {
        location.validate_hash()?;

        // Read the BLTE data first
        let blte_data = self
            .archive
            .read_at_offset(location.offset, location.size)?;

        // Parse BLTE structure
        let blte = BlteFile::read_options(
            &mut std::io::Cursor::new(&blte_data),
            binrw::Endian::Big,
            (),
        )?;

        Ok(ContentStream::new(
            blte,
            chunk_size,
            self.key_store.as_ref(),
        ))
    }

    /// Get current position
    pub fn position(&self) -> u64 {
        self.archive.position()
    }

    /// Set key store for decryption
    pub fn set_key_store(&mut self, key_store: TactKeyStore) {
        self.key_store = Some(key_store);
    }
}

impl ArchiveReader<File> {
    /// Open archive reader from file path
    pub fn open<P: AsRef<Path>>(path: P) -> ArchiveResult<Self> {
        let file = File::open(path)?;
        Ok(Self::new(file))
    }

    /// Open archive reader with keys from file path
    pub fn open_with_keys<P: AsRef<Path>>(path: P, key_store: TactKeyStore) -> ArchiveResult<Self> {
        let file = File::open(path)?;
        Ok(Self::with_keys(file, key_store))
    }
}

/// Streaming content reader for large BLTE files
pub struct ContentStream {
    /// Parsed BLTE file
    blte: BlteFile,
    /// Current chunk index
    current_chunk: usize,
    /// Chunk size for streaming
    #[allow(dead_code)] // Future streaming optimization
    chunk_size: usize,
    /// Optional key store for decryption
    key_store: Option<TactKeyStore>,
    /// Buffer for decompressed data
    buffer: Vec<u8>,
    /// Current position in buffer
    buffer_position: usize,
}

impl ContentStream {
    /// Create new content stream
    fn new(blte: BlteFile, chunk_size: usize, key_store: Option<&TactKeyStore>) -> Self {
        Self {
            blte,
            current_chunk: 0,
            chunk_size,
            key_store: key_store.cloned(),
            buffer: Vec::new(),
            buffer_position: 0,
        }
    }

    /// Read next chunk of decompressed data
    pub fn read_chunk(&mut self) -> ArchiveResult<Option<Vec<u8>>> {
        if self.current_chunk >= self.blte.chunks.len() {
            return Ok(None);
        }

        let chunk = &self.blte.chunks[self.current_chunk];

        let decompressed = if let Some(ref key_store) = self.key_store {
            // Use BLTE's decryption support
            chunk.decompress(self.current_chunk).or_else(|_| {
                // Try decryption if regular decompression fails
                crate::blte::decrypt_chunk_with_keys(&chunk.data, key_store, self.current_chunk)
            })?
        } else {
            chunk.decompress(self.current_chunk)?
        };

        self.current_chunk += 1;
        Ok(Some(decompressed))
    }

    /// Read all remaining content
    pub fn read_all(&self) -> ArchiveResult<Vec<u8>> {
        if let Some(ref key_store) = self.key_store {
            Ok(self.blte.decompress_with_keys(key_store)?)
        } else {
            Ok(self.blte.decompress()?)
        }
    }

    /// Check if more chunks are available
    pub fn has_more_chunks(&self) -> bool {
        self.current_chunk < self.blte.chunks.len()
    }

    /// Get total number of chunks
    pub fn total_chunks(&self) -> usize {
        self.blte.chunks.len()
    }

    /// Get current chunk index
    pub fn current_chunk_index(&self) -> usize {
        self.current_chunk
    }
}

impl Read for ContentStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Fill buffer if needed
        while self.buffer_position >= self.buffer.len() && self.has_more_chunks() {
            match self.read_chunk() {
                Ok(Some(chunk_data)) => {
                    self.buffer = chunk_data;
                    self.buffer_position = 0;
                }
                Ok(None) => break,
                Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
            }
        }

        // Return data from buffer
        if self.buffer_position < self.buffer.len() {
            let available = self.buffer.len() - self.buffer_position;
            let to_copy = buf.len().min(available);

            buf[..to_copy].copy_from_slice(
                &self.buffer[self.buffer_position..self.buffer_position + to_copy],
            );

            self.buffer_position += to_copy;
            Ok(to_copy)
        } else {
            Ok(0) // EOF
        }
    }
}

/// Archive content resolver that maps encoding keys to archive locations
pub struct ArchiveResolver {
    /// Map of encoding key to archive location
    index_map: std::collections::HashMap<[u8; 16], ArchiveLocation>,
}

#[allow(dead_code)] // Future archive resolution features
impl ArchiveResolver {
    /// Create new empty resolver
    pub fn new() -> Self {
        Self {
            index_map: std::collections::HashMap::new(),
        }
    }

    /// Add mapping from encoding key to archive location
    pub fn add_mapping(&mut self, encoding_key: [u8; 16], location: ArchiveLocation) {
        self.index_map.insert(encoding_key, location);
    }

    /// Find archive location for encoding key
    pub fn locate(&self, encoding_key: &[u8; 16]) -> Option<&ArchiveLocation> {
        self.index_map.get(encoding_key)
    }

    /// Get all encoding keys
    #[allow(dead_code)] // Future archive iteration feature
    pub fn encoding_keys(&self) -> impl Iterator<Item = &[u8; 16]> {
        self.index_map.keys()
    }

    /// Get number of mappings
    pub fn len(&self) -> usize {
        self.index_map.len()
    }

    /// Check if resolver is empty
    pub fn is_empty(&self) -> bool {
        self.index_map.is_empty()
    }
}

impl Default for ArchiveResolver {
    fn default() -> Self {
        Self::new()
    }
}

// BlteError conversion is already handled by the #[from] attribute in error.rs

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_archive_location_validation() {
        let valid =
            ArchiveLocation::new("1234567890abcdef1234567890abcdef".to_string(), 1000, 2000);
        assert!(valid.validate_hash().is_ok());

        let invalid_length = ArchiveLocation::new("short".to_string(), 1000, 2000);
        assert!(matches!(
            invalid_length.validate_hash(),
            Err(ArchiveError::InvalidHashLength(_))
        ));

        let invalid_hex = ArchiveLocation::new(
            "1234567890abcdef1234567890abcdeG".to_string(), // G is not hex
            1000,
            2000,
        );
        assert!(matches!(
            invalid_hex.validate_hash(),
            Err(ArchiveError::InvalidFormat(_))
        ));
    }

    #[test]
    fn test_archive_file_reading() {
        let data = vec![0x42u8; 1000];
        let cursor = Cursor::new(data);
        let mut archive = ArchiveFile::new(cursor);

        // Read from offset
        let result = archive
            .read_at_offset(100, 50)
            .expect("Operation should succeed");
        assert_eq!(result.len(), 50);
        assert_eq!(result[0], 0x42);
        assert_eq!(archive.position(), 150);
    }

    #[test]
    fn test_archive_resolver() {
        let mut resolver = ArchiveResolver::new();
        let encoding_key = [1u8; 16];
        let location =
            ArchiveLocation::new("1234567890abcdef1234567890abcdef".to_string(), 1000, 2000);

        resolver.add_mapping(encoding_key, location.clone());

        assert_eq!(resolver.len(), 1);
        assert_eq!(resolver.locate(&encoding_key), Some(&location));

        let missing_key = [2u8; 16];
        assert_eq!(resolver.locate(&missing_key), None);
    }

    #[test]
    fn test_content_stream_creation() {
        // Create a simple BLTE file for testing
        let test_data = b"Hello, Archive!";
        let blte = crate::blte::BlteFile::single_chunk(
            test_data.to_vec(),
            crate::blte::CompressionMode::None,
        )
        .expect("Operation should succeed");

        let stream = ContentStream::new(blte, 1024, None);
        assert_eq!(stream.total_chunks(), 1);
        assert!(stream.has_more_chunks());
        assert_eq!(stream.current_chunk_index(), 0);
    }
}

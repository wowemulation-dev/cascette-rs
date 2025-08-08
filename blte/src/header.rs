//! BLTE header parsing
//!
//! Handles parsing of BLTE file headers including chunk tables.

use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;
use tracing::debug;

use crate::{BLTE_MAGIC, Error, MD5_LENGTH, Md5, Result};

/// [BLTE][0] archive header / metadata.
///
/// [0]: https://wowdev.wiki/BLTE
#[derive(Debug, Clone)]
pub struct BLTEHeader {
    /// Length of the BLTE headers, in bytes.
    length: u32,

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Set to 0 if unknown.
    total_decompressed_size: u64,

    /// Chunk information
    ///
    /// When empty, the remainder of the BLTE stream contains a single chunk.
    chunks: Vec<ChunkInfo>,
}

impl BLTEHeader {
    /// Parse a BLTE header at the file's current position.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let mut magic = [0; BLTE_MAGIC.len()];
        f.read_exact(&mut magic)?;
        if magic != BLTE_MAGIC {
            return Err(Error::InvalidMagic(magic));
        }

        let length = f.read_u32::<BigEndian>()?;
        if length <= 8 + 4 + 24 {
            // We couldn't fit a BlockInfo entry, so this must be a single
            // stream.
            return Ok(BLTEHeader {
                length: length.max(8),
                total_decompressed_size: 0,
                chunks: Vec::with_capacity(0),
            });
        }

        if length > 65535 {
            // Probably invalid, number is arbitrary.
            // This allows up to 1638 or 2730 chunks, depending on format.
            return Err(Error::InvalidHeaderSize(length));
        }

        // We have a ChunkInfo struct with 1 or more BlockInfos
        let table_format = f.read_u8()?;
        debug!("Chunk table format: {table_format:#x}");
        if table_format != 0xf && table_format != 0x10 {
            return Err(Error::UnsupportedTableFormat(table_format));
        }
        let has_uncompressed_hash = table_format == 0x10;
        let chunk_count = f.read_u24::<BigEndian>()?;

        // How much header have we got length for chunks?
        let chunks_len = length - 8 - 4;
        // Length of a single chunk
        let chunk_len = if has_uncompressed_hash { 40 } else { 24 };

        debug!("Chunk count: {chunk_count}");

        // Is the header the correct size for the expected number of blocks?
        if chunks_len != chunk_count * chunk_len {
            return Err(Error::InvalidChunkCount(chunk_count));
        }

        let mut chunks = Vec::with_capacity(chunk_count as usize);
        let mut compressed_offset = u64::from(length);
        let mut decompressed_offset = 0;
        for _ in 0..chunk_count {
            let compressed_size = f.read_u32::<BigEndian>()?;
            let decompressed_size = f.read_u32::<BigEndian>()?;

            let mut compressed_hash = [0; MD5_LENGTH];
            f.read_exact(&mut compressed_hash)?;

            let decompressed_hash = if has_uncompressed_hash {
                let mut hash = [0; MD5_LENGTH];
                f.read_exact(&mut hash)?;
                Some(hash)
            } else {
                None
            };

            chunks.push(ChunkInfo {
                compressed_size,
                decompressed_size,
                compressed_hash,
                decompressed_hash,
                compressed_offset,
                decompressed_offset,
            });

            decompressed_offset += u64::from(decompressed_size);
            compressed_offset += u64::from(compressed_size);
        }

        Ok(Self {
            chunks,
            length,
            total_decompressed_size: decompressed_offset,
        })
    }

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Returns 0 if unknown.
    pub fn total_decompressed_size(&self) -> u64 {
        self.total_decompressed_size
    }

    /// Find the offset of `chunk`, relative to the start of the header block.
    ///
    /// Returns `None` if `chunk` is out of range.
    pub fn chunk_data_offset(&self, chunk: usize) -> Option<u64> {
        if chunk == 0 {
            return Some(self.length.into());
        }

        Some(self.chunks.get(chunk)?.compressed_offset)
    }

    /// Get total number of chunks
    pub fn chunk_count(&self) -> usize {
        self.chunks.len().max(1)
    }

    /// Get information about a chunk.
    ///
    /// Returns `None` if the stream consists of a single chunk, or `chunk` is
    /// out of range.
    pub fn get_chunk_info(&self, chunk: usize) -> Option<&ChunkInfo> {
        self.chunks.get(chunk)
    }
}

/// Information about a single chunk
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    /// The compressed size of the block, including block header byte(s).
    pub compressed_size: u32,

    /// The decompressed size of the block.
    ///
    /// For non-compressed blocks, this is `compressed_size - 1` (for the block
    /// header byte).
    pub decompressed_size: u32,

    /// The MD5 checksum of the compressed block, including header byte(s).
    ///
    /// Can be verified with [`BlteExtractor::verify_compressed_checksum`][].
    pub compressed_hash: Md5,

    /// The MD5 checksum of the block when decompressed.
    ///
    /// Only present for table format `0x10`.
    pub decompressed_hash: Option<Md5>,

    /// Offset of this data block, relative to the start of the header.
    pub compressed_offset: u64,

    /// Offset of this data block when decompressed, relative to the start of
    /// the decompressed file.
    pub decompressed_offset: u64,
}

impl ChunkInfo {
    // /// Create chunk info for single chunk mode
    // pub fn single_chunk(compressed_size: u32, decompressed_size: u32) -> Self {
    //     Self {
    //         compressed_size,
    //         decompressed_size,
    //         checksum: [0u8; 16], // No checksum for single chunk
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, ErrorKind};
    use super::*;

    #[test]
    fn test_single_chunk_header() {
        let data = [
            b'B', b'L', b'T', b'E', // Magic
            0x00, 0x00, 0x00, 0x00, // Header size = 0 (single chunk)
        ];

        let header = BLTEHeader::parse(&mut Cursor::new(&data)).unwrap();
        assert_eq!(header.chunk_count(), 1);
        assert_eq!(header.chunk_data_offset(0), Some(8));
        assert_eq!(header.chunk_data_offset(1), None);
        assert!(header.chunks.is_empty());
    }

    #[test]
    fn test_multi_chunk_header() {
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&60u32.to_be_bytes()); // Header size (8 + 4 + (2 * 24))

        // Chunk table
        data.push(0x0F); // Flags
        data.extend_from_slice(&[0x00, 0x00, 0x02]); // 2 chunks (3-byte big-endian)

        // Chunk 1
        data.extend_from_slice(&1000u32.to_be_bytes()); // Compressed size
        data.extend_from_slice(&2000u32.to_be_bytes()); // Decompressed size
        data.extend_from_slice(&[0xAA; 16]); // Checksum

        // Chunk 2
        data.extend_from_slice(&1500u32.to_be_bytes()); // Compressed size
        data.extend_from_slice(&3000u32.to_be_bytes()); // Decompressed size
        data.extend_from_slice(&[0xBB; 16]); // Checksum

        let header = BLTEHeader::parse(&mut Cursor::new(&data)).unwrap();
        assert_eq!(header.chunk_count(), 2);
        assert_eq!(header.chunk_data_offset(0), Some(60));
        assert_eq!(header.chunk_data_offset(1), Some(60 + 1000));
        assert_eq!(header.chunk_data_offset(2), None);

        // Check chunks
        assert_eq!(header.chunks.len(), 2);
        assert_eq!(header.chunks[0].compressed_size, 1000);
        assert_eq!(header.chunks[0].decompressed_size, 2000);
        assert_eq!(header.chunks[0].compressed_hash, [0xAA; 16]);
        assert_eq!(header.chunks[1].compressed_size, 1500);
        assert_eq!(header.chunks[1].decompressed_size, 3000);
        assert_eq!(header.chunks[1].compressed_hash, [0xBB; 16]);
    }

    #[test]
    fn test_invalid_magic() {
        let data = b"BAD!\0\0\0\0"; // Wrong magic
        let err = BLTEHeader::parse(&mut Cursor::new(data)).unwrap_err();
        assert!(matches!(err, Error::InvalidMagic(_)));
    }

    #[test]
    fn test_truncated_header() {
        let data = b"BLT"; // Too short
        let err = BLTEHeader::parse(&mut Cursor::new(data)).unwrap_err();
        assert!(matches!(err, Error::Io(e) if e.kind() == ErrorKind::UnexpectedEof));
    }
}

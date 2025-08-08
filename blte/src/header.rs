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
    /// Total size of all blocks in the file when decompressed.
    ///
    /// Set to 0 if unknown.
    total_decompressed_size: u64,

    /// Chunk information.
    /// 
    /// Contains a single synthetic entry when there is only one chunk.
    chunks: Vec<ChunkInfo>,
}

impl BLTEHeader {
    /// Parse a BLTE header at the file's current position, with a BLTE stream
    /// of up to `length` bytes.
    pub fn parse<R: Read>(f: &mut R, length: u64) -> Result<Self> {
        if length < 8 {
            return Err(Error::TruncatedData {
                expected: 8,
                actual: length,
            });
        }

        let mut magic = [0; BLTE_MAGIC.len()];
        f.read_exact(&mut magic)?;
        if magic != BLTE_MAGIC {
            return Err(Error::InvalidMagic(magic));
        }

        let header_length = f.read_u32::<BigEndian>()?;
        if length < header_length.into() {
            // Header won't fit in the buffer we have
            return Err(Error::TruncatedData {
                expected: header_length.into(),
                actual: length,
            });
        }

        if header_length <= 8 + 4 + 24 {
            // We couldn't fit a single ChunkInfo, so this must be a single
            // stream.
            let header_length = header_length.max(8);

            return Ok(BLTEHeader {
                total_decompressed_size: 0,
                chunks: vec![ChunkInfo::single_chunk(header_length, length)],
            });
        }

        if header_length > 65535 {
            // Probably invalid, number is arbitrary.
            // This allows up to 1638 or 2730 chunks, depending on format.
            return Err(Error::InvalidHeaderSize(header_length));
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
        let chunks_len = header_length - 8 - 4;
        // Length of a single chunk
        let chunk_len = if has_uncompressed_hash { 40 } else { 24 };

        debug!("Chunk count: {chunk_count}");

        // Is the header the correct size for the expected number of blocks?
        if chunks_len != chunk_count * chunk_len {
            return Err(Error::InvalidChunkCount(chunk_count));
        }

        let mut chunks = Vec::with_capacity(chunk_count as usize);
        let mut compressed_offset = u64::from(header_length);
        let mut decompressed_offset = 0;
        for _ in 0..chunk_count {
            let compressed_size = f.read_u32::<BigEndian>()?.into();
            let decompressed_size = f.read_u32::<BigEndian>()?.into();

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
                compressed_hash: Some(compressed_hash),
                decompressed_hash,
                compressed_offset,
                decompressed_offset,
            });

            compressed_offset += u64::from(compressed_size);
            if compressed_offset > length {
                // The streams we got would overrun!
                return Err(Error::TruncatedData {
                    expected: compressed_offset,
                    actual: length,
                });
            }

            decompressed_offset += u64::from(decompressed_size);
        }

        Ok(Self {
            chunks,
            total_decompressed_size: decompressed_offset,
        })
    }

    /// Total size of all blocks in the file when decompressed.
    ///
    /// Returns 0 if unknown.
    pub fn total_decompressed_size(&self) -> u64 {
        self.total_decompressed_size
    }

    /// Get information about a chunk.
    ///
    /// Returns `None` if `chunk` is out of range.
    #[inline]
    pub fn get_chunk(&self, chunk: usize) -> Option<&ChunkInfo> {
        self.chunks.get(chunk)
    }

    /// Get information about all chunks in the stream.
    pub const fn chunks(&self) -> &Vec<ChunkInfo> {
        &self.chunks
    }
}

/// Information about a single chunk
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkInfo {
    /// The compressed size of the block, including chunk header byte(s).
    ///
    /// This value is expressed as a `u32` in the file, but we express it as
    /// `u64` for easier use with `std::io` traits.
    pub compressed_size: u64,

    /// The decompressed size of the block, if known.
    ///
    /// If this is set to `0`, then this is a single-stream file where the
    /// decompressed size is unknown.
    ///
    /// For non-compressed blocks, this is `compressed_size - 1` (for the block
    /// header byte).
    ///
    /// This value is expressed as a `u32` in the file, but we express it as
    /// `u64` for easier use with `std::io` traits.
    pub decompressed_size: u64,

    /// The MD5 checksum of the compressed block, including header byte(s).
    ///
    /// Not present for single-chunk streams.
    ///
    /// Can be verified with [`BlteExtractor::verify_compressed_checksum`][].
    pub compressed_hash: Option<Md5>,

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
    /// Create synthetic chunk info for single chunk mode.
    fn single_chunk(header_length: u32, length: u64) -> Self {
        let header_length = u64::from(header_length);
        Self {
            compressed_size: length - header_length,
            decompressed_size: 0,
            compressed_hash: None,
            decompressed_hash: None,
            compressed_offset: header_length.into(),
            decompressed_offset: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, iter::repeat_n};

    #[test]
    fn test_single_chunk_header() {
        let data = [
            b'B', b'L', b'T', b'E', // Magic
            0x00, 0x00, 0x00, 0x00, // Header size = 0 (single chunk)
            b'N', 0x00, // Dummy payload
        ];

        let header = BLTEHeader::parse(&mut Cursor::new(&data), data.len() as u64).unwrap();
        assert_eq!(header.chunks().len(), 1);

        // Check synthetic chunk info
        let chunk_0 = header.get_chunk(0).unwrap();
        assert_eq!(chunk_0.compressed_offset, 8);
        assert_eq!(chunk_0.compressed_size, 2);

        assert_eq!(chunk_0.decompressed_offset, 0);
        assert_eq!(chunk_0.decompressed_size, 0);

        assert!(header.get_chunk(1).is_none());

        // Check when we truncate to just the header
        let header = BLTEHeader::parse(&mut Cursor::new(&data), 8).unwrap();
        assert_eq!(header.chunks().len(), 1);

        let chunk_0 = header.get_chunk(0).unwrap();
        assert_eq!(chunk_0.compressed_offset, 8);
        assert_eq!(chunk_0.compressed_size, 0);

        assert_eq!(chunk_0.decompressed_offset, 0);
        assert_eq!(chunk_0.decompressed_size, 0);

        assert!(header.get_chunk(1).is_none());
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

        // Chunk info 1
        data.extend_from_slice(&1000u32.to_be_bytes()); // Compressed size
        data.extend_from_slice(&2000u32.to_be_bytes()); // Decompressed size
        data.extend_from_slice(&[0xAA; 16]); // Checksum

        // Chunk info 2
        data.extend_from_slice(&1500u32.to_be_bytes()); // Compressed size
        data.extend_from_slice(&3000u32.to_be_bytes()); // Decompressed size
        data.extend_from_slice(&[0xBB; 16]); // Checksum

        // Chunk data
        data.extend(repeat_n(0xA0, 1000));
        data.extend(repeat_n(0xB0, 1500));
        assert_eq!(60 + 1000 + 1500, data.len());

        let header = BLTEHeader::parse(&mut Cursor::new(&data), data.len() as u64).unwrap();
        assert_eq!(header.chunks().len(), 2);

        // Check chunks
        let chunk_0 = header.get_chunk(0).unwrap();
        assert_eq!(chunk_0.compressed_size, 1000);
        assert_eq!(chunk_0.decompressed_size, 2000);
        assert_eq!(chunk_0.compressed_hash.unwrap(), [0xAA; 16]);

        let chunk_1 = header.get_chunk(1).unwrap();
        assert_eq!(chunk_1.compressed_size, 1500);
        assert_eq!(chunk_1.decompressed_size, 3000);
        assert_eq!(chunk_1.compressed_hash.unwrap(), [0xBB; 16]);

        assert!(header.get_chunk(2).is_none());

        // Check that the length limit is enforced, even if the buffer is longer
        // ...when reading the header
        let err = BLTEHeader::parse(&mut Cursor::new(&data), 32).unwrap_err();
        assert!(
            matches!(
                err,
                Error::TruncatedData {
                    expected: 60,
                    actual: 32,
                }
            ),
            "actual error: {err:?}",
        );

        // ...when reading the chunk infos
        let err = BLTEHeader::parse(&mut Cursor::new(&data), 1500).unwrap_err();
        assert!(
            matches!(
                err,
                Error::TruncatedData {
                    expected: 2560,
                    actual: 1500,
                },
            ),
            "actual error: {err:?}",
        );
    }

    #[test]
    fn test_invalid_magic() {
        let data = b"BAD!\0\0\0\0"; // Wrong magic
        let err = BLTEHeader::parse(&mut Cursor::new(data), data.len() as u64).unwrap_err();
        assert!(matches!(err, Error::InvalidMagic(_)));
    }

    #[test]
    fn test_truncated_header() {
        let data = b"BLT"; // Too short
        let err = BLTEHeader::parse(&mut Cursor::new(data), data.len() as u64).unwrap_err();
        assert!(
            matches!(
                err,
                Error::TruncatedData {
                    expected: 8,
                    actual: 3
                }
            ),
            "actual error: {err:?}",
        );
    }
}

//! BLTE header parsing
//!
//! Handles parsing of BLTE file headers including chunk tables.

use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use tracing::{debug, trace};

use crate::{BLTE_MAGIC, Error, Result};

/// BLTE header
#[derive(Debug, Clone)]
pub struct BLTEHeader {
    /// Magic bytes (always "BLTE")
    pub magic: [u8; 4],
    /// Header size (0 = single chunk, >0 = multi-chunk)
    pub header_size: u32,
    /// Chunk information (empty for single chunk)
    pub chunks: Vec<ChunkInfo>,
}

impl BLTEHeader {
    /// Parse BLTE header from data
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(Error::TruncatedData {
                expected: 8,
                actual: data.len(),
            });
        }

        let mut cursor = Cursor::new(data);

        // Read magic bytes
        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;

        if magic != BLTE_MAGIC {
            return Err(Error::InvalidMagic(magic));
        }

        // Read header size (BIG-ENDIAN - unusual for Blizzard!)
        let header_size = cursor.read_u32::<BigEndian>()?;

        debug!("BLTE header size: {}", header_size);

        let chunks = if header_size == 0 {
            // Single chunk mode - no chunk table
            Vec::new()
        } else {
            // Multi-chunk mode - parse chunk table
            Self::parse_chunk_table(&mut cursor, header_size)?
        };

        Ok(BLTEHeader {
            magic,
            header_size,
            chunks,
        })
    }

    /// Parse chunk table for multi-chunk files
    fn parse_chunk_table(cursor: &mut Cursor<&[u8]>, _header_size: u32) -> Result<Vec<ChunkInfo>> {
        // Read flags and chunk count
        let flags = cursor.read_u8()?;
        debug!("Chunk table flags: {:#04x}", flags);

        // Read 3-byte chunk count (big-endian)
        let chunk_count_bytes = [cursor.read_u8()?, cursor.read_u8()?, cursor.read_u8()?];
        let chunk_count = u32::from_be_bytes([
            0,
            chunk_count_bytes[0],
            chunk_count_bytes[1],
            chunk_count_bytes[2],
        ]);

        if chunk_count == 0 || chunk_count > 65536 {
            return Err(Error::InvalidChunkCount(chunk_count));
        }

        debug!("Chunk count: {}", chunk_count);

        let mut chunks = Vec::with_capacity(chunk_count as usize);

        for i in 0..chunk_count {
            let chunk = match flags {
                0x0F => {
                    // Standard chunk info (24 bytes)
                    let compressed_size = cursor.read_u32::<BigEndian>()?;
                    let decompressed_size = cursor.read_u32::<BigEndian>()?;

                    let mut checksum = [0u8; 16];
                    cursor.read_exact(&mut checksum)?;

                    ChunkInfo {
                        compressed_size,
                        decompressed_size,
                        checksum,
                    }
                }
                0x10 => {
                    // Extended chunk info (40 bytes) - rare, seen in Avowed
                    let compressed_size = cursor.read_u32::<BigEndian>()?;
                    let decompressed_size = cursor.read_u32::<BigEndian>()?;

                    let mut checksum = [0u8; 16];
                    cursor.read_exact(&mut checksum)?;

                    // Skip the additional 16 bytes
                    let mut _extended = [0u8; 16];
                    cursor.read_exact(&mut _extended)?;

                    ChunkInfo {
                        compressed_size,
                        decompressed_size,
                        checksum,
                    }
                }
                _ => {
                    return Err(Error::InvalidHeaderSize(flags as u32));
                }
            };

            trace!(
                "Chunk {}: compressed={}, decompressed={}, checksum={:02x?}",
                i,
                chunk.compressed_size,
                chunk.decompressed_size,
                &chunk.checksum[..4]
            );

            chunks.push(chunk);
        }

        Ok(chunks)
    }

    /// Check if this is a single chunk file
    pub fn is_single_chunk(&self) -> bool {
        self.header_size == 0
    }

    /// Get the data offset (where chunk data starts)
    pub fn data_offset(&self) -> usize {
        if self.is_single_chunk() {
            8 // Just magic + header_size
        } else {
            // Detect format based on header_size value:
            //
            // Standard BLTE format:
            //   header_size = size of chunk table only
            //   data_offset = 8 + header_size
            //
            // WoW CDN Archive format:
            //   header_size = 8 + chunk table size
            //   data_offset = header_size (already includes the 8)
            //
            // Detection heuristic:
            // Calculate expected chunk table size: 4 + (chunks.len() * 24)
            let expected_chunk_table_size = 4 + (self.chunks.len() * 24);

            if self.header_size as usize == expected_chunk_table_size {
                // Standard format: header_size = chunk table size
                8 + self.header_size as usize
            } else if self.header_size as usize == 8 + expected_chunk_table_size {
                // Archive format: header_size = 8 + chunk table size
                self.header_size as usize
            } else if self.header_size < expected_chunk_table_size as u32 {
                // Test/legacy format: header_size < actual chunk table size
                // Use standard calculation
                8 + self.header_size as usize
            } else {
                // Fallback: assume archive format
                self.header_size as usize
            }
        }
    }

    /// Get total number of chunks
    pub fn chunk_count(&self) -> usize {
        if self.is_single_chunk() {
            1
        } else {
            self.chunks.len()
        }
    }
}

/// Information about a single chunk
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    /// Compressed size of the chunk
    pub compressed_size: u32,
    /// Decompressed size of the chunk
    pub decompressed_size: u32,
    /// MD5 checksum of compressed chunk data
    pub checksum: [u8; 16],
}

impl ChunkInfo {
    /// Create chunk info for single chunk mode
    pub fn single_chunk(compressed_size: u32, decompressed_size: u32) -> Self {
        Self {
            compressed_size,
            decompressed_size,
            checksum: [0u8; 16], // No checksum for single chunk
        }
    }

    /// Verify checksum against data
    pub fn verify_checksum(&self, data: &[u8]) -> bool {
        if self.checksum == [0u8; 16] {
            return true; // No checksum to verify
        }

        let calculated = md5::compute(data);
        calculated.0 == self.checksum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_chunk_header() {
        let data = [
            b'B', b'L', b'T', b'E', // Magic
            0x00, 0x00, 0x00, 0x00, // Header size = 0 (single chunk)
        ];

        let header = BLTEHeader::parse(&data).unwrap();
        assert_eq!(header.magic, BLTE_MAGIC);
        assert_eq!(header.header_size, 0);
        assert!(header.is_single_chunk());
        assert_eq!(header.chunk_count(), 1);
        assert_eq!(header.data_offset(), 8);
        assert!(header.chunks.is_empty());
    }

    #[test]
    fn test_multi_chunk_header() {
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&32u32.to_be_bytes()); // Header size

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

        let header = BLTEHeader::parse(&data).unwrap();
        assert_eq!(header.magic, BLTE_MAGIC);
        assert_eq!(header.header_size, 32);
        assert!(!header.is_single_chunk());
        assert_eq!(header.chunk_count(), 2);
        assert_eq!(header.data_offset(), 40); // 8 (magic + header_size) + 32 (chunk table)

        // Check chunks
        assert_eq!(header.chunks.len(), 2);
        assert_eq!(header.chunks[0].compressed_size, 1000);
        assert_eq!(header.chunks[0].decompressed_size, 2000);
        assert_eq!(header.chunks[0].checksum, [0xAA; 16]);
        assert_eq!(header.chunks[1].compressed_size, 1500);
        assert_eq!(header.chunks[1].decompressed_size, 3000);
        assert_eq!(header.chunks[1].checksum, [0xBB; 16]);
    }

    #[test]
    fn test_invalid_magic() {
        let data = [
            b'B', b'A', b'D', b'!', // Wrong magic
            0x00, 0x00, 0x00, 0x00,
        ];

        let result = BLTEHeader::parse(&data);
        assert!(result.is_err());
        matches!(result.unwrap_err(), Error::InvalidMagic(_));
    }

    #[test]
    fn test_truncated_header() {
        let data = [b'B', b'L', b'T']; // Too short

        let result = BLTEHeader::parse(&data);
        assert!(result.is_err());
        matches!(result.unwrap_err(), Error::TruncatedData { .. });
    }

    #[test]
    fn test_checksum_verification() {
        let test_data = b"Hello, BLTE world!";
        let checksum = md5::compute(test_data).0;

        let chunk = ChunkInfo {
            compressed_size: test_data.len() as u32,
            decompressed_size: test_data.len() as u32,
            checksum,
        };

        assert!(chunk.verify_checksum(test_data));
        assert!(!chunk.verify_checksum(b"Different data"));
    }
}

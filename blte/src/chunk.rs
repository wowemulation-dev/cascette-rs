//! BLTE chunk handling and file structure

use crate::{BLTEHeader, Error, Result};

/// A complete BLTE file with header and data
#[derive(Debug, Clone)]
pub struct BLTEFile {
    /// BLTE header
    pub header: BLTEHeader,
    /// Raw data (everything after header)
    pub data: Vec<u8>,
}

/// A BLTE file that borrows its data (zero-copy)
#[derive(Debug)]
pub struct BLTEFileRef<'a> {
    /// BLTE header
    pub header: BLTEHeader,
    /// Raw data (everything after header)
    pub data: &'a [u8],
}

impl BLTEFile {
    /// Parse a BLTE file from bytes (allocates)
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let header = BLTEHeader::parse(&data)?;
        let data_offset = header.data_offset();

        if data.len() < data_offset {
            return Err(Error::TruncatedData {
                expected: data_offset,
                actual: data.len(),
            });
        }

        let chunk_data = data[data_offset..].to_vec();

        Ok(BLTEFile {
            header,
            data: chunk_data,
        })
    }

    /// Parse a BLTE file from bytes (zero-copy)
    pub fn parse_ref(data: &[u8]) -> Result<BLTEFileRef> {
        let header = BLTEHeader::parse(data)?;
        let data_offset = header.data_offset();

        if data.len() < data_offset {
            return Err(Error::TruncatedData {
                expected: data_offset,
                actual: data.len(),
            });
        }

        Ok(BLTEFileRef {
            header,
            data: &data[data_offset..],
        })
    }

    /// Get chunk data by index
    pub fn get_chunk_data(&self, chunk_index: usize) -> Result<ChunkData> {
        if self.header.is_single_chunk() {
            if chunk_index != 0 {
                return Err(Error::InvalidChunkCount(chunk_index as u32));
            }

            // For single chunk, we don't know the decompressed size ahead of time
            return Ok(ChunkData {
                data: self.data.clone(),
                compressed_size: self.data.len() as u32,
                decompressed_size: 0, // Unknown until decompressed
                checksum: [0u8; 16],  // No checksum for single chunk
            });
        }

        if chunk_index >= self.header.chunks.len() {
            return Err(Error::InvalidChunkCount(chunk_index as u32));
        }

        let chunk_info = &self.header.chunks[chunk_index];

        // Calculate offset for this chunk
        let mut offset = 0;
        for i in 0..chunk_index {
            offset += self.header.chunks[i].compressed_size as usize;
        }

        let end_offset = offset + chunk_info.compressed_size as usize;

        if end_offset > self.data.len() {
            return Err(Error::TruncatedData {
                expected: end_offset,
                actual: self.data.len(),
            });
        }

        let chunk_data = self.data[offset..end_offset].to_vec();

        Ok(ChunkData {
            data: chunk_data,
            compressed_size: chunk_info.compressed_size,
            decompressed_size: chunk_info.decompressed_size,
            checksum: chunk_info.checksum,
        })
    }

    /// Get all chunk data
    pub fn get_all_chunks(&self) -> Result<Vec<ChunkData>> {
        let mut chunks = Vec::new();
        let chunk_count = self.header.chunk_count();

        for i in 0..chunk_count {
            chunks.push(self.get_chunk_data(i)?);
        }

        Ok(chunks)
    }

    /// Check if the file is single-chunk
    pub fn is_single_chunk(&self) -> bool {
        self.header.is_single_chunk()
    }

    /// Get total number of chunks
    pub fn chunk_count(&self) -> usize {
        self.header.chunk_count()
    }

    /// Get total size of BLTE file (header + data)
    pub fn total_size(&self) -> usize {
        self.header.data_offset() + self.data.len()
    }

    /// Get raw BLTE file data (header + chunk data)
    pub fn raw_data(&self) -> Vec<u8> {
        let header_size = self.header.data_offset();
        let mut raw = Vec::with_capacity(self.total_size());

        // Reconstruct header
        raw.extend_from_slice(&crate::BLTE_MAGIC);
        raw.extend_from_slice(&self.header.header_size.to_be_bytes());

        // Add chunk table if multi-chunk
        if !self.header.is_single_chunk() {
            // Add chunk table data
            if self.header.chunks.is_empty() {
                // This shouldn't happen, but handle gracefully
                raw.extend_from_slice(&[0x0F, 0x00, 0x00, 0x00]); // flags + 0 chunks
            } else {
                raw.push(0x0F); // Standard chunk flags
                let chunk_count = self.header.chunks.len() as u32;
                raw.extend_from_slice(&chunk_count.to_be_bytes()[1..]); // 3-byte chunk count

                // Add chunk entries
                for chunk in &self.header.chunks {
                    raw.extend_from_slice(&chunk.compressed_size.to_be_bytes());
                    raw.extend_from_slice(&chunk.decompressed_size.to_be_bytes());
                    raw.extend_from_slice(&chunk.checksum);
                }
            }
        }

        // Pad to expected header size if needed
        while raw.len() < header_size {
            raw.push(0);
        }

        // Add chunk data
        raw.extend_from_slice(&self.data);
        raw
    }
}

impl<'a> BLTEFileRef<'a> {
    /// Get chunk data by index (zero-copy)
    pub fn get_chunk_data(&self, chunk_index: usize) -> Result<ChunkDataRef<'a>> {
        if self.header.is_single_chunk() {
            if chunk_index != 0 {
                return Err(Error::InvalidChunkCount(chunk_index as u32));
            }

            return Ok(ChunkDataRef {
                data: self.data,
                compressed_size: self.data.len() as u32,
                decompressed_size: 0, // Unknown until decompressed
                checksum: [0u8; 16],  // No checksum for single chunk
            });
        }

        if chunk_index >= self.header.chunks.len() {
            return Err(Error::InvalidChunkCount(chunk_index as u32));
        }

        let chunk_info = &self.header.chunks[chunk_index];

        // Calculate offset for this chunk
        let mut offset = 0;
        for i in 0..chunk_index {
            offset += self.header.chunks[i].compressed_size as usize;
        }

        let end_offset = offset + chunk_info.compressed_size as usize;

        if end_offset > self.data.len() {
            return Err(Error::TruncatedData {
                expected: end_offset,
                actual: self.data.len(),
            });
        }

        Ok(ChunkDataRef {
            data: &self.data[offset..end_offset],
            compressed_size: chunk_info.compressed_size,
            decompressed_size: chunk_info.decompressed_size,
            checksum: chunk_info.checksum,
        })
    }

    /// Get the total number of chunks
    pub fn chunk_count(&self) -> usize {
        self.header.chunk_count()
    }

    /// Check if the file is single-chunk
    pub fn is_single_chunk(&self) -> bool {
        self.header.is_single_chunk()
    }
}

/// Data for a single chunk
#[derive(Debug, Clone)]
pub struct ChunkData {
    /// Raw chunk data (compressed)
    pub data: Vec<u8>,
    /// Compressed size
    pub compressed_size: u32,
    /// Expected decompressed size (0 if unknown)
    pub decompressed_size: u32,
    /// MD5 checksum of compressed data
    pub checksum: [u8; 16],
}

/// Chunk data reference (zero-copy)
#[derive(Debug)]
pub struct ChunkDataRef<'a> {
    /// Raw chunk data reference (compressed)
    pub data: &'a [u8],
    /// Compressed size
    pub compressed_size: u32,
    /// Expected decompressed size (0 if unknown)
    pub decompressed_size: u32,
    /// MD5 checksum of compressed data
    pub checksum: [u8; 16],
}

impl ChunkData {
    /// Verify the checksum of this chunk
    pub fn verify_checksum(&self) -> bool {
        if self.checksum == [0u8; 16] {
            return true; // No checksum to verify
        }

        let calculated = md5::compute(&self.data);
        calculated.0 == self.checksum
    }

    /// Get the compression mode of this chunk
    pub fn compression_mode(&self) -> Result<crate::CompressionMode> {
        if self.data.is_empty() {
            return Err(Error::TruncatedData {
                expected: 1,
                actual: 0,
            });
        }

        crate::CompressionMode::from_byte(self.data[0])
            .ok_or(Error::UnknownCompressionMode(self.data[0]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_single_chunk_blte() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
        data.extend_from_slice(b"N"); // No compression
        data.extend_from_slice(b"Hello, BLTE!"); // Payload
        data
    }

    fn create_multi_chunk_blte() -> Vec<u8> {
        let chunk1_data = b"NHello";
        let chunk2_data = b"N, BLTE!";

        let mut data = Vec::new();

        // Calculate header size: 1 (flags) + 3 (chunk_count) + 2 * 24 (chunk_info)
        // Note: header_size does NOT include the 8 bytes for magic and header_size field itself
        let header_size = 1 + 3 + 2 * 24; // = 52

        // Header
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&(header_size as u32).to_be_bytes());

        // Chunk table
        data.push(0x0F); // Standard flags
        data.extend_from_slice(&[0x00, 0x00, 0x02]); // 2 chunks

        // Chunk 1 info
        data.extend_from_slice(&(chunk1_data.len() as u32).to_be_bytes()); // Compressed size
        data.extend_from_slice(&5u32.to_be_bytes()); // Decompressed: "Hello"
        data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

        // Chunk 2 info
        data.extend_from_slice(&(chunk2_data.len() as u32).to_be_bytes()); // Compressed size
        data.extend_from_slice(&7u32.to_be_bytes()); // Decompressed: ", BLTE!"
        data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

        // Chunk data
        data.extend_from_slice(chunk1_data); // Chunk 1
        data.extend_from_slice(chunk2_data); // Chunk 2

        data
    }

    #[test]
    fn test_single_chunk_file() {
        let data = create_single_chunk_blte();
        let blte_file = BLTEFile::parse(data).unwrap();

        assert!(blte_file.is_single_chunk());
        assert_eq!(blte_file.chunk_count(), 1);

        let chunk = blte_file.get_chunk_data(0).unwrap();
        assert_eq!(chunk.data, b"NHello, BLTE!");
        assert_eq!(chunk.compressed_size, 13);
        assert_eq!(chunk.decompressed_size, 0); // Unknown for single chunk
        assert_eq!(
            chunk.compression_mode().unwrap(),
            crate::CompressionMode::None
        );
    }

    #[test]
    fn test_multi_chunk_file() {
        let data = create_multi_chunk_blte();
        let blte_file = BLTEFile::parse(data).unwrap();

        assert!(!blte_file.is_single_chunk());
        assert_eq!(blte_file.chunk_count(), 2);

        // Test chunk 1
        let chunk1 = blte_file.get_chunk_data(0).unwrap();
        assert_eq!(chunk1.data, b"NHello");
        assert_eq!(chunk1.compressed_size, 6);
        assert_eq!(chunk1.decompressed_size, 5);
        assert_eq!(
            chunk1.compression_mode().unwrap(),
            crate::CompressionMode::None
        );

        // Test chunk 2
        let chunk2 = blte_file.get_chunk_data(1).unwrap();
        assert_eq!(chunk2.data, b"N, BLTE!");
        assert_eq!(chunk2.compressed_size, 8);
        assert_eq!(chunk2.decompressed_size, 7);
        assert_eq!(
            chunk2.compression_mode().unwrap(),
            crate::CompressionMode::None
        );
    }

    #[test]
    fn test_get_all_chunks() {
        let data = create_multi_chunk_blte();
        let blte_file = BLTEFile::parse(data).unwrap();

        let chunks = blte_file.get_all_chunks().unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].data, b"NHello");
        assert_eq!(chunks[1].data, b"N, BLTE!");
    }

    #[test]
    fn test_invalid_chunk_index() {
        let data = create_single_chunk_blte();
        let blte_file = BLTEFile::parse(data).unwrap();

        let result = blte_file.get_chunk_data(1);
        assert!(result.is_err());
        matches!(result.unwrap_err(), Error::InvalidChunkCount(_));
    }

    #[test]
    fn test_compression_mode_detection() {
        let test_cases = [
            (b'N', crate::CompressionMode::None),
            (b'Z', crate::CompressionMode::ZLib),
            (b'4', crate::CompressionMode::LZ4),
            (b'F', crate::CompressionMode::Frame),
            (b'E', crate::CompressionMode::Encrypted),
        ];

        for (byte, expected_mode) in test_cases {
            let chunk = ChunkData {
                data: vec![byte],
                compressed_size: 1,
                decompressed_size: 1,
                checksum: [0u8; 16],
            };

            assert_eq!(chunk.compression_mode().unwrap(), expected_mode);
        }
    }

    #[test]
    fn test_unknown_compression_mode() {
        let chunk = ChunkData {
            data: vec![b'X'], // Unknown mode
            compressed_size: 1,
            decompressed_size: 1,
            checksum: [0u8; 16],
        };

        let result = chunk.compression_mode();
        assert!(result.is_err());
        matches!(result.unwrap_err(), Error::UnknownCompressionMode(b'X'));
    }
}

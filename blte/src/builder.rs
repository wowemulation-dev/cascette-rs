//! BLTE builder pattern for convenient file construction
//!
//! This module provides a builder API for creating BLTE files with flexible
//! compression and chunking strategies.

use crate::compress::{compress_chunk, compress_data_multi, compress_data_single};
use crate::{BLTE_MAGIC, CompressionMode, Error, Result};
use std::collections::HashMap;

/// Builder for creating BLTE files with custom specifications
#[derive(Debug, Clone)]
pub struct BLTEBuilder {
    chunks: Vec<ChunkSpec>,
    default_compression: CompressionMode,
    default_chunk_size: usize,
    compression_level: Option<u8>,
    metadata: HashMap<String, String>,
}

/// Specification for a single chunk
#[derive(Debug, Clone)]
pub struct ChunkSpec {
    pub data: Vec<u8>,
    pub compression: Option<CompressionMode>,
    pub encryption: Option<EncryptionSpec>,
}

/// Specification for chunk encryption
#[derive(Debug, Clone)]
pub struct EncryptionSpec {
    pub key_name: u64,
    pub algorithm: EncryptionAlgorithm,
}

/// Encryption algorithm type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncryptionAlgorithm {
    Salsa20,
    ARC4,
}

/// Compression strategy for automatic mode selection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionStrategy {
    /// Automatically choose best compression for each chunk
    Auto,
    /// Use the same compression mode for all chunks
    Uniform(CompressionMode),
    /// Use specified compression level (for ZLib)
    WithLevel(CompressionMode, u8),
    /// Custom per-chunk specifications
    Custom,
}

impl Default for BLTEBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BLTEBuilder {
    /// Create a new BLTE builder
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            default_compression: CompressionMode::ZLib,
            default_chunk_size: 256 * 1024, // 256KB default
            compression_level: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the default compression mode for chunks
    pub fn with_compression(mut self, mode: CompressionMode) -> Self {
        self.default_compression = mode;
        self
    }

    /// Set the compression level (for ZLib, 1-9)
    pub fn with_compression_level(mut self, level: u8) -> Self {
        self.compression_level = Some(level.clamp(1, 9));
        self
    }

    /// Set the default chunk size for automatic chunking
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.default_chunk_size = size.max(1024); // Minimum 1KB
        self
    }

    /// Add metadata (for future use)
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add raw data to be compressed as a single chunk
    pub fn add_data(mut self, data: Vec<u8>) -> Self {
        self.chunks.push(ChunkSpec {
            data,
            compression: None,
            encryption: None,
        });
        self
    }

    /// Add data with specific compression mode
    pub fn add_data_with_compression(mut self, data: Vec<u8>, mode: CompressionMode) -> Self {
        self.chunks.push(ChunkSpec {
            data,
            compression: Some(mode),
            encryption: None,
        });
        self
    }

    /// Add a complete chunk specification
    pub fn add_chunk(mut self, spec: ChunkSpec) -> Self {
        self.chunks.push(spec);
        self
    }

    /// Add data that will be automatically chunked
    pub fn add_large_data(mut self, data: Vec<u8>) -> Self {
        // Split into chunks based on default_chunk_size
        for chunk in data.chunks(self.default_chunk_size) {
            self.chunks.push(ChunkSpec {
                data: chunk.to_vec(),
                compression: None,
                encryption: None,
            });
        }
        self
    }

    /// Build a single-chunk BLTE file
    pub fn build_single(self) -> Result<Vec<u8>> {
        if self.chunks.is_empty() {
            return Err(Error::DecompressionFailed("No data to compress".into()));
        }

        if self.chunks.len() > 1 {
            return Err(Error::DecompressionFailed(
                "Multiple chunks provided for single-chunk build".into(),
            ));
        }

        let chunk = &self.chunks[0];
        let compression = chunk.compression.unwrap_or(self.default_compression);

        compress_data_single(chunk.data.clone(), compression, self.compression_level)
    }

    /// Build a multi-chunk BLTE file
    pub fn build_multi(self) -> Result<Vec<u8>> {
        if self.chunks.is_empty() {
            return Err(Error::DecompressionFailed("No data to compress".into()));
        }

        // If only one chunk, use single-chunk format
        if self.chunks.len() == 1 {
            return self.build_single();
        }

        // Compress each chunk
        let mut compressed_chunks = Vec::with_capacity(self.chunks.len());
        let mut chunk_infos = Vec::with_capacity(self.chunks.len());

        for chunk_spec in &self.chunks {
            let compression = chunk_spec.compression.unwrap_or(self.default_compression);
            let compressed = compress_chunk(&chunk_spec.data, compression, self.compression_level)?;

            // Calculate MD5 checksum of the compressed data (including mode byte)
            let checksum = md5::compute(&compressed);

            chunk_infos.push(ChunkTableEntry {
                compressed_size: compressed.len() as u32,
                decompressed_size: chunk_spec.data.len() as u32,
                checksum: checksum.0,
            });

            compressed_chunks.push(compressed);
        }

        // Build the complete BLTE file
        build_multi_chunk_blte(&chunk_infos, &compressed_chunks)
    }

    /// Build BLTE file, automatically choosing single or multi-chunk format
    pub fn build(self) -> Result<Vec<u8>> {
        match self.chunks.len() {
            0 => Err(Error::DecompressionFailed("No data to compress".into())),
            1 => self.build_single(),
            _ => self.build_multi(),
        }
    }

    /// Build with automatic chunking of provided data
    pub fn build_from_data(
        data: Vec<u8>,
        chunk_size: usize,
        compression: CompressionMode,
        level: Option<u8>,
    ) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Err(Error::DecompressionFailed("No data to compress".into()));
        }

        if chunk_size == 0 {
            return Err(Error::DecompressionFailed("Invalid chunk size".into()));
        }

        // Use the multi-chunk compression function directly
        compress_data_multi(data, chunk_size, compression, level)
    }

    /// Apply a compression strategy to all chunks
    pub fn with_compression_strategy(mut self, strategy: CompressionStrategy) -> Self {
        match strategy {
            CompressionStrategy::Auto => {
                // Auto-select compression for each chunk
                for chunk in &mut self.chunks {
                    if chunk.compression.is_none() {
                        chunk.compression =
                            Some(crate::compress::auto_select_compression_mode(&chunk.data));
                    }
                }
            }
            CompressionStrategy::Uniform(mode) => {
                self.default_compression = mode;
            }
            CompressionStrategy::WithLevel(mode, level) => {
                self.default_compression = mode;
                self.compression_level = Some(level);
            }
            CompressionStrategy::Custom => {
                // Custom mode - chunks retain their individual settings
            }
        }
        self
    }
}

// Internal structures
#[derive(Debug)]
struct ChunkTableEntry {
    compressed_size: u32,
    decompressed_size: u32,
    checksum: [u8; 16],
}

fn build_multi_chunk_blte(
    chunk_infos: &[ChunkTableEntry],
    compressed_chunks: &[Vec<u8>],
) -> Result<Vec<u8>> {
    // Calculate header size
    let chunk_table_size = 1 + 3 + (24 * chunk_infos.len());
    let header_size = chunk_table_size as u32;

    // Calculate total size
    let data_size: usize = compressed_chunks.iter().map(|c| c.len()).sum();
    let total_size = 8 + chunk_table_size + data_size;

    let mut result = Vec::with_capacity(total_size);

    // BLTE magic
    result.extend_from_slice(&BLTE_MAGIC);

    // Header size (big-endian)
    result.extend_from_slice(&header_size.to_be_bytes());

    // Chunk table flags (0x0F for standard format)
    result.push(0x0F);

    // Chunk count (24-bit big-endian)
    let chunk_count = chunk_infos.len() as u32;
    result.push((chunk_count >> 16) as u8);
    result.push((chunk_count >> 8) as u8);
    result.push(chunk_count as u8);

    // Write chunk table entries
    for info in chunk_infos {
        result.extend_from_slice(&info.compressed_size.to_be_bytes());
        result.extend_from_slice(&info.decompressed_size.to_be_bytes());
        result.extend_from_slice(&info.checksum);
    }

    // Write compressed chunk data
    for chunk in compressed_chunks {
        result.extend_from_slice(chunk);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompress::decompress_blte;

    #[test]
    fn test_builder_single_chunk() {
        let data = b"Hello from BLTE builder!".to_vec();

        let blte = BLTEBuilder::new()
            .with_compression(CompressionMode::ZLib)
            .add_data(data.clone())
            .build_single()
            .unwrap();

        // Decompress and verify
        let decompressed = decompress_blte(blte, None).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_builder_multi_chunk() {
        let chunk1 = b"First chunk".to_vec();
        let chunk2 = b"Second chunk".to_vec();
        let chunk3 = b"Third chunk".to_vec();

        let blte = BLTEBuilder::new()
            .with_compression(CompressionMode::ZLib)
            .add_data(chunk1.clone())
            .add_data(chunk2.clone())
            .add_data(chunk3.clone())
            .build_multi()
            .unwrap();

        // Decompress and verify
        let decompressed = decompress_blte(blte, None).unwrap();
        let expected: Vec<u8> = [chunk1, chunk2, chunk3].concat();
        assert_eq!(decompressed, expected);
    }

    #[test]
    fn test_builder_auto_chunking() {
        let data = vec![b'A'; 1024]; // 1KB of data

        let blte = BLTEBuilder::new()
            .with_compression(CompressionMode::ZLib)
            .with_chunk_size(256) // 256 byte chunks
            .add_large_data(data.clone())
            .build()
            .unwrap();

        // Should create 4 chunks
        assert!(blte.len() > 8); // Has header

        // Decompress and verify
        let decompressed = decompress_blte(blte, None).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_builder_mixed_compression() {
        let blte = BLTEBuilder::new()
            .add_data_with_compression(b"Uncompressed".to_vec(), CompressionMode::None)
            .add_data_with_compression(b"ZLib compressed".to_vec(), CompressionMode::ZLib)
            .add_data_with_compression(b"LZ4 compressed".to_vec(), CompressionMode::LZ4)
            .build()
            .unwrap();

        // Decompress and verify
        let decompressed = decompress_blte(blte, None).unwrap();
        let expected = b"UncompressedZLib compressedLZ4 compressed".to_vec();
        assert_eq!(decompressed, expected);
    }

    #[test]
    fn test_builder_with_strategy() {
        let small_data = vec![b'A'; 50];
        let large_data = vec![b'B'; 5000];

        let blte = BLTEBuilder::new()
            .add_data(small_data.clone())
            .add_data(large_data.clone())
            .with_compression_strategy(CompressionStrategy::Auto)
            .build()
            .unwrap();

        // Decompress and verify
        let decompressed = decompress_blte(blte, None).unwrap();
        let expected: Vec<u8> = [small_data, large_data].concat();
        assert_eq!(decompressed, expected);
    }

    #[test]
    fn test_builder_from_data() {
        let data = vec![b'X'; 1024];

        let blte = BLTEBuilder::build_from_data(
            data.clone(),
            256, // 256 byte chunks
            CompressionMode::ZLib,
            Some(6),
        )
        .unwrap();

        // Decompress and verify
        let decompressed = decompress_blte(blte, None).unwrap();
        assert_eq!(decompressed, data);
    }
}

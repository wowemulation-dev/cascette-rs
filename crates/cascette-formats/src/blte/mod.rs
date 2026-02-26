//! BLTE (Block Table Encoded) format implementation
//!
//! BLTE is NGDP's container format for compressed and optionally encrypted content.
//! It provides block-based compression, encryption support, and efficient streaming
//! capabilities for game data delivery.
//!
//! # Features
//!
//! - Parser and builder for all BLTE modes
//! - Support for single and multi-chunk files
//! - Compression modes: None, `ZLib`, LZ4
//! - Encryption support: Salsa20, ARC4
//! - Round-trip validation

mod builder;
mod chunk;
mod compression;
mod encryption;
mod error;
mod header;

pub use builder::BlteBuilder;
pub use chunk::{ChunkData, CompressionMode};
pub use compression::{
    EncryptionSpec, compress_chunk, decompress_chunk, decrypt_chunk_with_keys,
    encrypt_chunk_with_key,
};
pub use encryption::{EncryptedHeader, EncryptionType};
pub use error::{BlteError, BlteResult};
pub use header::{BlteHeader, ChunkInfo, HeaderFlags};

use binrw::io::{Read, Seek, SeekFrom, Write};
use binrw::{BinRead, BinResult, BinWrite};
use cascette_crypto::TactKeyStore;

/// Complete BLTE file structure
#[derive(Debug, Clone)]
pub struct BlteFile {
    /// BLTE header
    pub header: BlteHeader,
    /// Chunk data
    pub chunks: Vec<ChunkData>,
}

impl BinRead for BlteFile {
    type Args<'a> = ();

    #[allow(clippy::cast_possible_truncation)]
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read header
        let header = BlteHeader::read_options(reader, endian, ())?;

        // Read chunks based on header type
        let mut chunks = Vec::new();

        if header.is_single_chunk() {
            // For single chunk, we need to read the rest of the file
            // Get current position to calculate remaining size
            let start_pos = reader.stream_position()?;
            let end_pos = reader.seek(SeekFrom::End(0))?;
            reader.seek(SeekFrom::Start(start_pos))?;

            // Safe cast: file positions shouldn't exceed usize
            let chunk_size = (end_pos - start_pos) as usize;
            if chunk_size > 0 {
                let chunk = ChunkData::read_options(reader, endian, (chunk_size,))?;
                chunks.push(chunk);
            }
        } else {
            // Multi-chunk: read based on chunk info
            if let Some(ref extended) = header.extended {
                for info in &extended.chunk_infos {
                    let chunk =
                        ChunkData::read_options(reader, endian, (info.compressed_size as usize,))?;
                    chunks.push(chunk);
                }
            }
        }

        Ok(Self { header, chunks })
    }
}

impl BinWrite for BlteFile {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write header
        self.header.write_options(writer, endian, ())?;

        // Write chunks
        for chunk in &self.chunks {
            chunk.write_options(writer, endian, ())?;
        }

        Ok(())
    }
}

impl BlteFile {
    /// Create a new single-chunk BLTE file
    pub fn single_chunk(data: Vec<u8>, mode: CompressionMode) -> BlteResult<Self> {
        Ok(Self {
            header: BlteHeader::single_chunk(),
            chunks: vec![ChunkData::new(data, mode)?],
        })
    }

    /// Create a new multi-chunk BLTE file
    pub fn multi_chunk(chunks: Vec<ChunkData>) -> BlteResult<Self> {
        let header = BlteHeader::multi_chunk(&chunks)?;
        Ok(Self { header, chunks })
    }

    /// Decompress all chunks and return the complete data
    ///
    /// Performance: Pre-allocates the output buffer based on the total
    /// decompressed size from chunk headers or chunk metadata.
    pub fn decompress(&self) -> BlteResult<Vec<u8>> {
        // Performance: Pre-allocate with estimated total decompressed size
        let total_size = self.estimate_decompressed_size();
        let mut result = Vec::with_capacity(total_size);

        for (index, chunk) in self.chunks.iter().enumerate() {
            let decompressed = chunk.decompress(index)?;
            result.extend_from_slice(&decompressed);
        }
        Ok(result)
    }

    /// Decompress all chunks with decryption support
    ///
    /// Encrypted BLTE files must use the extended (multi-chunk) header format.
    /// Single-chunk encrypted files are rejected.
    ///
    /// Performance: Pre-allocates the output buffer based on the total
    /// decompressed size from chunk headers or chunk metadata.
    pub fn decompress_with_keys(&self, key_store: &TactKeyStore) -> BlteResult<Vec<u8>> {
        // Single-chunk encrypted BLTE is not valid. The spec requires
        // encrypted content to use the extended header with a chunk table.
        if self.header.is_single_chunk()
            && self
                .chunks
                .first()
                .is_some_and(|c| c.mode == CompressionMode::Encrypted)
        {
            return Err(BlteError::SingleChunkEncrypted);
        }

        // Performance: Pre-allocate with estimated total decompressed size
        let total_size = self.estimate_decompressed_size();
        let mut result = Vec::with_capacity(total_size);

        for (index, chunk) in self.chunks.iter().enumerate() {
            let decompressed = if chunk.mode == CompressionMode::Encrypted {
                // Decrypt encrypted chunks
                decrypt_chunk_with_keys(&chunk.data, key_store, index)?
            } else {
                // Regular decompression for non-encrypted chunks
                chunk.decompress(index)?
            };
            result.extend_from_slice(&decompressed);
        }
        Ok(result)
    }

    /// Estimate total decompressed size from header or chunk metadata
    fn estimate_decompressed_size(&self) -> usize {
        // Try to get size from extended header first (most accurate)
        if let Some(ref extended) = self.header.extended {
            let total: u64 = extended
                .chunk_infos
                .iter()
                .map(|info| u64::from(info.decompressed_size))
                .sum();
            // Saturate to usize max to handle potential overflow gracefully
            return usize::try_from(total).unwrap_or(usize::MAX);
        }

        // Fall back to chunk-level estimates
        self.chunks.iter().map(|c| c.decompressed_size()).sum()
    }

    /// Compress data with automatic chunking
    pub fn compress(data: &[u8], chunk_size: usize, mode: CompressionMode) -> BlteResult<Self> {
        if data.len() <= chunk_size {
            // Single chunk
            Self::single_chunk(data.to_vec(), mode)
        } else {
            // Multi-chunk
            let mut chunks = Vec::new();
            let mut offset = 0;

            while offset < data.len() {
                let end = (offset + chunk_size).min(data.len());
                let chunk_data = data[offset..end].to_vec();
                chunks.push(ChunkData::new(chunk_data, mode)?);
                offset = end;
            }

            Self::multi_chunk(chunks)
        }
    }
}

impl crate::CascFormat for BlteFile {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        use std::io::Cursor;
        let mut cursor = Cursor::new(data);
        Self::read_options(&mut cursor, binrw::Endian::Big, ())
            .map_err(|e| Box::new(BlteError::BinRw(e)) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use std::io::Cursor;
        let mut data = Vec::new();
        let mut cursor = Cursor::new(&mut data);
        self.write_options(&mut cursor, binrw::Endian::Big, ())
            .map_err(|e| Box::new(BlteError::BinRw(e)) as Box<dyn std::error::Error>)?;
        Ok(data)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::CascFormat;

    #[test]
    fn test_single_chunk_encrypted_rejected() {
        // Manually construct a single-chunk encrypted BLTE (bypassing builder)
        // to verify that decompress_with_keys rejects it.
        let chunk = ChunkData::from_compressed(
            CompressionMode::Encrypted,
            vec![0x08, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            Some(9),
        );
        let blte = BlteFile {
            header: BlteHeader::single_chunk(),
            chunks: vec![chunk],
        };

        let key_store = cascette_crypto::TactKeyStore::new();
        let result = blte.decompress_with_keys(&key_store);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("Should fail")
                .to_string()
                .contains("multi-chunk")
        );
    }

    #[test]
    fn test_single_chunk_round_trip() {
        let data = b"Hello, BLTE!";
        let blte = BlteFile::single_chunk(data.to_vec(), CompressionMode::None)
            .expect("Test operation should succeed");

        // Use test utility for build-parse validation
        crate::test_utils::test_build_parse(&blte).expect("Build-parse should succeed");

        // Also verify the decompressed content is correct
        let built = blte.build().expect("Build should succeed");
        let parsed = BlteFile::parse(&built).expect("Parse should succeed");
        assert_eq!(parsed.decompress().expect("Operation should succeed"), data);
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use crate::blte::header::BLTE_MAGIC;
        use proptest::prelude::*;
        use proptest::test_runner::TestCaseError;

        /// Generate arbitrary compression modes (excluding deprecated Frame mode)
        fn compression_mode() -> impl Strategy<Value = CompressionMode> {
            prop_oneof![
                Just(CompressionMode::None),
                Just(CompressionMode::ZLib),
                Just(CompressionMode::LZ4),
            ]
        }

        /// Generate arbitrary data chunks (reasonable sizes for testing)
        fn data_chunk() -> impl Strategy<Value = Vec<u8>> {
            prop::collection::vec(any::<u8>(), 1..=10000)
        }

        /// Generate arbitrary header flags
        fn header_flags() -> impl Strategy<Value = HeaderFlags> {
            prop_oneof![Just(HeaderFlags::Standard), Just(HeaderFlags::Extended),]
        }

        proptest! {
                    /// Test that BLTE round-trip works for any valid data and compression mode
                    #[test]
                    fn blte_round_trip_always_works(
                        data in data_chunk(),
                        mode in compression_mode()
                    ) {
                        let blte = BlteFile::single_chunk(data.clone(), mode).map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let serialized = blte.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let deserialized = BlteFile::parse(&serialized).map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let decompressed = deserialized.decompress().map_err(|e| TestCaseError::fail(e.to_string()))?;

                        prop_assert_eq!(decompressed, data);
                    }

                    /// Test that invalid magic bytes are always rejected
                    fn invalid_magic_bytes_rejected(
                        magic in prop::array::uniform4(0u8..255).prop_filter("Not BLTE magic", |m| m != &BLTE_MAGIC)
                    ) {
                        let mut data = vec![0u8; 100];
                        data[0..4].copy_from_slice(&magic);

                        prop_assert!(BlteFile::parse(&data).is_err());
                    }

                    /// Test that multi-chunk files work correctly
                    #[test]
                    fn multi_chunk_round_trip(
                        chunks in prop::collection::vec(
                            (data_chunk(), compression_mode()),
                            1..10
                        ),
        _flags in header_flags()
                    ) {
                        // Create chunk data from test pairs
                        let chunk_data: Result<Vec<ChunkData>, BlteError> = chunks
                            .iter()
                            .map(|(data, mode)| ChunkData::new(data.clone(), *mode))
                            .collect();

                        let chunk_data = chunk_data.map_err(|e| TestCaseError::fail(e.to_string()))?;

                        // Create BLTE file with appropriate header
                        let blte = if chunk_data.len() == 1 {
                            BlteFile::single_chunk(chunks[0].0.clone(), chunks[0].1).map_err(|e| TestCaseError::fail(e.to_string()))?
                        } else {
                            BlteFile::multi_chunk(chunk_data).map_err(|e| TestCaseError::fail(e.to_string()))?
                        };

                        // Test round-trip
                        let serialized = blte.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let deserialized = BlteFile::parse(&serialized).map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let decompressed = deserialized.decompress().map_err(|e| TestCaseError::fail(e.to_string()))?;

                        // Concatenate original data for comparison
                        let expected: Vec<u8> = chunks.into_iter()
                            .flat_map(|(data, _)| data)
                            .collect();

                        prop_assert_eq!(decompressed, expected);
                    }

                    /// Test that compression mode bytes are always valid
                    fn compression_mode_bytes_valid(mode in compression_mode()) {
                        let byte = mode.as_byte();
                        prop_assert!(CompressionMode::from_byte(byte).is_some());
                        prop_assert_eq!(CompressionMode::from_byte(byte).expect("Valid compression mode byte"), mode);
                    }

                    /// Test that invalid compression mode bytes are rejected
                    #[test]
                    fn invalid_compression_modes_rejected(
                        invalid_mode in any::<u8>().prop_filter(
                            "Not a valid compression mode",
                            |&b| !matches!(b, b'N' | b'Z' | b'4' | b'E' | b'F')
                        )
                    ) {
                        prop_assert!(CompressionMode::from_byte(invalid_mode).is_none());
                    }

                    /// Test that chunk count validation works correctly
                    fn chunk_count_validation(
                        chunk_count in 1u32..=0xFF_FFFF_u32
                    ) {
                        // Create dummy chunk data
                        let chunks: Vec<ChunkData> = (0..chunk_count.min(100)) // Limit to 100 for test performance
                            .map(|i| ChunkData::new(vec![i as u8; 10], CompressionMode::None))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|e| TestCaseError::fail(e.to_string()))?;

                        if chunks.len() <= 0xFF_FFFF {
                            let result = BlteHeader::multi_chunk(&chunks);
                            prop_assert!(result.is_ok());
                        } else {
                            // This branch won't execute due to our limit above, but shows the logic
                            let result = BlteHeader::multi_chunk(&chunks);
                            prop_assert!(result.is_err());
                        }
                    }

                    /// Test that header size calculations are consistent
                    fn header_size_calculations_consistent(
                        chunk_count in 1usize..=100,
        flags in header_flags()
                    ) {
                        let chunks: Vec<ChunkData> = (0..chunk_count)
                            .map(|i| ChunkData::new(vec![i as u8; 10], CompressionMode::None))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|e| TestCaseError::fail(e.to_string()))?;

                        let header = BlteHeader::multi_chunk(&chunks).map_err(|e| TestCaseError::fail(e.to_string()))?;

                        let expected_size = if header.is_single_chunk() {
                            8 // magic + header_size
                        } else {
                            // header_size includes 8-byte preamble + 4 (flags + count) + chunk_infos
                            12 + (chunk_count * flags.chunk_info_size())
                        };

                        if !header.is_single_chunk() {
                            prop_assert_eq!(header.header_size as usize, expected_size);
                        }
                        // For single-chunk: data starts at offset 8
                        // For multi-chunk: header_size already includes the preamble
                        prop_assert_eq!(header.data_offset(), if header.is_single_chunk() { 8 } else { header.header_size as usize });
                    }

                    /// Test that checksums are deterministic
                    #[test]
                    fn checksums_are_deterministic(
                        data in data_chunk(),
                        mode in compression_mode()
                    ) {
                        let chunk1 = ChunkData::new(data.clone(), mode).map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let chunk2 = ChunkData::new(data, mode).map_err(|e| TestCaseError::fail(e.to_string()))?;

                        let info1 = ChunkInfo::from_chunk_data(&chunk1);
                        let info2 = ChunkInfo::from_chunk_data(&chunk2);

                        prop_assert_eq!(info1.checksum, info2.checksum);
                        prop_assert_eq!(info1.compressed_size, info2.compressed_size);
                        prop_assert_eq!(info1.decompressed_size, info2.decompressed_size);
                    }

                    /// Test that different data produces different checksums
                    fn different_data_different_checksums(
                        data1 in data_chunk(),
                        data2 in data_chunk(),
                        mode in compression_mode()
                    ) {
                        prop_assume!(data1 != data2); // Only test when data is actually different

                        let chunk1 = ChunkData::new(data1, mode).map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let chunk2 = ChunkData::new(data2, mode).map_err(|e| TestCaseError::fail(e.to_string()))?;

                        let info1 = ChunkInfo::from_chunk_data(&chunk1);
                        let info2 = ChunkInfo::from_chunk_data(&chunk2);

                        // Different data should produce different checksums
                        prop_assert_ne!(info1.checksum, info2.checksum);
                    }

                    /// Test that automatic chunking produces consistent results
                    fn automatic_chunking_consistent(
                        data in prop::collection::vec(any::<u8>(), 1..=100_000),
                        chunk_size in 1000usize..=50000,
                        mode in compression_mode()
                    ) {
                        let blte = BlteFile::compress(&data, chunk_size, mode).map_err(|e| TestCaseError::fail(e.to_string()))?;
                        let decompressed = blte.decompress().map_err(|e| TestCaseError::fail(e.to_string()))?;

                        prop_assert_eq!(decompressed, data.clone());

                        // Verify chunk count is reasonable
                        let expected_chunks = data.len().div_ceil(chunk_size);
                        prop_assert_eq!(blte.chunks.len(), expected_chunks.max(1));
                    }

                    /// Test that header flags parsing is bijective
                    fn header_flags_bijective(flags in header_flags()) {
                        let byte = flags as u8;
                        let parsed = HeaderFlags::from_byte(byte);

                        prop_assert!(parsed.is_some());
                        prop_assert_eq!(parsed.expect("Valid header flags"), flags);
                    }
                }
    }
}

//! BLTE chunk data structures and compression modes

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};

use super::error::{BlteError, BlteResult};

/// BLTE compression modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionMode {
    /// No compression (mode 'N')
    None = b'N',
    /// `ZLib` compression (mode 'Z')
    ZLib = b'Z',
    /// LZ4 compression (mode '4')
    LZ4 = b'4',
    /// Encrypted (mode 'E')
    Encrypted = b'E',
    /// Frame/Recursive BLTE (mode 'F') - deprecated
    #[deprecated(since = "0.1.0", note = "Recursive BLTE is deprecated")]
    Frame = b'F',
}

impl CompressionMode {
    /// Parse compression mode from byte
    #[allow(deprecated)]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'N' => Some(Self::None),
            b'Z' => Some(Self::ZLib),
            b'4' => Some(Self::LZ4),
            b'E' => Some(Self::Encrypted),
            b'F' => Some(Self::Frame),
            _ => None,
        }
    }

    /// Get the byte representation
    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

/// Chunk data with compression
#[derive(Debug, Clone)]
pub struct ChunkData {
    /// Compression mode
    pub mode: CompressionMode,
    /// Compressed data (without mode byte)
    pub data: Vec<u8>,
    /// Original decompressed size (for validation)
    decompressed_size: Option<usize>,
}

impl BinRead for ChunkData {
    type Args<'a> = (usize,); // compressed_size from ChunkInfo

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let (compressed_size,) = args;

        if compressed_size == 0 {
            return Err(binrw::Error::Custom {
                pos: 0,
                err: Box::new(BlteError::EmptyChunk),
            });
        }

        // Read compression mode byte directly using binrw
        let mode_byte = u8::read_options(reader, endian, ())?;

        let mode = CompressionMode::from_byte(mode_byte).ok_or_else(|| binrw::Error::Custom {
            pos: 0,
            err: Box::new(BlteError::UnknownCompressionMode(mode_byte)),
        })?;

        // Read remaining data
        let data_size = compressed_size - 1;
        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;

        Ok(Self {
            mode,
            data,
            decompressed_size: None,
        })
    }
}

impl BinWrite for ChunkData {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write compression mode byte
        writer.write_all(&[self.mode.as_byte()])?;

        // Write compressed data
        writer.write_all(&self.data)?;

        Ok(())
    }
}

impl ChunkData {
    /// Create a new chunk with specified compression mode
    pub fn new(data: Vec<u8>, mode: CompressionMode) -> BlteResult<Self> {
        let decompressed_size = data.len();

        if mode == CompressionMode::None {
            Ok(Self {
                mode,
                data,
                decompressed_size: Some(decompressed_size),
            })
        } else {
            // Use the compression module to compress the data
            let compressed_data = super::compression::compress_chunk(&data, mode)?;
            Ok(Self {
                mode,
                data: compressed_data,
                decompressed_size: Some(decompressed_size),
            })
        }
    }

    /// Create from already compressed data
    pub fn from_compressed(
        mode: CompressionMode,
        data: Vec<u8>,
        decompressed_size: Option<usize>,
    ) -> Self {
        Self {
            mode,
            data,
            decompressed_size,
        }
    }

    /// Get the compressed data including mode byte
    pub fn compressed_data(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(1 + self.data.len());
        result.push(self.mode.as_byte());
        result.extend_from_slice(&self.data);
        result
    }

    /// Get the compressed size (including mode byte)
    pub fn compressed_size(&self) -> usize {
        1 + self.data.len()
    }

    /// Get the decompressed size if known
    pub fn decompressed_size(&self) -> usize {
        self.decompressed_size.unwrap_or(self.data.len())
    }

    /// Decompress the chunk data
    pub fn decompress(&self, _chunk_index: usize) -> BlteResult<Vec<u8>> {
        use super::compression::decompress_chunk;
        decompress_chunk(&self.data, self.mode)
    }

    /// Verify checksum if provided
    pub fn verify_checksum(&self, checksum: &[u8; 16]) -> bool {
        use cascette_crypto::md5::ContentKey;

        if *checksum == [0u8; 16] {
            return true; // No checksum to verify
        }

        let compressed = self.compressed_data();
        let calculated = ContentKey::from_data(&compressed);
        calculated.as_bytes() == checksum
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_mode_conversion() {
        let modes = [
            (b'N', CompressionMode::None),
            (b'Z', CompressionMode::ZLib),
            (b'4', CompressionMode::LZ4),
            (b'E', CompressionMode::Encrypted),
        ];

        for (byte, mode) in modes {
            assert_eq!(CompressionMode::from_byte(byte), Some(mode));
            assert_eq!(mode.as_byte(), byte);
        }

        assert_eq!(CompressionMode::from_byte(b'X'), None);
    }

    #[test]
    fn test_chunk_data_uncompressed() {
        let data = b"Hello, BLTE!".to_vec();
        let chunk = ChunkData::new(data.clone(), CompressionMode::None)
            .expect("Test operation should succeed");

        assert_eq!(chunk.mode, CompressionMode::None);
        assert_eq!(chunk.data, data);
        assert_eq!(chunk.compressed_size(), data.len() + 1);
        assert_eq!(chunk.decompressed_size(), data.len());

        let decompressed = chunk.decompress(0).expect("Test operation should succeed");
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_chunk_data_round_trip() {
        let data = b"Test data".to_vec();
        let chunk =
            ChunkData::new(data, CompressionMode::None).expect("Test operation should succeed");

        // Serialize using write_options
        let mut writer = Vec::new();
        chunk
            .write_options(
                &mut std::io::Cursor::new(&mut writer),
                binrw::Endian::Big,
                (),
            )
            .expect("Test operation should succeed");

        // Deserialize using read_options
        let parsed = ChunkData::read_options(
            &mut std::io::Cursor::new(&writer),
            binrw::Endian::Big,
            (writer.len(),),
        )
        .expect("Test operation should succeed");

        assert_eq!(parsed.mode, chunk.mode);
        assert_eq!(parsed.data, chunk.data);
    }

    #[test]
    fn test_chunk_data_lz4_compression() {
        let original_data = b"This is test data for LZ4 compression in ChunkData.".to_vec();
        let chunk = ChunkData::new(original_data.clone(), CompressionMode::LZ4)
            .expect("Test operation should succeed");

        assert_eq!(chunk.mode, CompressionMode::LZ4);
        assert_eq!(chunk.decompressed_size(), original_data.len());

        // The compressed data should include the size header
        assert!(chunk.data.len() >= 8);

        // Decompress and verify
        let decompressed = chunk.decompress(0).expect("Test operation should succeed");
        assert_eq!(decompressed, original_data);
    }

    #[test]
    fn test_chunk_data_lz4_round_trip() {
        let test_cases = vec![
            b"Hello, LZ4!".to_vec(),
            b"This is a longer test case for LZ4 compression in BLTE chunks.".to_vec(),
            vec![0u8; 512],                // Large zeros
            (0..100).collect::<Vec<u8>>(), // Byte sequence
        ];

        for (i, original_data) in test_cases.into_iter().enumerate() {
            // Create chunk with LZ4 compression
            let chunk = ChunkData::new(original_data.clone(), CompressionMode::LZ4)
                .expect("LZ4 chunk creation should succeed in test");

            // Verify metadata
            assert_eq!(chunk.mode, CompressionMode::LZ4);
            assert_eq!(chunk.decompressed_size(), original_data.len());

            // Decompress and verify
            let decompressed = chunk
                .decompress(0)
                .expect("LZ4 chunk decompression should succeed in test");

            assert_eq!(
                decompressed, original_data,
                "LZ4 round-trip failed for test case {i}"
            );
        }
    }

    #[test]
    fn test_chunk_data_lz4_serialization() {
        let original_data = b"LZ4 serialization test data".to_vec();
        let chunk = ChunkData::new(original_data.clone(), CompressionMode::LZ4)
            .expect("Test operation should succeed");

        // Serialize using write_options
        let mut writer = Vec::new();
        chunk
            .write_options(
                &mut std::io::Cursor::new(&mut writer),
                binrw::Endian::Big,
                (),
            )
            .expect("Test operation should succeed");

        // First byte should be the mode (0x34)
        assert_eq!(writer[0], b'4');

        // Deserialize using read_options
        let parsed = ChunkData::read_options(
            &mut std::io::Cursor::new(&writer),
            binrw::Endian::Big,
            (writer.len(),),
        )
        .expect("Test operation should succeed");

        assert_eq!(parsed.mode, CompressionMode::LZ4);
        assert_eq!(parsed.data, chunk.data);

        // Verify round-trip decompression
        let decompressed = parsed.decompress(0).expect("Test operation should succeed");
        assert_eq!(decompressed, original_data);
    }
}

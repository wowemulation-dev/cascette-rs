//! BLTE compression functionality
//!
//! This module provides compression support for all BLTE modes:
//! - Mode 'N' (None): No compression
//! - Mode 'Z' (ZLib): ZLib compression with configurable levels
//! - Mode '4' (LZ4): LZ4 compression
//! - Mode 'F' (Frame): Recursive BLTE compression
//! - Mode 'E' (Encrypted): Encryption with Salsa20 or ARC4

use crate::{BLTE_MAGIC, CompressionMode, Error, Result};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;

/// Compress data using the specified BLTE compression mode
///
/// # Arguments
/// * `data` - Raw data to compress
/// * `mode` - Compression mode to use
/// * `level` - Optional compression level (used for ZLib, 1-9)
///
/// # Returns
/// Compressed data with mode byte prefix
pub fn compress_chunk(data: &[u8], mode: CompressionMode, level: Option<u8>) -> Result<Vec<u8>> {
    match mode {
        CompressionMode::None => compress_none(data),
        CompressionMode::ZLib => compress_zlib(data, level.unwrap_or(6)),
        CompressionMode::LZ4 => compress_lz4(data),
        CompressionMode::Frame => compress_frame(data, level),
        CompressionMode::Encrypted => Err(Error::DecompressionFailed(
            "Use compress_encrypted() for encryption mode".into(),
        )),
    }
}

/// Mode 'N' - No compression
pub fn compress_none(data: &[u8]) -> Result<Vec<u8>> {
    let mut result = Vec::with_capacity(data.len() + 1);
    result.push(b'N');
    result.extend_from_slice(data);
    Ok(result)
}

/// Mode 'Z' - ZLib compression
fn compress_zlib(data: &[u8], level: u8) -> Result<Vec<u8>> {
    let compression_level = match level {
        0 => Compression::none(),
        1 => Compression::fast(),
        2..=5 => Compression::new(level as u32),
        6 => Compression::default(),
        7..=8 => Compression::new(level as u32),
        9 => Compression::best(),
        _ => Compression::default(),
    };

    let mut encoder = ZlibEncoder::new(Vec::new(), compression_level);
    encoder.write_all(data).map_err(Error::Io)?;
    let compressed = encoder.finish().map_err(Error::Io)?;

    let mut result = Vec::with_capacity(compressed.len() + 1);
    result.push(b'Z');
    result.extend_from_slice(&compressed);
    Ok(result)
}

/// Mode '4' - LZ4 compression
fn compress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    // Compress data using lz4_flex (without size prefix)
    let compressed = lz4_flex::compress(data);

    // Build LZ4 BLTE format: mode + decompressed_size + compressed_size + compressed_data
    let mut result = Vec::with_capacity(9 + compressed.len());
    result.push(b'4');
    result.extend_from_slice(&(data.len() as u32).to_le_bytes());
    result.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    result.extend_from_slice(&compressed);
    Ok(result)
}

/// Mode 'F' - Recursive BLTE compression
fn compress_frame(data: &[u8], level: Option<u8>) -> Result<Vec<u8>> {
    // First compress the data with ZLib
    let compressed_inner = compress_zlib(data, level.unwrap_or(6))?;

    // Create a single-chunk BLTE frame
    let blte_data = create_single_chunk_blte(&compressed_inner)?;

    // Add the 'F' mode byte
    let mut result = Vec::with_capacity(blte_data.len() + 1);
    result.push(b'F');
    result.extend_from_slice(&blte_data);
    Ok(result)
}

/// Create a single-chunk BLTE file
pub fn create_single_chunk_blte(data: &[u8]) -> Result<Vec<u8>> {
    let mut result = Vec::with_capacity(data.len() + 8);

    // BLTE magic
    result.extend_from_slice(&BLTE_MAGIC);

    // Header size (0 for single chunk)
    result.extend_from_slice(&0u32.to_be_bytes());

    // Chunk data
    result.extend_from_slice(data);

    Ok(result)
}

/// Compress data into a complete BLTE file
///
/// # Arguments
/// * `data` - Raw data to compress
/// * `mode` - Compression mode to use
/// * `level` - Optional compression level
///
/// # Returns
/// Complete BLTE file with header
pub fn compress_data_single(
    data: Vec<u8>,
    mode: CompressionMode,
    level: Option<u8>,
) -> Result<Vec<u8>> {
    // Compress the chunk
    let compressed_chunk = compress_chunk(&data, mode, level)?;

    // Create single-chunk BLTE file
    create_single_chunk_blte(&compressed_chunk)
}

/// Compress data into multiple chunks
///
/// # Arguments
/// * `data` - Raw data to compress
/// * `chunk_size` - Target size for each chunk (before compression)
/// * `mode` - Compression mode to use
/// * `level` - Optional compression level
///
/// # Returns
/// Complete multi-chunk BLTE file with header and chunk table
pub fn compress_data_multi(
    data: Vec<u8>,
    chunk_size: usize,
    mode: CompressionMode,
    level: Option<u8>,
) -> Result<Vec<u8>> {
    if chunk_size == 0 {
        return Err(Error::DecompressionFailed(
            "Chunk size cannot be zero".into(),
        ));
    }

    // Split data into chunks
    let chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();
    let num_chunks = chunks.len();

    if num_chunks == 1 {
        // Use single-chunk format for efficiency
        return compress_data_single(data, mode, level);
    }

    // Compress each chunk and calculate metadata
    let mut compressed_chunks = Vec::with_capacity(num_chunks);
    let mut chunk_infos = Vec::with_capacity(num_chunks);

    for chunk_data in chunks.iter() {
        let compressed = compress_chunk(chunk_data, mode, level)?;

        // Calculate MD5 checksum of compressed data (including mode byte)
        let checksum = md5::compute(&compressed);

        chunk_infos.push(ChunkTableEntry {
            compressed_size: compressed.len() as u32,
            decompressed_size: chunk_data.len() as u32,
            checksum: checksum.0,
        });

        compressed_chunks.push(compressed);
    }

    // Build the BLTE file
    build_multi_chunk_blte(&chunk_infos, &compressed_chunks)
}

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
    // Header: 4 (magic) + 4 (header size) + 1 (flags) + 3 (chunk count) + (24 * num_chunks)
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
        // Compressed size (big-endian)
        result.extend_from_slice(&info.compressed_size.to_be_bytes());
        // Decompressed size (big-endian)
        result.extend_from_slice(&info.decompressed_size.to_be_bytes());
        // MD5 checksum
        result.extend_from_slice(&info.checksum);
    }

    // Write compressed chunk data
    for chunk in compressed_chunks {
        result.extend_from_slice(chunk);
    }

    Ok(result)
}

/// Choose optimal compression mode for data
///
/// Analyzes data characteristics to select the best compression mode
pub fn auto_select_compression_mode(data: &[u8]) -> CompressionMode {
    // Simple heuristic: use size and entropy estimation
    if data.len() < 256 {
        // Small data doesn't compress well
        CompressionMode::None
    } else if is_likely_compressed(data) {
        // Already compressed data (high entropy)
        CompressionMode::None
    } else if data.len() > 1024 * 1024 {
        // Large files benefit from LZ4's speed
        CompressionMode::LZ4
    } else {
        // Default to ZLib for good balance
        CompressionMode::ZLib
    }
}

/// Simple entropy check to detect already-compressed data
fn is_likely_compressed(data: &[u8]) -> bool {
    if data.len() < 256 {
        return false;
    }

    // Sample first 256 bytes for byte frequency
    let sample = &data[..256.min(data.len())];
    let mut freq = [0u32; 256];

    for &byte in sample {
        freq[byte as usize] += 1;
    }

    // Count unique bytes
    let unique_bytes = freq.iter().filter(|&&count| count > 0).count();

    // High entropy if most byte values appear
    unique_bytes > 200
}

/// Encryption method for BLTE mode 'E'
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncryptionMethod {
    /// Salsa20 stream cipher (modern)
    Salsa20,
    /// ARC4/RC4 stream cipher (legacy)
    ARC4,
}

/// Encrypt data using BLTE mode 'E' with specified encryption method
///
/// # Arguments
/// * `data` - Raw data to encrypt (can be pre-compressed)
/// * `method` - Encryption method (Salsa20 or ARC4)
/// * `key` - 16-byte TACT encryption key
/// * `iv` - 4-byte initialization vector
/// * `block_index` - Block index for multi-chunk files
///
/// # Returns
/// Encrypted data with mode byte 'E' prefix
pub fn compress_encrypted(
    data: &[u8],
    method: EncryptionMethod,
    key: &[u8; 16],
    iv: &[u8; 4],
    block_index: usize,
) -> Result<Vec<u8>> {
    let encrypted = match method {
        EncryptionMethod::Salsa20 => ngdp_crypto::encrypt_salsa20(data, key, iv, block_index)
            .map_err(|e| Error::DecompressionFailed(format!("Salsa20 encryption failed: {e}")))?,
        EncryptionMethod::ARC4 => ngdp_crypto::encrypt_arc4(data, key, iv, block_index)
            .map_err(|e| Error::DecompressionFailed(format!("ARC4 encryption failed: {e}")))?,
    };

    let mut result = Vec::with_capacity(encrypted.len() + 1);
    result.push(b'E');
    result.extend_from_slice(&encrypted);
    Ok(result)
}

/// Create encrypted single-chunk BLTE file
///
/// This compresses the data first (if needed), then encrypts it.
///
/// # Arguments
/// * `data` - Raw data to process
/// * `compression` - Optional compression mode to apply before encryption
/// * `compression_level` - Compression level (for ZLib)
/// * `encryption` - Encryption method
/// * `key` - 16-byte TACT key
/// * `iv` - 4-byte IV
///
/// # Returns
/// Complete BLTE file with encrypted content
pub fn compress_data_encrypted_single(
    data: Vec<u8>,
    compression: Option<CompressionMode>,
    compression_level: Option<u8>,
    encryption: EncryptionMethod,
    key: &[u8; 16],
    iv: &[u8; 4],
) -> Result<Vec<u8>> {
    // Step 1: Apply compression if requested
    let processed_data = if let Some(comp_mode) = compression {
        if comp_mode == CompressionMode::Encrypted {
            return Err(Error::DecompressionFailed(
                "Cannot use Encrypted mode as compression before encryption".into(),
            ));
        }

        compress_chunk(&data, comp_mode, compression_level)?
    } else {
        data
    };

    // Step 2: Encrypt the data (with mode 'E' prefix)
    let encrypted_chunk = compress_encrypted(&processed_data, encryption, key, iv, 0)?;

    // Step 3: Create single-chunk BLTE file
    create_single_chunk_blte(&encrypted_chunk)
}

/// Create encrypted multi-chunk BLTE file
///
/// Splits data into chunks, compresses each (if needed), then encrypts each chunk.
///
/// # Arguments
/// * `data` - Raw data to process
/// * `chunk_size` - Size of each chunk before processing
/// * `compression` - Optional compression mode
/// * `compression_level` - Compression level
/// * `encryption` - Encryption method
/// * `key` - 16-byte TACT key
/// * `iv` - 4-byte IV
///
/// # Returns
/// Complete multi-chunk BLTE file with encrypted chunks
pub fn compress_data_encrypted_multi(
    data: Vec<u8>,
    chunk_size: usize,
    compression: Option<CompressionMode>,
    compression_level: Option<u8>,
    encryption: EncryptionMethod,
    key: &[u8; 16],
    iv: &[u8; 4],
) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Err(Error::DecompressionFailed(
            "Cannot encrypt empty data".into(),
        ));
    }

    if chunk_size == 0 {
        return Err(Error::InvalidChunkCount(0));
    }

    // Validate compression mode
    if let Some(comp_mode) = compression {
        if comp_mode == CompressionMode::Encrypted {
            return Err(Error::DecompressionFailed(
                "Cannot use Encrypted mode as compression before encryption".into(),
            ));
        }
    }

    // Split data into chunks
    let chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();
    let mut compressed_chunks = Vec::with_capacity(chunks.len());
    let mut chunk_infos = Vec::with_capacity(chunks.len());

    for (i, chunk) in chunks.iter().enumerate() {
        // Step 1: Compress if requested
        let processed_chunk = if let Some(comp_mode) = compression {
            compress_chunk(chunk, comp_mode, compression_level)?
        } else {
            chunk.to_vec()
        };

        // Step 2: Encrypt the chunk (with block index)
        let encrypted_chunk = compress_encrypted(&processed_chunk, encryption, key, iv, i)?;

        // Calculate checksum on encrypted data
        let checksum = md5::compute(&encrypted_chunk).0;

        let info = ChunkTableEntry {
            compressed_size: encrypted_chunk.len() as u32,
            decompressed_size: chunk.len() as u32,
            checksum,
        };

        compressed_chunks.push(encrypted_chunk);
        chunk_infos.push(info);
    }

    build_multi_chunk_blte(&chunk_infos, &compressed_chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_none() {
        let data = b"Hello, World!";
        let compressed = compress_chunk(data, CompressionMode::None, None).unwrap();
        assert_eq!(compressed[0], b'N');
        assert_eq!(&compressed[1..], data);
    }

    #[test]
    fn test_compress_zlib() {
        let data = b"Hello, World! Hello, World! Hello, World!";
        let compressed = compress_chunk(data, CompressionMode::ZLib, Some(6)).unwrap();
        assert_eq!(compressed[0], b'Z');
        // ZLib should compress repeated data
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_compress_lz4() {
        let data = b"Hello, World! Hello, World! Hello, World!";
        let compressed = compress_chunk(data, CompressionMode::LZ4, None).unwrap();
        assert_eq!(compressed[0], b'4');
        // LZ4 should compress repeated data
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_single_chunk_blte() {
        let data = b"Test data";
        let blte = compress_data_single(data.to_vec(), CompressionMode::None, None).unwrap();

        // Check BLTE magic
        assert_eq!(&blte[0..4], &BLTE_MAGIC);
        // Check header size (0 for single chunk)
        assert_eq!(&blte[4..8], &[0, 0, 0, 0]);
        // Check mode byte
        assert_eq!(blte[8], b'N');
        // Check data
        assert_eq!(&blte[9..], data);
    }

    #[test]
    fn test_round_trip_compression() {
        use crate::decompress::decompress_blte;

        let original = b"This is test data that should round-trip correctly!".to_vec();

        // Test each compression mode
        for mode in [
            CompressionMode::None,
            CompressionMode::ZLib,
            CompressionMode::LZ4,
        ] {
            let compressed = compress_data_single(original.clone(), mode, None).unwrap();
            let decompressed = decompress_blte(compressed.clone(), None).unwrap();
            assert_eq!(
                decompressed, original,
                "Round-trip failed for mode {mode:?}"
            );
        }
    }

    #[test]
    fn test_multi_chunk_compression() {
        let data = vec![b'A'; 1024]; // 1KB of 'A's
        let compressed =
            compress_data_multi(data.clone(), 256, CompressionMode::ZLib, None).unwrap();

        // Should have created 4 chunks
        assert_eq!(&compressed[0..4], &BLTE_MAGIC);

        // Header size should be non-zero for multi-chunk
        let header_size =
            u32::from_be_bytes([compressed[4], compressed[5], compressed[6], compressed[7]]);
        assert!(header_size > 0);
    }

    #[test]
    fn test_auto_compression_selection() {
        // Small data -> None
        let small = vec![0u8; 100];
        assert_eq!(auto_select_compression_mode(&small), CompressionMode::None);

        // Compressed data -> None
        let compressed: Vec<u8> = (0..=255).collect();
        assert_eq!(
            auto_select_compression_mode(&compressed),
            CompressionMode::None
        );

        // Large data -> LZ4
        let large = vec![b'A'; 2 * 1024 * 1024];
        assert_eq!(auto_select_compression_mode(&large), CompressionMode::LZ4);

        // Normal data -> ZLib
        let normal = vec![b'A'; 10000];
        assert_eq!(auto_select_compression_mode(&normal), CompressionMode::ZLib);
    }

    #[test]
    fn test_encrypt_salsa20() {
        let data = b"Hello, encrypted BLTE world!";
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;

        let encrypted =
            compress_encrypted(data, EncryptionMethod::Salsa20, &key, &iv, block_index).unwrap();

        // Should have 'E' prefix
        assert_eq!(encrypted[0], b'E');

        // Should be different from original
        assert_ne!(&encrypted[1..], data);

        // Length should be original length + 1 (for mode byte)
        assert_eq!(encrypted.len(), data.len() + 1);
    }

    #[test]
    fn test_encrypt_arc4() {
        let data = b"Hello, encrypted BLTE world!";
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;

        let encrypted =
            compress_encrypted(data, EncryptionMethod::ARC4, &key, &iv, block_index).unwrap();

        // Should have 'E' prefix
        assert_eq!(encrypted[0], b'E');

        // Should be different from original
        assert_ne!(&encrypted[1..], data);

        // Length should be original length + 1 (for mode byte)
        assert_eq!(encrypted.len(), data.len() + 1);
    }

    #[test]
    fn test_encrypted_single_chunk_blte() {
        let data = b"Test data for encrypted BLTE".to_vec();
        let key = [0xAAu8; 16];
        let iv = [0xBB, 0xCC, 0xDD, 0xEE];

        let encrypted_blte = compress_data_encrypted_single(
            data.clone(),
            None,
            None,
            EncryptionMethod::Salsa20,
            &key,
            &iv,
        )
        .unwrap();

        // Should be a valid BLTE file
        assert_eq!(&encrypted_blte[0..4], &BLTE_MAGIC);

        // Header size should be 0 for single chunk
        assert_eq!(&encrypted_blte[4..8], &[0, 0, 0, 0]);

        // Data should start with 'E' mode byte
        assert_eq!(encrypted_blte[8], b'E');
    }

    #[test]
    fn test_encrypted_with_compression() {
        let data = b"This is test data that compresses well: AAAAAAAAAAAABBBBBBBBBBBBCCCCCCCCCCCC"
            .to_vec();
        let key = [0x12u8; 16];
        let iv = [0x34, 0x56, 0x78, 0x9A];

        let encrypted_blte = compress_data_encrypted_single(
            data.clone(),
            Some(CompressionMode::ZLib),
            Some(6),
            EncryptionMethod::Salsa20,
            &key,
            &iv,
        )
        .unwrap();

        // Should be a valid BLTE file
        assert_eq!(&encrypted_blte[0..4], &BLTE_MAGIC);

        // Data should start with 'E' mode byte
        assert_eq!(encrypted_blte[8], b'E');
    }

    #[test]
    fn test_encrypted_multi_chunk() {
        let data = vec![b'X'; 1000]; // 1KB of data
        let key = [0x55u8; 16];
        let iv = [0x11, 0x22, 0x33, 0x44];

        let encrypted_blte = compress_data_encrypted_multi(
            data.clone(),
            256, // 256 byte chunks
            Some(CompressionMode::ZLib),
            Some(6),
            EncryptionMethod::ARC4,
            &key,
            &iv,
        )
        .unwrap();

        // Should be a valid BLTE file
        assert_eq!(&encrypted_blte[0..4], &BLTE_MAGIC);

        // Header size should be non-zero for multi-chunk
        let header_size = u32::from_be_bytes([
            encrypted_blte[4],
            encrypted_blte[5],
            encrypted_blte[6],
            encrypted_blte[7],
        ]);
        assert!(header_size > 0);
    }

    #[test]
    fn test_encryption_integration() {
        // Test that our encryption functions work with the crypto library
        let data = b"Round-trip test data for encryption";
        let key = [0x99u8; 16];
        let iv = [0x88, 0x77, 0x66, 0x55];

        // Test Salsa20 encryption/decryption directly
        let salsa20_encrypted =
            compress_encrypted(data, EncryptionMethod::Salsa20, &key, &iv, 0).unwrap();
        assert_eq!(salsa20_encrypted[0], b'E'); // Mode byte

        // Decrypt using ngdp-crypto functions directly
        let salsa20_decrypted =
            ngdp_crypto::decrypt_salsa20(&salsa20_encrypted[1..], &key, &iv, 0).unwrap();
        assert_eq!(&salsa20_decrypted, data);

        // Test ARC4 encryption/decryption directly
        let arc4_encrypted =
            compress_encrypted(data, EncryptionMethod::ARC4, &key, &iv, 0).unwrap();
        assert_eq!(arc4_encrypted[0], b'E'); // Mode byte

        // Decrypt using ngdp-crypto functions directly
        let arc4_decrypted = ngdp_crypto::decrypt_arc4(&arc4_encrypted[1..], &key, &iv, 0).unwrap();
        assert_eq!(&arc4_decrypted, data);
    }

    #[test]
    fn test_full_round_trip_encryption() {
        use ngdp_crypto::KeyService;

        let original_data =
            b"Full round-trip test for BLTE encryption and decryption workflow".to_vec();
        let test_key = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ];
        let test_iv = [0xAA, 0xBB, 0xCC, 0xDD];
        let test_key_id = 0x1234567890ABCDEF_u64;

        // Test 1: Single-chunk Salsa20 round-trip
        println!("Testing single-chunk Salsa20 round-trip...");

        // Create encrypted BLTE
        let encrypted_blte = compress_data_encrypted_single(
            original_data.clone(),
            Some(CompressionMode::ZLib), // Apply compression first
            Some(6),
            EncryptionMethod::Salsa20,
            &test_key,
            &test_iv,
        )
        .unwrap();

        // Create KeyService with our test key
        let mut key_service = KeyService::empty();
        key_service.add_key(test_key_id, test_key);

        // Parse and manually decrypt to verify the encryption worked
        let blte_file = crate::BLTEFile::parse(encrypted_blte.clone()).unwrap();
        assert_eq!(blte_file.chunk_count(), 1);

        let chunk = blte_file.get_chunk_data(0).unwrap();
        assert_eq!(chunk.data[0], b'E'); // Should be encrypted

        // Manually decrypt the chunk to verify
        let decrypted_chunk =
            ngdp_crypto::decrypt_salsa20(&chunk.data[1..], &test_key, &test_iv, 0).unwrap();

        // The decrypted chunk should be ZLib compressed data
        let decompressed = crate::decompress::decompress_chunk(&decrypted_chunk, 0, None).unwrap();
        assert_eq!(decompressed, original_data);

        println!("✅ Single-chunk Salsa20 round-trip successful");

        // Test 2: Multi-chunk ARC4 round-trip
        println!("Testing multi-chunk ARC4 round-trip...");

        let large_data = vec![b'X'; 300]; // 300 bytes to create multiple chunks

        let encrypted_multi_blte = compress_data_encrypted_multi(
            large_data.clone(),
            100, // 100-byte chunks -> 3 chunks
            Some(CompressionMode::LZ4),
            None,
            EncryptionMethod::ARC4,
            &test_key,
            &test_iv,
        )
        .unwrap();

        // Parse and verify multi-chunk structure
        let multi_blte_file = crate::BLTEFile::parse(encrypted_multi_blte.clone()).unwrap();
        assert_eq!(multi_blte_file.chunk_count(), 3);

        // Manually decrypt each chunk and verify
        let mut reconstructed = Vec::new();
        for i in 0..3 {
            let chunk = multi_blte_file.get_chunk_data(i).unwrap();
            assert_eq!(chunk.data[0], b'E'); // Should be encrypted

            // Decrypt with correct block index
            let decrypted_chunk =
                ngdp_crypto::decrypt_arc4(&chunk.data[1..], &test_key, &test_iv, i).unwrap();

            // Decompress the LZ4 data
            let decompressed =
                crate::decompress::decompress_chunk(&decrypted_chunk, i, None).unwrap();
            reconstructed.extend_from_slice(&decompressed);
        }

        assert_eq!(reconstructed, large_data);

        println!("✅ Multi-chunk ARC4 round-trip successful");

        // Test 3: Different encryption methods produce different results
        let salsa20_encrypted = compress_data_encrypted_single(
            original_data.clone(),
            None,
            None,
            EncryptionMethod::Salsa20,
            &test_key,
            &test_iv,
        )
        .unwrap();

        let arc4_encrypted = compress_data_encrypted_single(
            original_data.clone(),
            None,
            None,
            EncryptionMethod::ARC4,
            &test_key,
            &test_iv,
        )
        .unwrap();

        // The encrypted data should be different
        assert_ne!(&salsa20_encrypted[9..], &arc4_encrypted[9..]);

        // But both should decrypt to the same original data
        let salsa20_blte = crate::BLTEFile::parse(salsa20_encrypted).unwrap();
        let arc4_blte = crate::BLTEFile::parse(arc4_encrypted).unwrap();

        let salsa20_chunk = salsa20_blte.get_chunk_data(0).unwrap();
        let arc4_chunk = arc4_blte.get_chunk_data(0).unwrap();

        let salsa20_decrypted =
            ngdp_crypto::decrypt_salsa20(&salsa20_chunk.data[1..], &test_key, &test_iv, 0).unwrap();
        let arc4_decrypted =
            ngdp_crypto::decrypt_arc4(&arc4_chunk.data[1..], &test_key, &test_iv, 0).unwrap();

        assert_eq!(salsa20_decrypted, original_data);
        assert_eq!(arc4_decrypted, original_data);

        println!("✅ Encryption method differentiation test successful");
        println!("✅ Full round-trip encryption test completed successfully!");
    }
}

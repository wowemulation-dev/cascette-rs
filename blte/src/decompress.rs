//! BLTE decompression implementation
//!
//! Handles all BLTE compression modes including encryption support.

use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::io::{Cursor, Read};
use tracing::{debug, trace};

use crate::{BLTEFile, CompressionMode, Error, Result};
use ngdp_crypto::{KeyService, arc4::decrypt_arc4, salsa20::decrypt_salsa20};

/// Decompress a complete BLTE file
pub fn decompress_blte(data: Vec<u8>, key_service: Option<&KeyService>) -> Result<Vec<u8>> {
    let blte_file = BLTEFile::parse(data)?;

    debug!(
        "Decompressing BLTE file with {} chunks",
        blte_file.chunk_count()
    );

    let mut result = Vec::new();

    for chunk_index in 0..blte_file.chunk_count() {
        let chunk = blte_file.get_chunk_data(chunk_index)?;

        // Verify checksum if present (skip for zero checksum)
        if !chunk.verify_checksum() {
            return Err(Error::ChecksumMismatch {
                expected: hex::encode(chunk.checksum),
                actual: hex::encode(md5::compute(&chunk.data).0),
            });
        }

        let decompressed = decompress_chunk(&chunk.data, chunk_index, key_service)?;
        result.extend_from_slice(&decompressed);
    }

    Ok(result)
}

/// Decompress a single chunk
pub fn decompress_chunk(
    data: &[u8],
    block_index: usize,
    key_service: Option<&KeyService>,
) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Err(Error::TruncatedData {
            expected: 1,
            actual: 0,
        });
    }

    let mode = CompressionMode::from_byte(data[0]).ok_or(Error::UnknownCompressionMode(data[0]))?;

    trace!(
        "Decompressing chunk with mode {:?} (block_index={})",
        mode, block_index
    );

    match mode {
        CompressionMode::None => decompress_none(&data[1..]),
        CompressionMode::ZLib => decompress_zlib(&data[1..]),
        CompressionMode::LZ4 => decompress_lz4(&data[1..]),
        CompressionMode::Frame => decompress_frame(&data[1..], key_service),
        CompressionMode::Encrypted => {
            let key_service = key_service.ok_or_else(|| {
                Error::DecompressionFailed("Key service required for encrypted blocks".to_string())
            })?;
            decompress_encrypted(&data[1..], block_index, key_service)
        }
    }
}

/// Mode 'N' - No compression
fn decompress_none(data: &[u8]) -> Result<Vec<u8>> {
    trace!("No compression - returning {} bytes as-is", data.len());
    Ok(data.to_vec())
}

/// Mode 'Z' - ZLib compression
fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>> {
    trace!("ZLib decompression of {} bytes", data.len());

    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();

    decoder
        .read_to_end(&mut result)
        .map_err(|e| Error::DecompressionFailed(format!("ZLib decompression failed: {e}")))?;

    debug!("ZLib: {} bytes -> {} bytes", data.len(), result.len());
    Ok(result)
}

/// Mode '4' - LZ4 compression  
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    trace!("LZ4 decompression of {} bytes", data.len());

    if data.len() < 8 {
        return Err(Error::TruncatedData {
            expected: 8,
            actual: data.len(),
        });
    }

    let mut cursor = Cursor::new(data);
    let decompressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;

    if compressed_size + 8 != data.len() {
        return Err(Error::DecompressionFailed(format!(
            "LZ4 size mismatch: expected {} bytes, got {}",
            compressed_size + 8,
            data.len()
        )));
    }

    let lz4_data = &data[8..];
    let result = lz4_flex::decompress(lz4_data, decompressed_size)
        .map_err(|e| Error::DecompressionFailed(format!("LZ4 decompression failed: {e}")))?;

    debug!("LZ4: {} bytes -> {} bytes", data.len(), result.len());
    Ok(result)
}

/// Mode 'F' - Frame/Recursive BLTE
fn decompress_frame(data: &[u8], key_service: Option<&KeyService>) -> Result<Vec<u8>> {
    trace!("Frame/recursive decompression of {} bytes", data.len());

    // The data contains another complete BLTE structure
    decompress_blte(data.to_vec(), key_service)
}

/// Mode 'E' - Encrypted
fn decompress_encrypted(
    data: &[u8],
    block_index: usize,
    key_service: &KeyService,
) -> Result<Vec<u8>> {
    trace!(
        "Encrypted decompression of {} bytes (block_index={})",
        data.len(),
        block_index
    );

    if data.len() < 17 {
        return Err(Error::InvalidEncryptedBlock(format!(
            "Encrypted block too short: {} bytes (minimum 17)",
            data.len()
        )));
    }

    let mut cursor = Cursor::new(data);

    // Read key name size (should be 8)
    let key_name_size = cursor.read_u64::<LittleEndian>()?;
    if key_name_size != 8 {
        return Err(Error::InvalidEncryptedBlock(format!(
            "Invalid key name size: {key_name_size} (expected 8)"
        )));
    }

    // Read key name (8 bytes, little-endian)
    let key_name = cursor.read_u64::<LittleEndian>()?;

    // Look up the key
    let key = key_service
        .get_key(key_name)
        .ok_or(Error::KeyNotFound(key_name))?;

    // Read IV size (should be 4)
    let iv_size = cursor.read_u32::<LittleEndian>()?;
    if iv_size != 4 {
        return Err(Error::InvalidEncryptedBlock(format!(
            "Invalid IV size: {iv_size} (expected 4)"
        )));
    }

    // Read IV (4 bytes)
    let mut iv = [0u8; 4];
    cursor.read_exact(&mut iv)?;

    // Read encryption type
    let enc_type = cursor.read_u8()?;

    // Get the encrypted data
    let encrypted_data = &data[cursor.position() as usize..];

    debug!(
        "Decrypting block: key_name={:#018x}, enc_type={:#04x}, block_index={}",
        key_name, enc_type, block_index
    );

    // Decrypt based on encryption type
    let decrypted = match enc_type {
        0x53 => {
            // Salsa20
            decrypt_salsa20(encrypted_data, key, &iv, block_index)?
        }
        0x41 => {
            // ARC4
            decrypt_arc4(encrypted_data, key, &iv, block_index)?
        }
        _ => {
            return Err(Error::UnsupportedEncryptionType(enc_type));
        }
    };

    debug!(
        "Decrypted {} bytes -> {} bytes",
        encrypted_data.len(),
        decrypted.len()
    );

    // Recursively decompress the decrypted data if it's compressed
    if !decrypted.is_empty() {
        let decrypted_mode = CompressionMode::from_byte(decrypted[0]);
        if decrypted_mode.is_some() && decrypted_mode != Some(CompressionMode::Encrypted) {
            trace!("Recursively decompressing decrypted data");
            return decompress_chunk(&decrypted, block_index, Some(key_service));
        }
    }

    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompress_none() {
        let test_data = b"Hello, BLTE!";
        let result = decompress_none(test_data).unwrap();
        assert_eq!(result, test_data);
    }

    #[test]
    fn test_decompress_zlib() {
        // Create some zlib-compressed data
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        let original = b"Hello, BLTE! This is a longer string to get better compression.";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decompress_zlib(&compressed).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_decompress_lz4() {
        let original = b"Hello, BLTE! This is some test data for LZ4 compression testing.";
        let compressed = lz4_flex::compress(original);

        // Create LZ4 BLTE format: decompressed_size + compressed_size + data
        let mut lz4_data = Vec::new();
        lz4_data.extend_from_slice(&(original.len() as u32).to_le_bytes());
        lz4_data.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        lz4_data.extend_from_slice(&compressed);

        let result = decompress_lz4(&lz4_data).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_compression_mode_from_byte() {
        assert_eq!(
            CompressionMode::from_byte(b'N'),
            Some(CompressionMode::None)
        );
        assert_eq!(
            CompressionMode::from_byte(b'Z'),
            Some(CompressionMode::ZLib)
        );
        assert_eq!(CompressionMode::from_byte(b'4'), Some(CompressionMode::LZ4));
        assert_eq!(
            CompressionMode::from_byte(b'F'),
            Some(CompressionMode::Frame)
        );
        assert_eq!(
            CompressionMode::from_byte(b'E'),
            Some(CompressionMode::Encrypted)
        );
        assert_eq!(CompressionMode::from_byte(b'X'), None);
    }

    #[test]
    fn test_single_chunk_decompression() {
        let mut blte_data = Vec::new();
        blte_data.extend_from_slice(b"BLTE");
        blte_data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
        blte_data.push(b'N'); // No compression
        blte_data.extend_from_slice(b"Hello, BLTE!");

        let result = decompress_blte(blte_data, None).unwrap();
        assert_eq!(result, b"Hello, BLTE!");
    }

    #[test]
    fn test_multi_chunk_decompression() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        // Create two compressed chunks
        let chunk1_data = b"Hello, ";
        let chunk2_data = b"BLTE!";

        let mut encoder1 = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder1.write_all(chunk1_data).unwrap();
        let compressed1 = encoder1.finish().unwrap();

        let mut encoder2 = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder2.write_all(chunk2_data).unwrap();
        let compressed2 = encoder2.finish().unwrap();

        // Build chunk data with compression mode prefixes
        let mut chunk1_full = Vec::new();
        chunk1_full.push(b'Z'); // ZLib compression mode
        chunk1_full.extend_from_slice(&compressed1);

        let mut chunk2_full = Vec::new();
        chunk2_full.push(b'Z'); // ZLib compression mode
        chunk2_full.extend_from_slice(&compressed2);

        // Calculate header size
        let header_size = 8 + 1 + 3 + 2 * 24; // magic + header_size + flags + chunk_count + 2 * chunk_info

        // Build BLTE file
        let mut blte_data = Vec::new();

        // Header
        blte_data.extend_from_slice(b"BLTE");
        blte_data.extend_from_slice(&(header_size as u32).to_be_bytes());

        // Chunk table
        blte_data.push(0x0F); // Flags
        blte_data.extend_from_slice(&[0x00, 0x00, 0x02]); // 2 chunks

        // Chunk 1 info
        blte_data.extend_from_slice(&(chunk1_full.len() as u32).to_be_bytes());
        blte_data.extend_from_slice(&(chunk1_data.len() as u32).to_be_bytes());
        blte_data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

        // Chunk 2 info
        blte_data.extend_from_slice(&(chunk2_full.len() as u32).to_be_bytes());
        blte_data.extend_from_slice(&(chunk2_data.len() as u32).to_be_bytes());
        blte_data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

        // Chunk data
        blte_data.extend_from_slice(&chunk1_full);
        blte_data.extend_from_slice(&chunk2_full);

        let result = decompress_blte(blte_data, None).unwrap();
        assert_eq!(result, b"Hello, BLTE!");
    }
}

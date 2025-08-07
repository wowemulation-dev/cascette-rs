//! BLTE streaming decompression implementation
//!
//! Provides streaming decompression for BLTE files, allowing processing
//! of large files without loading everything into memory.

use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::io::{Cursor, Read, Result as IoResult};
use tracing::{debug, trace, warn};

use crate::{BLTEFile, CompressionMode, Error, Result};
use ngdp_crypto::{KeyService, arc4::decrypt_arc4, salsa20::decrypt_salsa20};

/// A streaming BLTE decompressor
///
/// This allows decompressing BLTE files chunk by chunk without loading
/// the entire file into memory. Useful for large game assets.
pub struct BLTEStream {
    /// The underlying BLTEFile structure
    blte_file: BLTEFile,
    /// Current chunk being processed
    current_chunk: usize,
    /// Key service for encrypted chunks
    key_service: Option<KeyService>,
    /// Internal buffer for chunk data
    chunk_buffer: Vec<u8>,
    /// Position within current chunk data
    chunk_position: usize,
}

impl BLTEStream {
    /// Create a new streaming BLTE decompressor
    ///
    /// # Arguments
    /// * `data` - The raw BLTE data to decompress
    /// * `key_service` - Optional key service for encrypted chunks
    ///
    /// # Errors
    /// Returns an error if the BLTE file cannot be parsed.
    pub fn new(data: Vec<u8>, key_service: Option<KeyService>) -> Result<Self> {
        let blte_file = BLTEFile::parse(data)?;
        
        debug!(
            "Created BLTE stream with {} chunks",
            blte_file.chunk_count()
        );

        Ok(Self {
            blte_file,
            current_chunk: 0,
            key_service,
            chunk_buffer: Vec::new(),
            chunk_position: 0,
        })
    }

    /// Get the total number of chunks
    pub fn chunk_count(&self) -> usize {
        self.blte_file.chunk_count()
    }

    /// Get the current chunk index being processed
    pub fn current_chunk_index(&self) -> usize {
        self.current_chunk
    }

    /// Check if we have more chunks to process
    pub fn has_more_chunks(&self) -> bool {
        self.current_chunk < self.blte_file.chunk_count()
    }

    /// Prepare the next chunk for reading
    fn prepare_next_chunk(&mut self) -> Result<()> {
        if !self.has_more_chunks() {
            return Ok(()); // No more chunks
        }

        let chunk = self.blte_file.get_chunk_data(self.current_chunk)?;
        
        // Verify checksum if present (skip for zero checksum)
        if !chunk.verify_checksum() {
            return Err(Error::ChecksumMismatch {
                expected: hex::encode(chunk.checksum),
                actual: hex::encode(md5::compute(&chunk.data).0),
            });
        }

        // Decompress the chunk data
        let decompressed = decompress_chunk_streaming(&chunk.data, self.current_chunk, self.key_service.as_ref())?;
        
        self.chunk_buffer = decompressed;
        self.chunk_position = 0;
        self.current_chunk += 1;

        trace!(
            "Prepared chunk {} with {} bytes",
            self.current_chunk - 1,
            self.chunk_buffer.len()
        );

        Ok(())
    }
}

impl Read for BLTEStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let mut bytes_read = 0;

        while bytes_read < buf.len() {
            // If we've consumed all data from the current chunk, prepare the next one
            if self.chunk_position >= self.chunk_buffer.len() {
                if !self.has_more_chunks() {
                    break; // No more data
                }
                
                if let Err(e) = self.prepare_next_chunk() {
                    warn!("Failed to prepare next chunk: {}", e);
                    break;
                }
                
                if self.chunk_buffer.is_empty() {
                    continue; // This chunk was empty, try the next one
                }
            }

            // Copy data from current chunk buffer
            let available = self.chunk_buffer.len() - self.chunk_position;
            let to_copy = std::cmp::min(available, buf.len() - bytes_read);
            
            if to_copy == 0 {
                break;
            }

            buf[bytes_read..bytes_read + to_copy]
                .copy_from_slice(&self.chunk_buffer[self.chunk_position..self.chunk_position + to_copy]);
            
            self.chunk_position += to_copy;
            bytes_read += to_copy;
        }

        Ok(bytes_read)
    }
}

/// Create a streaming reader from BLTE data
///
/// This is a convenience function that creates a BLTEStream for immediate use.
pub fn create_streaming_reader(data: Vec<u8>, key_service: Option<KeyService>) -> Result<BLTEStream> {
    BLTEStream::new(data, key_service)
}

/// Decompress a single chunk for streaming (internal function)
fn decompress_chunk_streaming(
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
        "Decompressing streaming chunk with mode {:?} (block_index={})",
        mode, block_index
    );

    match mode {
        CompressionMode::None => decompress_none_streaming(&data[1..]),
        CompressionMode::ZLib => decompress_zlib_streaming(&data[1..]),
        CompressionMode::LZ4 => decompress_lz4_streaming(&data[1..]),
        CompressionMode::Frame => decompress_frame_streaming(&data[1..], key_service),
        CompressionMode::Encrypted => {
            let key_service = key_service.ok_or_else(|| {
                Error::DecompressionFailed("Key service required for encrypted blocks".to_string())
            })?;
            decompress_encrypted_streaming(&data[1..], block_index, key_service)
        }
    }
}

/// Mode 'N' - No compression (streaming)
fn decompress_none_streaming(data: &[u8]) -> Result<Vec<u8>> {
    trace!("No compression (streaming) - returning {} bytes as-is", data.len());
    Ok(data.to_vec())
}

/// Mode 'Z' - ZLib compression (streaming)
fn decompress_zlib_streaming(data: &[u8]) -> Result<Vec<u8>> {
    trace!("ZLib decompression (streaming) of {} bytes", data.len());

    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();

    decoder
        .read_to_end(&mut result)
        .map_err(|e| Error::DecompressionFailed(format!("ZLib decompression failed: {e}")))?;

    debug!("ZLib (streaming): {} bytes -> {} bytes", data.len(), result.len());
    Ok(result)
}

/// Mode '4' - LZ4 compression (streaming)
fn decompress_lz4_streaming(data: &[u8]) -> Result<Vec<u8>> {
    trace!("LZ4 decompression (streaming) of {} bytes", data.len());

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

    debug!("LZ4 (streaming): {} bytes -> {} bytes", data.len(), result.len());
    Ok(result)
}

/// Mode 'F' - Frame/Recursive BLTE (streaming)
fn decompress_frame_streaming(data: &[u8], key_service: Option<&KeyService>) -> Result<Vec<u8>> {
    trace!("Frame/recursive decompression (streaming) of {} bytes", data.len());

    // For now, fall back to the regular decompression for nested BLTE frames
    // This avoids the KeyService cloning issue but still works correctly
    crate::decompress::decompress_blte(data.to_vec(), key_service)
}

/// Mode 'E' - Encrypted (streaming)
fn decompress_encrypted_streaming(
    data: &[u8],
    block_index: usize,
    key_service: &KeyService,
) -> Result<Vec<u8>> {
    trace!(
        "Encrypted decompression (streaming) of {} bytes (block_index={})",
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
        "Decrypting block (streaming): key_name={:#018x}, enc_type={:#04x}, block_index={}",
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
        "Decrypted (streaming) {} bytes -> {} bytes",
        encrypted_data.len(),
        decrypted.len()
    );

    // Recursively decompress the decrypted data if it's compressed
    if !decrypted.is_empty() {
        let decrypted_mode = CompressionMode::from_byte(decrypted[0]);
        if decrypted_mode.is_some() && decrypted_mode != Some(CompressionMode::Encrypted) {
            trace!("Recursively decompressing decrypted data (streaming)");
            return decompress_chunk_streaming(&decrypted, block_index, Some(key_service));
        }
    }

    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_single_chunk() {
        let mut blte_data = Vec::new();
        blte_data.extend_from_slice(b"BLTE");
        blte_data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
        blte_data.push(b'N'); // No compression
        blte_data.extend_from_slice(b"Hello, BLTE Streaming!");

        let mut stream = BLTEStream::new(blte_data, None).unwrap();
        let mut result = String::new();
        stream.read_to_string(&mut result).unwrap();

        assert_eq!(result, "Hello, BLTE Streaming!");
    }

    #[test]
    fn test_streaming_multi_chunk() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        // Create two compressed chunks
        let chunk1_data = b"Hello, ";
        let chunk2_data = b"BLTE Streaming!";

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

        let mut stream = BLTEStream::new(blte_data, None).unwrap();
        let mut result = String::new();
        stream.read_to_string(&mut result).unwrap();

        assert_eq!(result, "Hello, BLTE Streaming!");
    }

    #[test]
    fn test_streaming_chunk_by_chunk_read() {
        let mut blte_data = Vec::new();
        blte_data.extend_from_slice(b"BLTE");
        blte_data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
        blte_data.push(b'N'); // No compression
        blte_data.extend_from_slice(b"Hello, BLTE!");

        let mut stream = BLTEStream::new(blte_data, None).unwrap();
        
        // Read in small chunks
        let mut result = Vec::new();
        let mut buffer = [0u8; 4];
        
        loop {
            let bytes_read = stream.read(&mut buffer).unwrap();
            if bytes_read == 0 {
                break;
            }
            result.extend_from_slice(&buffer[..bytes_read]);
        }

        assert_eq!(String::from_utf8(result).unwrap(), "Hello, BLTE!");
    }

    #[test]
    fn test_create_streaming_reader() {
        let mut blte_data = Vec::new();
        blte_data.extend_from_slice(b"BLTE");
        blte_data.extend_from_slice(&0u32.to_be_bytes()); // Single chunk
        blte_data.push(b'N'); // No compression
        blte_data.extend_from_slice(b"Hello, Reader!");

        let mut reader = create_streaming_reader(blte_data, None).unwrap();
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();

        assert_eq!(result, "Hello, Reader!");
    }
}
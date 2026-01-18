//! BLTE file builder

use super::compression::{EncryptionSpec, encrypt_chunk_with_key};
use super::error::{BlteError, BlteResult};
use super::{BlteFile, BlteHeader, ChunkData, CompressionMode};

/// Minimum chunk size (1 KB) - smaller chunks create too much overhead
const MIN_CHUNK_SIZE: usize = 1024;

/// Maximum chunk size (16 MB) - typical CDN chunk limit for BLTE
const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// Default chunk size (256 KB) - balanced for performance
const DEFAULT_CHUNK_SIZE: usize = 256 * 1024;

/// Encryption configuration for BLTE builder
#[derive(Debug, Clone, Copy)]
pub struct EncryptionConfig {
    /// Encryption specification (key name, IV, type)
    pub spec: EncryptionSpec,
    /// The 128-bit encryption key
    pub key: [u8; 16],
}

/// Builder for creating BLTE files
pub struct BlteBuilder {
    chunks: Vec<ChunkData>,
    default_mode: CompressionMode,
    chunk_size: usize,
    encryption: Option<EncryptionConfig>,
}

impl BlteBuilder {
    /// Create a new BLTE builder
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            default_mode: CompressionMode::None,
            chunk_size: DEFAULT_CHUNK_SIZE,
            encryption: None,
        }
    }

    /// Set the default compression mode
    #[must_use]
    pub fn with_compression(mut self, mode: CompressionMode) -> Self {
        self.default_mode = mode;
        self
    }

    /// Set the chunk size for automatic chunking with validation
    ///
    /// # Arguments
    /// * `size` - Chunk size in bytes (must be between 1 KB and 16 MB)
    ///
    /// # Returns
    /// * `Ok(Self)` if the chunk size is valid
    /// * `Err(BlteError)` if the chunk size is invalid
    pub fn with_chunk_size(mut self, size: usize) -> BlteResult<Self> {
        if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&size) {
            return Err(BlteError::InvalidChunkSize {
                size,
                min: MIN_CHUNK_SIZE,
                max: MAX_CHUNK_SIZE,
            });
        }
        self.chunk_size = size;
        Ok(self)
    }

    /// Set the chunk size without validation (for testing purposes)
    #[must_use]
    pub fn with_chunk_size_unchecked(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set encryption parameters for the builder
    /// All chunks added after this call will be encrypted using these parameters
    #[must_use]
    pub fn with_encryption(mut self, spec: EncryptionSpec, key: [u8; 16]) -> Self {
        self.encryption = Some(EncryptionConfig { spec, key });
        self
    }

    /// Remove encryption from the builder
    /// Chunks added after this call will not be encrypted
    #[must_use]
    pub fn without_encryption(mut self) -> Self {
        self.encryption = None;
        self
    }

    /// Add a pre-built chunk
    #[must_use]
    pub fn add_chunk(mut self, chunk: ChunkData) -> Self {
        self.chunks.push(chunk);
        self
    }

    /// Add data that will be automatically chunked
    pub fn add_data(mut self, data: &[u8]) -> BlteResult<Self> {
        if data.len() <= self.chunk_size {
            // Single chunk
            let chunk = if let Some(_encryption) = &self.encryption {
                self.create_encrypted_chunk(data.to_vec(), 0)?
            } else {
                ChunkData::new(data.to_vec(), self.default_mode)?
            };
            self.chunks.push(chunk);
        } else {
            // Multiple chunks
            let mut offset = 0;
            let mut chunk_index = 0;
            while offset < data.len() {
                let end = (offset + self.chunk_size).min(data.len());
                let chunk_data = data[offset..end].to_vec();
                let chunk = if let Some(_encryption) = &self.encryption {
                    self.create_encrypted_chunk(chunk_data, chunk_index)?
                } else {
                    ChunkData::new(chunk_data, self.default_mode)?
                };
                self.chunks.push(chunk);
                offset = end;
                chunk_index += 1;
            }
        }
        Ok(self)
    }

    /// Add data as a single encrypted chunk (regardless of size)
    /// This is useful when you want to encrypt large data as one chunk
    pub fn add_encrypted_data(
        mut self,
        data: &[u8],
        spec: EncryptionSpec,
        key: [u8; 16],
        block_index: usize,
    ) -> BlteResult<Self> {
        let chunk =
            self.create_encrypted_chunk_with_params(data.to_vec(), spec, key, block_index)?;
        self.chunks.push(chunk);
        Ok(self)
    }

    /// Add multiple chunks with individual encryption parameters
    /// Useful for mixing encrypted and non-encrypted chunks
    pub fn add_mixed_data(
        mut self,
        data: &[u8],
        encryption_per_chunk: Option<(EncryptionSpec, [u8; 16])>,
    ) -> BlteResult<Self> {
        if data.len() <= self.chunk_size {
            // Single chunk - use current chunk count as block index for encryption
            let chunk_index = self.chunks.len();
            let chunk = if let Some((spec, key)) = encryption_per_chunk {
                self.create_encrypted_chunk_with_params(data.to_vec(), spec, key, chunk_index)?
            } else {
                ChunkData::new(data.to_vec(), self.default_mode)?
            };
            self.chunks.push(chunk);
        } else {
            // Multiple chunks
            let mut offset = 0;
            let mut chunk_index = self.chunks.len();
            while offset < data.len() {
                let end = (offset + self.chunk_size).min(data.len());
                let chunk_data = data[offset..end].to_vec();
                let chunk = if let Some((spec, key)) = encryption_per_chunk {
                    self.create_encrypted_chunk_with_params(chunk_data, spec, key, chunk_index)?
                } else {
                    ChunkData::new(chunk_data, self.default_mode)?
                };
                self.chunks.push(chunk);
                offset = end;
                chunk_index += 1;
            }
        }
        Ok(self)
    }

    /// Create an encrypted chunk using the builder's current encryption config
    fn create_encrypted_chunk(
        &self,
        mut data: Vec<u8>,
        block_index: usize,
    ) -> BlteResult<ChunkData> {
        let encryption = self.encryption.as_ref().ok_or_else(|| {
            super::error::BlteError::CompressionError("No encryption config set".to_string())
        })?;

        // Apply compression first if needed
        if self.default_mode != CompressionMode::None
            && self.default_mode != CompressionMode::Encrypted
        {
            let compressed = super::compression::compress_chunk(&data, self.default_mode)?;
            data = vec![self.default_mode.as_byte()];
            data.extend_from_slice(&compressed);
        }

        // Then encrypt
        let encrypted_data =
            encrypt_chunk_with_key(&data, encryption.spec, &encryption.key, block_index)?;

        Ok(ChunkData::from_compressed(
            CompressionMode::Encrypted,
            encrypted_data,
            Some(data.len()),
        ))
    }

    /// Create an encrypted chunk with specific parameters
    fn create_encrypted_chunk_with_params(
        &self,
        mut data: Vec<u8>,
        spec: EncryptionSpec,
        key: [u8; 16],
        block_index: usize,
    ) -> BlteResult<ChunkData> {
        // Apply compression first if needed
        if self.default_mode != CompressionMode::None
            && self.default_mode != CompressionMode::Encrypted
        {
            let compressed = super::compression::compress_chunk(&data, self.default_mode)?;
            data = vec![self.default_mode.as_byte()];
            data.extend_from_slice(&compressed);
        }

        // Then encrypt
        let encrypted_data = encrypt_chunk_with_key(&data, spec, &key, block_index)?;

        Ok(ChunkData::from_compressed(
            CompressionMode::Encrypted,
            encrypted_data,
            Some(data.len()),
        ))
    }

    /// Build the BLTE file
    pub fn build(self) -> BlteResult<BlteFile> {
        if self.chunks.is_empty() {
            return Err(super::error::BlteError::InvalidChunkCount(0));
        }

        if self.chunks.len() == 1 {
            // Single chunk file
            Ok(BlteFile {
                header: BlteHeader::single_chunk(),
                chunks: self.chunks,
            })
        } else {
            // Multi-chunk file
            let header = BlteHeader::multi_chunk(&self.chunks)?;
            Ok(BlteFile {
                header,
                chunks: self.chunks,
            })
        }
    }
}

impl Default for BlteBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::blte::compression::decrypt_chunk_with_keys;
    use cascette_crypto::{TactKey, TactKeyStore};

    #[test]
    fn test_builder_single_chunk() {
        let data = b"Hello, BLTE!";
        let blte = BlteBuilder::new()
            .add_data(data)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        assert!(blte.header.is_single_chunk());
        assert_eq!(blte.chunks.len(), 1);
        assert_eq!(blte.decompress().expect("Operation should succeed"), data);
    }

    #[test]
    fn test_builder_chunk_size_validation() {
        // Test too small chunk size
        let result = BlteBuilder::new().with_chunk_size(512); // Less than 1KB
        assert!(result.is_err());
        if let Err(BlteError::InvalidChunkSize { size, min, max }) = result {
            assert_eq!(size, 512);
            assert_eq!(min, MIN_CHUNK_SIZE);
            assert_eq!(max, MAX_CHUNK_SIZE);
        } else {
            panic!("Expected InvalidChunkSize error");
        }

        // Test too large chunk size
        let result = BlteBuilder::new().with_chunk_size(20 * 1024 * 1024); // 20MB
        assert!(result.is_err());

        // Test valid chunk sizes
        assert!(BlteBuilder::new().with_chunk_size(1024).is_ok()); // 1KB - minimum
        assert!(BlteBuilder::new().with_chunk_size(256 * 1024).is_ok()); // 256KB - default
        assert!(BlteBuilder::new().with_chunk_size(1024 * 1024).is_ok()); // 1MB
        assert!(BlteBuilder::new().with_chunk_size(16 * 1024 * 1024).is_ok()); // 16MB - maximum

        // Test unchecked method allows any size
        let builder = BlteBuilder::new().with_chunk_size_unchecked(100); // Very small
        assert_eq!(builder.chunk_size, 100);
    }

    #[test]
    fn test_builder_multi_chunk() {
        let blte = BlteBuilder::new()
            .with_chunk_size_unchecked(5) // Using unchecked for test with tiny chunks
            .add_data(b"Hello, BLTE!")
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        assert!(!blte.header.is_single_chunk());
        assert_eq!(blte.chunks.len(), 3); // "Hello", ", BLT", "E!"
        assert_eq!(
            blte.decompress().expect("Operation should succeed"),
            b"Hello, BLTE!"
        );
    }

    #[test]
    fn test_builder_encryption_single_chunk() {
        let data = b"Hello, encrypted BLTE!";
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ];

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let blte = BlteBuilder::new()
            .with_encryption(spec, key)
            .add_data(data)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Should be single chunk with encryption mode
        assert!(blte.header.is_single_chunk());
        assert_eq!(blte.chunks.len(), 1);
        assert_eq!(blte.chunks[0].mode, CompressionMode::Encrypted);

        // Create key store for decryption
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Decrypt and verify
        let decrypted = blte
            .decompress_with_keys(&key_store)
            .expect("Test operation should succeed");
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_builder_encryption_multi_chunk() {
        let data = b"This is longer data that will be split into multiple encrypted chunks";
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ];

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let blte = BlteBuilder::new()
            .with_encryption(spec, key)
            .with_chunk_size_unchecked(20) // Force multiple chunks for testing
            .add_data(data)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Should be multi-chunk with all encrypted
        assert!(!blte.header.is_single_chunk());
        assert!(blte.chunks.len() > 1);

        // All chunks should be encrypted
        for chunk in &blte.chunks {
            assert_eq!(chunk.mode, CompressionMode::Encrypted);
        }

        // Create key store for decryption
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Decrypt and verify
        let decrypted = blte
            .decompress_with_keys(&key_store)
            .expect("Test operation should succeed");
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_builder_add_encrypted_data() {
        let data = b"Data encrypted with specific parameters";
        let key_name = 0xFEDC_BA09_8765_4321;
        let iv = [0xAA, 0xBB, 0xCC, 0xDD];
        let key = [
            0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC,
            0xDE, 0xF0,
        ];
        #[allow(clippy::no_effect_underscore_binding)]
        let _block_index = 42;

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let blte = BlteBuilder::new()
            .add_encrypted_data(data, spec, key, 0) // Use block index 0 for single chunk
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Should be single encrypted chunk
        assert!(blte.header.is_single_chunk());
        assert_eq!(blte.chunks[0].mode, CompressionMode::Encrypted);

        // Create key store for decryption
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Decrypt and verify
        let decrypted = blte
            .decompress_with_keys(&key_store)
            .expect("Test operation should succeed");
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_builder_mixed_encryption() {
        let data1 = b"Plain text chunk";
        let data2 = b"Encrypted chunk";
        let key_name = 0x1111_2222_3333_4444;
        let iv = [0x01, 0x02, 0x03, 0x04];
        let key = [0xAB; 16];

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let blte = BlteBuilder::new()
            // Add plain chunk
            .add_mixed_data(data1, None)
            .expect("Operation should succeed")
            // Add encrypted chunk (will use chunk index 1)
            .add_mixed_data(data2, Some((spec, key)))
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Should be multi-chunk with mixed modes
        assert!(!blte.header.is_single_chunk());
        assert_eq!(blte.chunks.len(), 2);
        assert_eq!(blte.chunks[0].mode, CompressionMode::None);
        assert_eq!(blte.chunks[1].mode, CompressionMode::Encrypted);

        // Create key store for decryption
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Decrypt and verify
        let decrypted = blte
            .decompress_with_keys(&key_store)
            .expect("Test operation should succeed");
        let mut expected = Vec::new();
        expected.extend_from_slice(data1);
        expected.extend_from_slice(data2);
        assert_eq!(decrypted, expected);
    }

    #[test]
    fn test_builder_encryption_with_compression() {
        let data = b"This data will be compressed then encrypted using ZLib compression mode";
        let key_name = 0x5555_6666_7777_8888;
        let iv = [0xEE, 0xFF, 0x00, 0x11];
        let key = [0x33; 16];

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let blte = BlteBuilder::new()
            .with_compression(CompressionMode::ZLib)
            .with_encryption(spec, key)
            .add_data(data)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Should be encrypted (compression happens inside encryption)
        assert!(blte.header.is_single_chunk());
        assert_eq!(blte.chunks[0].mode, CompressionMode::Encrypted);

        // Create key store for decryption
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Decrypt and verify (should auto-decompress)
        let decrypted = blte
            .decompress_with_keys(&key_store)
            .expect("Test operation should succeed");
        assert_eq!(decrypted, data);
    }

    #[test]
    #[allow(clippy::panic)]
    fn test_builder_encryption_round_trip() {
        let byte_sequence = (0..=255).collect::<Vec<u8>>();
        let test_cases = vec![
            b"Short encrypted data".as_slice(),
            b"This is a longer test case for encryption round-trip testing with the BLTE builder"
                .as_slice(),
            &[0u8; 256],    // Zeros
            &byte_sequence, // Byte sequence
        ];

        let key_name = 0xDEAD_BEEF_CAFE_BABE;
        let iv = [0x12, 0x34, 0x56, 0x78];
        let key = [0x42; 16];

        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        let spec = EncryptionSpec::salsa20(key_name, iv);

        for (i, data) in test_cases.into_iter().enumerate() {
            // Build encrypted BLTE
            let blte = BlteBuilder::new()
                .with_encryption(spec, key)
                .add_data(data)
                .expect("BLTE encrypted data addition should succeed in test")
                .build()
                .expect("BLTE build should succeed in test");

            // Verify structure
            assert_eq!(blte.chunks[0].mode, CompressionMode::Encrypted);

            // Decrypt and verify
            let decrypted = blte
                .decompress_with_keys(&key_store)
                .expect("Failed to decrypt BLTE for test case");

            assert_eq!(
                decrypted, data,
                "Encryption round-trip failed for test case {i}"
            );
        }
    }

    #[test]
    fn test_builder_without_encryption() {
        let data = b"This will be encrypted then switched to plain";
        let key_name = 0x9999_AAAA_BBBB_CCCC;
        let iv = [0x99, 0xAA, 0xBB, 0xCC];
        let key = [0x88; 16];

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let blte = BlteBuilder::new()
            .with_encryption(spec, key)
            .without_encryption() // Disable encryption
            .add_data(data)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Should be unencrypted
        assert_eq!(blte.chunks[0].mode, CompressionMode::None);

        // Decrypt without keys (should work)
        let decrypted = blte.decompress().expect("Test operation should succeed");
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encryption_different_block_indices() {
        let data = b"Test data for different block indices";
        let key_name = 0x1111_2222_3333_4444;
        let iv = [0x01, 0x02, 0x03, 0x04];
        let key = [0xAB; 16];

        let spec = EncryptionSpec::salsa20(key_name, iv);

        // Build with different block indices
        let blte1 = BlteBuilder::new()
            .add_encrypted_data(data, spec, key, 0)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        let blte2 = BlteBuilder::new()
            .add_encrypted_data(data, spec, key, 1)
            .expect("Operation should succeed")
            .build()
            .expect("Test operation should succeed");

        // Encrypted data should be different
        assert_ne!(blte1.chunks[0].data, blte2.chunks[0].data);

        // Create key stores for decryption
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // For manual decryption with correct block indices
        let decrypted1 = decrypt_chunk_with_keys(&blte1.chunks[0].data, &key_store, 0)
            .expect("Test operation should succeed");
        let decrypted2 = decrypt_chunk_with_keys(&blte2.chunks[0].data, &key_store, 1)
            .expect("Test operation should succeed");

        // Both should decrypt to the same plaintext when using correct indices
        assert_eq!(decrypted1, data);
        assert_eq!(decrypted2, data);
        assert_eq!(decrypted1, decrypted2);
    }
}

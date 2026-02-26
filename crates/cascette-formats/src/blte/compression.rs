//! BLTE compression and decompression

use super::chunk::CompressionMode;
use super::error::{BlteError, BlteResult};
use cascette_crypto::TactKeyStore;
use cascette_crypto::salsa20::{decrypt_salsa20, encrypt_salsa20};
use flate2::Compression;
use flate2::read::{ZlibDecoder, ZlibEncoder};
use std::io::Read;

/// Maximum allowed decompression size (1 GB)
///
/// Security: Limits decompression output to prevent denial of service via
/// compression bombs. WoW's largest individual files are typically under
/// 100 MB, so 1 GB provides ample headroom while preventing abuse.
pub const MAX_DECOMPRESSION_SIZE: usize = 1024 * 1024 * 1024;

/// Compress data using specified mode
pub fn compress_chunk(data: &[u8], mode: CompressionMode) -> BlteResult<Vec<u8>> {
    match mode {
        CompressionMode::None => Ok(data.to_vec()),
        CompressionMode::ZLib => {
            let mut encoder = ZlibEncoder::new(data, Compression::default());
            let mut compressed = Vec::new();
            encoder.read_to_end(&mut compressed).map_err(|e| {
                BlteError::CompressionError(format!("ZLib compression failed: {e}"))
            })?;
            Ok(compressed)
        }
        CompressionMode::LZ4 => {
            // LZ4 compression: 8-byte LE decompressed size prefix + single LZ4 block.
            //
            // The WoWDev wiki describes a different format with headerVersion,
            // BE size, and blockShift for sub-blocks. However, Agent.exe 3.13.3
            // (`tact::Codec::DecodeLZ4` at 0x6f5fdb) is a stub that returns
            // error 5 — LZ4 decompression is not implemented in that binary.
            // This implementation uses the 8-byte LE prefix format observed in
            // real WoW BLTE data.
            let decompressed_size = data.len() as u64;

            // Pre-allocate with worst-case size (LZ4 worst case is ~1.06x original size)
            // We need 8 bytes for the size header plus the compressed data
            let max_compressed_size = lz4_flex::block::get_maximum_output_size(data.len());
            let mut result = vec![0u8; 8 + max_compressed_size];

            result[0..8].copy_from_slice(&decompressed_size.to_le_bytes());

            let compressed_len = lz4_flex::block::compress_into(data, &mut result[8..])
                .map_err(|e| BlteError::CompressionError(format!("LZ4 compression failed: {e}")))?;

            // Truncate to actual size
            result.truncate(8 + compressed_len);
            Ok(result)
        }
        CompressionMode::Encrypted => {
            // Encryption mode requires special handling via encrypt_chunk_with_key
            Err(BlteError::CompressionError(
                "Use encrypt_chunk_with_key for encryption mode".to_string(),
            ))
        }
        #[allow(deprecated)]
        CompressionMode::Frame => Err(super::error::BlteError::UnsupportedCompressionMode(
            mode.as_byte(),
        )),
    }
}

/// Decompress chunk data
pub fn decompress_chunk(data: &[u8], mode: CompressionMode) -> BlteResult<Vec<u8>> {
    match mode {
        CompressionMode::None => Ok(data.to_vec()),
        CompressionMode::ZLib => {
            let mut decoder = ZlibDecoder::new(data);
            let mut decompressed = Vec::new();

            // Read in chunks to enforce size limit
            let mut buffer = [0u8; 8192];
            loop {
                let bytes_read = decoder.read(&mut buffer).map_err(|e| {
                    BlteError::CompressionError(format!("ZLib decompression failed: {e}"))
                })?;

                if bytes_read == 0 {
                    break;
                }

                // Check size limit before extending
                if decompressed.len() + bytes_read > MAX_DECOMPRESSION_SIZE {
                    return Err(BlteError::CompressionError(format!(
                        "Decompressed size exceeds limit of {} bytes",
                        MAX_DECOMPRESSION_SIZE
                    )));
                }

                decompressed.extend_from_slice(&buffer[..bytes_read]);
            }

            Ok(decompressed)
        }
        CompressionMode::LZ4 => {
            // LZ4 decompression: 8-byte LE decompressed size prefix + single LZ4 block.
            // See compression comment above for format rationale.
            if data.len() < 8 {
                return Err(BlteError::CompressionError(
                    "LZ4 data too short - missing size header".to_string(),
                ));
            }

            let size_header = u64::from_le_bytes(
                data[0..8]
                    .try_into()
                    .map_err(|_| BlteError::CompressionError("Invalid size header".to_string()))?,
            );

            // Convert to usize with bounds checking
            let decompressed_size = usize::try_from(size_header).map_err(|_| {
                BlteError::CompressionError("Decompressed size too large".to_string())
            })?;

            // Security: Check against maximum decompression size to prevent DoS
            if decompressed_size > MAX_DECOMPRESSION_SIZE {
                return Err(BlteError::CompressionError(format!(
                    "LZ4 decompressed size {} exceeds limit of {} bytes",
                    decompressed_size, MAX_DECOMPRESSION_SIZE
                )));
            }

            // Decompress the remaining data
            let compressed_data = &data[8..];

            let decompressed = lz4_flex::block::decompress(compressed_data, decompressed_size)
                .map_err(|e| {
                    BlteError::CompressionError(format!("LZ4 decompression failed: {e}"))
                })?;

            // Validate decompressed size matches expected
            if decompressed.len() != decompressed_size {
                return Err(BlteError::CompressionError(format!(
                    "LZ4 decompression size mismatch: expected {decompressed_size}, got {}",
                    decompressed.len()
                )));
            }

            Ok(decompressed)
        }
        CompressionMode::Encrypted => {
            // Encryption mode requires special handling via decrypt_chunk_with_keys
            Err(BlteError::CompressionError(
                "Use decrypt_chunk_with_keys for encrypted chunks".to_string(),
            ))
        }
        #[allow(deprecated)]
        CompressionMode::Frame => Err(super::error::BlteError::UnsupportedCompressionMode(
            mode.as_byte(),
        )),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::{TactKey, TactKeyStore};

    #[test]
    fn test_compress_none() {
        let data = b"Hello, BLTE!";
        let compressed =
            compress_chunk(data, CompressionMode::None).expect("Test operation should succeed");
        assert_eq!(compressed, data);
    }

    #[test]
    fn test_decompress_none() {
        let data = b"Hello, BLTE!";
        let decompressed =
            decompress_chunk(data, CompressionMode::None).expect("Test operation should succeed");
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_lz4() {
        let data = b"Hello, BLTE! This is a test of LZ4 compression.";
        let compressed =
            compress_chunk(data, CompressionMode::LZ4).expect("Test operation should succeed");

        // Should have 8-byte size header + compressed data
        assert!(compressed.len() >= 8);

        // First 8 bytes should be the decompressed size in little-endian
        let size_bytes = &compressed[0..8];
        let stored_size =
            u64::from_le_bytes(size_bytes.try_into().expect("Operation should succeed"));
        assert_eq!(stored_size, data.len() as u64);
    }

    #[test]
    fn test_decompress_lz4() {
        let data = b"Hello, BLTE! This is a test of LZ4 decompression.";

        // First compress the data
        let compressed =
            compress_chunk(data, CompressionMode::LZ4).expect("Test operation should succeed");

        // Then decompress it
        let decompressed = decompress_chunk(&compressed, CompressionMode::LZ4)
            .expect("Test operation should succeed");

        // Should match original
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_lz4_round_trip() {
        let byte_sequence = (0..=255).collect::<Vec<u8>>();
        let test_cases = vec![
            b"Hello, world!".as_slice(),
            b"This is a longer string that should compress better with LZ4.".as_slice(),
            b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".as_slice(), // Highly compressible
            b"abcdefghijklmnopqrstuvwxyz0123456789".as_slice(),     // Less compressible
            b"".as_slice(),                                         // Empty data
            &[0u8; 1024],                                           // Large zeros
            &byte_sequence,                                         // Byte sequence
        ];

        for (i, original_data) in test_cases.into_iter().enumerate() {
            // Compress
            let compressed = compress_chunk(original_data, CompressionMode::LZ4)
                .expect("Test compression should succeed");

            // Decompress
            let decompressed = decompress_chunk(&compressed, CompressionMode::LZ4)
                .expect("Test decompression should succeed");

            // Verify
            assert_eq!(
                decompressed, original_data,
                "Round-trip failed for test case {i}"
            );
        }
    }

    #[test]
    fn test_lz4_invalid_data() {
        // Test with too short data (missing size header)
        let short_data = &[0x34, 0x01, 0x02];
        let result = decompress_chunk(short_data, CompressionMode::LZ4);
        assert!(result.is_err());

        // Test with invalid compressed data
        let mut invalid_data = vec![0u8; 8]; // 8-byte size header
        invalid_data.extend_from_slice(&[0xFF; 10]); // Invalid LZ4 data
        invalid_data[0..8].copy_from_slice(&100u64.to_le_bytes()); // Size header

        let result = decompress_chunk(&invalid_data, CompressionMode::LZ4);
        assert!(result.is_err());
    }

    #[test]
    fn test_lz4_size_mismatch() {
        let original = b"Hello, world!";
        let mut compressed =
            compress_chunk(original, CompressionMode::LZ4).expect("Test operation should succeed");

        // Corrupt the size header to indicate wrong decompressed size
        let wrong_size = (original.len() * 2) as u64;
        compressed[0..8].copy_from_slice(&wrong_size.to_le_bytes());

        let result = decompress_chunk(&compressed, CompressionMode::LZ4);
        assert!(result.is_err());
    }

    #[test]
    fn test_compress_zlib_round_trip() {
        let data = b"This is test data for ZLib compression round-trip testing.";

        // Compress
        let compressed =
            compress_chunk(data, CompressionMode::ZLib).expect("Test operation should succeed");

        // Should be smaller (or at least different)
        assert_ne!(compressed, data);

        // Decompress
        let decompressed = decompress_chunk(&compressed, CompressionMode::ZLib)
            .expect("Test operation should succeed");

        // Should match original
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_encryption_spec_creation() {
        // Test Salsa20 spec creation
        let salsa_spec = EncryptionSpec::salsa20(0x1234_5678_90AB_CDEF, [0x11, 0x22, 0x33, 0x44]);
        assert_eq!(salsa_spec.key_name, 0x1234_5678_90AB_CDEF);
        assert_eq!(salsa_spec.iv, [0x11, 0x22, 0x33, 0x44]);
        assert_eq!(salsa_spec.encryption_type, 0x53);
        assert!(salsa_spec.is_salsa20());
        assert!(!salsa_spec.is_arc4());

        // Test ARC4 spec creation
        let arc4_spec = EncryptionSpec::arc4(0xFEDC_BA09_8765_4321, [0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(arc4_spec.key_name, 0xFEDC_BA09_8765_4321);
        assert_eq!(arc4_spec.iv, [0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(arc4_spec.encryption_type, 0x41);
        assert!(arc4_spec.is_arc4());
        assert!(!arc4_spec.is_salsa20());
    }

    #[test]
    fn test_encrypt_chunk_salsa20() {
        let plaintext = b"Hello, BLTE encryption with Salsa20!";
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ];

        let spec = EncryptionSpec::salsa20(key_name, iv);
        let encrypted = encrypt_chunk_with_key(plaintext, spec, &key, 0)
            .expect("Test operation should succeed");

        // Check header structure
        assert!(encrypted.len() >= 17); // Header + at least some encrypted data
        assert_eq!(encrypted[0], 8); // Key name size
        assert_eq!(&encrypted[1..9], &key_name.to_le_bytes()); // Key name
        assert_eq!(encrypted[9], 4); // IV size
        assert_eq!(&encrypted[10..14], &iv); // IV
        assert_eq!(encrypted[14], 0x53); // Salsa20 encryption type

        // Encrypted data should be different from plaintext
        let encrypted_data = &encrypted[15..];
        assert_ne!(encrypted_data, plaintext);
        assert_eq!(encrypted_data.len(), plaintext.len());
    }

    #[test]
    fn test_decrypt_chunk_salsa20() {
        let plaintext = b"Hello, BLTE decryption with Salsa20!";
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ];

        // Create key store
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Encrypt first
        let spec = EncryptionSpec::salsa20(key_name, iv);
        let encrypted = encrypt_chunk_with_key(plaintext, spec, &key, 0)
            .expect("Test operation should succeed");

        // Decrypt
        let decrypted = decrypt_chunk_with_keys(&encrypted, &key_store, 0)
            .expect("Test operation should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encryption_round_trip() {
        let byte_sequence = (0..=255).collect::<Vec<u8>>();
        let zero_array = [0u8; 100];
        let test_cases = vec![
            b"Short text".as_slice(),
            b"This is a longer message that should still round-trip perfectly".as_slice(),
            &zero_array,                // Zeros
            &byte_sequence,             // Byte sequence
            b"minimal data".as_slice(), // Minimal data (must be long enough for header)
        ];

        let key_name = 0xDEAD_BEEF_CAFE_BABE;
        let iv = [0xAA, 0xBB, 0xCC, 0xDD];
        let key = [0x12; 16];

        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        for (i, plaintext) in test_cases.into_iter().enumerate() {
            let spec = EncryptionSpec::salsa20(key_name, iv);

            // Encrypt
            let encrypted = encrypt_chunk_with_key(plaintext, spec, &key, 0)
                .expect("Test encryption should succeed");

            // Decrypt
            let decrypted = decrypt_chunk_with_keys(&encrypted, &key_store, 0)
                .expect("Test decryption should succeed");

            assert_eq!(decrypted, plaintext, "Round-trip failed for test case {i}");
        }
    }

    #[test]
    fn test_encryption_different_block_indices() {
        let plaintext = b"Test data for different block indices";
        let key_name = 0x1111_2222_3333_4444;
        let iv = [0x01, 0x02, 0x03, 0x04];
        let key = [0xAB; 16];

        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        let spec = EncryptionSpec::salsa20(key_name, iv);

        // Different block indices should produce different ciphertexts
        let encrypted0 = encrypt_chunk_with_key(plaintext, spec, &key, 0)
            .expect("Test operation should succeed");
        let encrypted1 = encrypt_chunk_with_key(plaintext, spec, &key, 1)
            .expect("Test operation should succeed");
        let encrypted2 = encrypt_chunk_with_key(plaintext, spec, &key, 42)
            .expect("Test operation should succeed");

        // Headers should be identical except for encrypted data
        assert_eq!(&encrypted0[..15], &encrypted1[..15]);
        assert_eq!(&encrypted0[..15], &encrypted2[..15]);

        // But encrypted data should differ
        assert_ne!(&encrypted0[15..], &encrypted1[15..]);
        assert_ne!(&encrypted0[15..], &encrypted2[15..]);
        assert_ne!(&encrypted1[15..], &encrypted2[15..]);

        // But each should decrypt to the same plaintext
        let decrypted0 = decrypt_chunk_with_keys(&encrypted0, &key_store, 0)
            .expect("Test operation should succeed");
        let decrypted1 = decrypt_chunk_with_keys(&encrypted1, &key_store, 1)
            .expect("Test operation should succeed");
        let decrypted2 = decrypt_chunk_with_keys(&encrypted2, &key_store, 42)
            .expect("Test operation should succeed");

        assert_eq!(decrypted0, plaintext);
        assert_eq!(decrypted1, plaintext);
        assert_eq!(decrypted2, plaintext);
    }

    #[test]
    fn test_decrypt_missing_key() {
        let plaintext = b"Test data";
        let key_name = 0x1234_5678_90AB_CDEF;
        let missing_key_name = 0xFEDC_BA09_8765_4321;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [0x42; 16];

        // Create key store with one key
        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Encrypt with a different key name
        let spec = EncryptionSpec::salsa20(missing_key_name, iv);
        let encrypted = encrypt_chunk_with_key(plaintext, spec, &key, 0)
            .expect("Test operation should succeed");

        // Should fail to decrypt due to missing key
        let result = decrypt_chunk_with_keys(&encrypted, &key_store, 0);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("Test operation should fail")
                .to_string()
                .contains("Encryption key not found")
        );
    }

    #[test]
    fn test_decrypt_malformed_encrypted_chunk() {
        let key_store = TactKeyStore::new();

        // Test too short data
        let short_data = &[0x08, 0x01, 0x02]; // Only 3 bytes
        let result = decrypt_chunk_with_keys(short_data, &key_store, 0);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("Test operation should fail")
                .to_string()
                .contains("too short")
        );

        // Test wrong key name size
        let wrong_key_size = &[
            0x04, // Wrong key size (should be 8)
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // Still 8 bytes but header says 4
            0x04, // IV size
            0x11, 0x22, 0x33, 0x44, // IV
            0x53, // Encryption type
            0x00, 0x00, 0x00, // Some encrypted data
        ];
        let result = decrypt_chunk_with_keys(wrong_key_size, &key_store, 0);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("Test operation should fail")
                .to_string()
                .contains("Invalid key name size")
        );
    }

    #[test]
    fn test_arc4_round_trip() {
        let plaintext = b"Test data for ARC4 encryption";
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [0x42; 16];

        // Create key store
        let mut key_store = TactKeyStore::new();
        let tact_key = cascette_crypto::TactKey::new(key_name, key);
        key_store.add(tact_key);

        // Test encryption
        let spec = EncryptionSpec::arc4(key_name, iv);
        let encrypted_result = encrypt_chunk_with_key(plaintext, spec, &key, 0);
        assert!(encrypted_result.is_ok());
        let encrypted_data = encrypted_result.expect("Test operation should succeed");

        // Encrypted data should be different from plaintext
        assert_ne!(plaintext, &encrypted_data[..]);

        // Test decryption
        let decrypted_result = decrypt_chunk_with_keys(&encrypted_data, &key_store, 0);
        assert!(decrypted_result.is_ok());
        let decrypted_data = decrypted_result.expect("Test operation should succeed");

        // Should decrypt back to original plaintext
        assert_eq!(plaintext, &decrypted_data[..]);
    }

    #[test]
    fn test_encrypted_compressed_data() {
        // Test encrypting already compressed data
        let plaintext = b"This is test data that will be compressed then encrypted";
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [0x42; 16];

        let mut key_store = TactKeyStore::new();
        let tact_key = TactKey::new(key_name, key);
        key_store.add(tact_key);

        // First compress with ZLib
        let compressed = compress_chunk(plaintext, CompressionMode::ZLib)
            .expect("Test operation should succeed");

        // Prepend compression mode byte
        let mut compressed_with_mode = vec![CompressionMode::ZLib.as_byte()];
        compressed_with_mode.extend_from_slice(&compressed);

        // Then encrypt the compressed data
        let spec = EncryptionSpec::salsa20(key_name, iv);
        let encrypted = encrypt_chunk_with_key(&compressed_with_mode, spec, &key, 0)
            .expect("Test operation should succeed");

        // Decrypt should automatically decompress
        let decrypted = decrypt_chunk_with_keys(&encrypted, &key_store, 0)
            .expect("Test operation should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_8byte_iv() {
        let plaintext = b"Test data with 8-byte IV";
        let key_name = 0x1234_5678_90AB_CDEF;
        let key = [0x42; 16];

        let mut key_store = TactKeyStore::new();
        key_store.add(TactKey::new(key_name, key));

        // Manually build an encrypted chunk with 8-byte IV
        let iv8 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let encrypted_payload =
            encrypt_salsa20(plaintext, &key, &iv8, 0).expect("Salsa20 should succeed");

        let mut chunk_data = Vec::new();
        chunk_data.push(8); // key_name_size
        chunk_data.extend_from_slice(&key_name.to_le_bytes());
        chunk_data.push(8); // iv_size = 8
        chunk_data.extend_from_slice(&iv8);
        chunk_data.push(0x53); // Salsa20
        chunk_data.extend_from_slice(&encrypted_payload);

        let decrypted = decrypt_chunk_with_keys(&chunk_data, &key_store, 0)
            .expect("Decryption with 8-byte IV should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_iv_size() {
        let key_name = 0x1234_5678_90AB_CDEF;
        let key = [0x42; 16];

        let mut key_store = TactKeyStore::new();
        key_store.add(TactKey::new(key_name, key));

        // Build chunk with invalid IV size (6)
        let mut chunk_data = Vec::new();
        chunk_data.push(8); // key_name_size
        chunk_data.extend_from_slice(&key_name.to_le_bytes());
        chunk_data.push(6); // invalid iv_size
        chunk_data.extend_from_slice(&[0x11; 6]);
        chunk_data.push(0x53);
        chunk_data.extend_from_slice(&[0x00; 10]);

        let result = decrypt_chunk_with_keys(&chunk_data, &key_store, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid IV size"));
    }

    #[test]
    fn test_decrypt_nested_encryption_rejected() {
        let key_name = 0x1234_5678_90AB_CDEF;
        let iv = [0x11, 0x22, 0x33, 0x44];
        let key = [0x42; 16];

        let mut key_store = TactKeyStore::new();
        key_store.add(TactKey::new(key_name, key));

        // Create inner data with encryption mode byte (0x45 = 'E')
        let mut inner_data = vec![CompressionMode::Encrypted.as_byte()];
        inner_data.extend_from_slice(b"fake inner encrypted data");

        // Encrypt the inner data (simulating nested encryption)
        let spec = EncryptionSpec::salsa20(key_name, iv);
        let encrypted =
            encrypt_chunk_with_key(&inner_data, spec, &key, 0).expect("Encryption should succeed");

        let result = decrypt_chunk_with_keys(&encrypted, &key_store, 0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("nested encryption")
        );
    }
}

/// Encryption specification for BLTE chunks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncryptionSpec {
    /// 64-bit key name
    pub key_name: u64,
    /// 32-bit initialization vector
    pub iv: [u8; 4],
    /// Encryption type (0x53 for `Salsa20`, 0x41 for `ARC4`)
    pub encryption_type: u8,
}

impl EncryptionSpec {
    /// Create new encryption specification for `Salsa20`
    pub fn salsa20(key_name: u64, iv: [u8; 4]) -> Self {
        Self {
            key_name,
            iv,
            encryption_type: 0x53, // 'S' for Salsa20
        }
    }

    /// Create new encryption specification for `ARC4`
    pub fn arc4(key_name: u64, iv: [u8; 4]) -> Self {
        Self {
            key_name,
            iv,
            encryption_type: 0x41, // 'A' for ARC4
        }
    }

    /// Check if this is `Salsa20` encryption
    pub fn is_salsa20(&self) -> bool {
        self.encryption_type == 0x53
    }

    /// Check if this is `ARC4` encryption
    pub fn is_arc4(&self) -> bool {
        self.encryption_type == 0x41
    }
}

/// Encrypt chunk data with BLTE encryption format
///
/// Format: `[key_name_size:1] [key_name:8] [iv_size:1] [iv:4] [type:1] [encrypted_data...]`
///
/// Note: The caller must prepend the 0x45 mode byte
pub fn encrypt_chunk_with_key(
    data: &[u8],
    spec: EncryptionSpec,
    key: &[u8; 16],
    block_index: usize,
) -> BlteResult<Vec<u8>> {
    // Encrypt the data based on encryption type
    let encrypted_data = match spec.encryption_type {
        0x53 => {
            // Salsa20 encryption
            encrypt_salsa20(data, key, &spec.iv, block_index).map_err(|e| {
                BlteError::CompressionError(format!("Salsa20 encryption failed: {e}"))
            })?
        }
        0x41 => {
            // ARC4 encryption
            let mut cipher = cascette_crypto::Arc4Cipher::new(key.as_slice()).map_err(|e| {
                BlteError::CompressionError(format!("ARC4 initialization failed: {e}"))
            })?;

            cipher.encrypt(data)
        }
        _ => {
            return Err(BlteError::CompressionError(format!(
                "Unknown encryption type: 0x{:02X}",
                spec.encryption_type
            )));
        }
    };

    // Build encrypted chunk header
    let mut result = Vec::new();

    // Key name size (always 8 for 64-bit keys)
    result.push(8);

    // Key name (64-bit little-endian)
    result.extend_from_slice(&spec.key_name.to_le_bytes());

    // IV size (always 4)
    result.push(4);

    // IV (4 bytes)
    result.extend_from_slice(&spec.iv);

    // Encryption type
    result.push(spec.encryption_type);

    // Encrypted data
    result.extend_from_slice(&encrypted_data);

    Ok(result)
}

/// Parse and decrypt encrypted chunk data
///
/// Expects data without the 0x45 mode byte
pub fn decrypt_chunk_with_keys(
    data: &[u8],
    key_store: &TactKeyStore,
    block_index: usize,
) -> BlteResult<Vec<u8>> {
    if data.len() < 17 {
        return Err(BlteError::CompressionError(format!(
            "Encrypted chunk too short: {} bytes (minimum 17)",
            data.len()
        )));
    }

    let mut offset = 0;

    // Read key name size
    let key_name_size = data[offset];
    offset += 1;

    if key_name_size != 8 {
        return Err(BlteError::CompressionError(format!(
            "Invalid key name size: {key_name_size} (expected 8)"
        )));
    }

    if data.len() < offset + 8 {
        return Err(BlteError::CompressionError(
            "Encrypted chunk too short for key name".to_string(),
        ));
    }

    // Read key name (64-bit little-endian)
    let key_name = u64::from_le_bytes(
        data[offset..offset + 8]
            .try_into()
            .map_err(|_| BlteError::CompressionError("Invalid key name".to_string()))?,
    );
    offset += 8;

    // Look up key
    let key = key_store.get(key_name).ok_or_else(|| {
        BlteError::CompressionError(format!("Encryption key not found: 0x{key_name:016X}"))
    })?;

    if data.len() < offset + 1 {
        return Err(BlteError::CompressionError(
            "Encrypted chunk too short for IV size".to_string(),
        ));
    }

    // Read IV size (must be 4 or 8, matching TACTSharp and CascLib)
    let iv_size = data[offset];
    offset += 1;

    if iv_size != 4 && iv_size != 8 {
        return Err(BlteError::InvalidIvSize { actual: iv_size });
    }

    let iv_len = iv_size as usize;
    if data.len() < offset + iv_len {
        return Err(BlteError::CompressionError(
            "Encrypted chunk too short for IV".to_string(),
        ));
    }

    // Read IV (4 or 8 bytes)
    let iv = &data[offset..offset + iv_len];
    offset += iv_len;

    if data.len() < offset + 1 {
        return Err(BlteError::CompressionError(
            "Encrypted chunk too short for encryption type".to_string(),
        ));
    }

    // Read encryption type
    let encryption_type = data[offset];
    offset += 1;

    // Get encrypted data
    let encrypted_data = &data[offset..];

    // Decrypt based on encryption type
    let decrypted_data = match encryption_type {
        0x53 => {
            // Salsa20 decryption (accepts 4 or 8 byte IV)
            decrypt_salsa20(encrypted_data, key, iv, block_index).map_err(|e| {
                BlteError::CompressionError(format!("Salsa20 decryption failed: {e}"))
            })?
        }
        0x41 => {
            // ARC4 decryption
            let mut cipher = cascette_crypto::Arc4Cipher::new(key.as_slice()).map_err(|e| {
                BlteError::CompressionError(format!("ARC4 initialization failed: {e}"))
            })?;

            cipher.decrypt(encrypted_data)
        }
        _ => {
            return Err(BlteError::CompressionError(format!(
                "Unknown encryption type: 0x{encryption_type:02X}"
            )));
        }
    };

    // Check if decrypted data has a compression mode marker
    if !decrypted_data.is_empty()
        && let Some(inner_mode) = CompressionMode::from_byte(decrypted_data[0])
    {
        // Nested encryption (E inside E) is not valid
        if inner_mode == CompressionMode::Encrypted {
            return Err(BlteError::NestedEncryption);
        }
        // Decompress the decrypted data
        return decompress_chunk(&decrypted_data[1..], inner_mode);
    }

    Ok(decrypted_data)
}

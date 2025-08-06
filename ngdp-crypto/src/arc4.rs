//! ARC4 decryption implementation for BLTE mode 'E'
//!
//! This module implements ARC4 (RC4) stream cipher decryption specifically
//! for BLTE encrypted blocks, following Blizzard's key construction pattern.

use generic_array::typenum::U32;
use rc4::{KeyInit, Rc4, StreamCipher};
use tracing::{debug, trace};

use crate::{CryptoError, Result};

/// Decrypt data using ARC4 cipher with BLTE-specific key construction
///
/// BLTE ARC4 uses a specific key construction pattern:
/// 1. Start with 16-byte base key
/// 2. Append 4-byte IV
/// 3. Append 4-byte block index (little-endian)
/// 4. Pad to exactly 32 bytes with zeros
/// 5. Use resulting key for RC4
///
/// # Arguments
///
/// * `data` - Encrypted data to decrypt
/// * `key` - 16-byte base encryption key
/// * `iv` - 4-byte initialization vector  
/// * `block_index` - Block index for multi-chunk files
///
/// # Returns
///
/// Decrypted data
pub fn decrypt_arc4(data: &[u8], key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Vec<u8>> {
    if iv.len() != 4 {
        return Err(CryptoError::InvalidParameter(format!(
            "IV must be 4 bytes, got {}",
            iv.len()
        )));
    }

    trace!(
        "ARC4 decrypt: {} bytes, block_index={}",
        data.len(),
        block_index
    );

    // CRITICAL: Create combined key following BLTE pattern
    let mut arc4_key = Vec::with_capacity(32);

    // Add base key (16 bytes)
    arc4_key.extend_from_slice(key);

    // Add IV (4 bytes)
    arc4_key.extend_from_slice(iv);

    // Add block index as little-endian bytes (4 bytes)
    arc4_key.extend_from_slice(&(block_index as u32).to_le_bytes());

    // CRITICAL: Pad to exactly 32 bytes with zeros
    while arc4_key.len() < 32 {
        arc4_key.push(0);
    }

    debug!(
        "ARC4 key construction: base_key(16) + iv(4) + block_index(4) + padding({}) = {} bytes",
        32 - 24,
        arc4_key.len()
    );

    // Create cipher and decrypt
    let mut cipher: Rc4<U32> = Rc4::new_from_slice(&arc4_key).map_err(|_| {
        CryptoError::InitializationFailed("Failed to create RC4 cipher".to_string())
    })?;

    let mut decrypted = data.to_vec();
    cipher.apply_keystream(&mut decrypted);

    debug!(
        "ARC4 decrypted {} bytes -> {} bytes",
        data.len(),
        decrypted.len()
    );

    Ok(decrypted)
}

/// Encrypt data using ARC4 cipher (for testing)
///
/// Uses the same key construction as decrypt_arc4 but for encryption.
/// This is primarily useful for testing round-trip encryption/decryption.
pub fn encrypt_arc4(data: &[u8], key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Vec<u8>> {
    // ARC4 is symmetric, so encryption and decryption are identical
    decrypt_arc4(data, key, iv, block_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc4_roundtrip() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;
        let plaintext = b"Hello, BLTE ARC4 world!";

        // Encrypt
        let encrypted = encrypt_arc4(plaintext, &key, &iv, block_index).unwrap();

        // Should be different from original
        assert_ne!(encrypted, plaintext);

        // Decrypt
        let decrypted = decrypt_arc4(&encrypted, &key, &iv, block_index).unwrap();

        // Should match original
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_arc4_block_index_affects_output() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Test data for block index variation";

        // Encrypt with different block indices
        let encrypted_0 = encrypt_arc4(plaintext, &key, &iv, 0).unwrap();
        let encrypted_1 = encrypt_arc4(plaintext, &key, &iv, 1).unwrap();

        // Should produce different ciphertext due to different keys
        assert_ne!(encrypted_0, encrypted_1);

        // But decrypt correctly with matching indices
        let decrypted_0 = decrypt_arc4(&encrypted_0, &key, &iv, 0).unwrap();
        let decrypted_1 = decrypt_arc4(&encrypted_1, &key, &iv, 1).unwrap();

        assert_eq!(decrypted_0, plaintext);
        assert_eq!(decrypted_1, plaintext);
    }

    #[test]
    fn test_arc4_different_keys_produce_different_output() {
        let key1 = [0x01u8; 16];
        let key2 = [0x02u8; 16];
        let iv = [0x03, 0x04, 0x05, 0x06];
        let block_index = 0;
        let plaintext = b"Sensitive data";

        let encrypted1 = encrypt_arc4(plaintext, &key1, &iv, block_index).unwrap();
        let encrypted2 = encrypt_arc4(plaintext, &key2, &iv, block_index).unwrap();

        // Different keys should produce different ciphertext
        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_arc4_invalid_iv_size() {
        let key = [0x01u8; 16];
        let invalid_iv = [0x02, 0x03]; // Only 2 bytes instead of 4
        let block_index = 0;
        let data = b"test";

        let result = decrypt_arc4(data, &key, &invalid_iv, block_index);
        assert!(result.is_err());
    }

    #[test]
    fn test_arc4_key_construction() {
        let key = [0xAAu8; 16];
        let iv = [0xBB, 0xCC, 0xDD, 0xEE];
        let block_index = 0x12345678;
        let plaintext = b"test data";

        // This test verifies the key construction is working by ensuring
        // consistent results with the same inputs
        let encrypted1 = encrypt_arc4(plaintext, &key, &iv, block_index).unwrap();
        let encrypted2 = encrypt_arc4(plaintext, &key, &iv, block_index).unwrap();

        // Same inputs should produce identical output
        assert_eq!(encrypted1, encrypted2);

        // Verify decryption works
        let decrypted = decrypt_arc4(&encrypted1, &key, &iv, block_index).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_arc4_empty_data() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;
        let empty_data = b"";

        let encrypted = encrypt_arc4(empty_data, &key, &iv, block_index).unwrap();
        assert_eq!(encrypted.len(), 0);

        let decrypted = decrypt_arc4(&encrypted, &key, &iv, block_index).unwrap();
        assert_eq!(decrypted, empty_data);
    }
}

//! ARC4 decryption implementation for BLTE mode 'E'
//!
//! This module implements ARC4 (RC4) stream cipher decryption specifically
//! for BLTE encrypted blocks, following Blizzard's key construction pattern.

use crate::Result;
use cipher::StreamCipher;
use generic_array::typenum::U32;
use rc4::{KeyInit, Rc4};
use tracing::debug;

/// Create BLTE ARC4 stream cipher.
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
/// * `key` - 16-byte base encryption key
/// * `iv` - 4-byte initialization vector  
/// * `block_index` - Block index for multi-chunk files
pub fn init_arc4(key: &[u8; 16], iv: &[u8; 4], block_index: u32) -> Rc4<U32> {
    // Create combined key following BLTE pattern
    let mut arc4_key = [0; 32];

    // Add base key (16 bytes)
    arc4_key[..16].copy_from_slice(key);

    // Add IV (4 bytes)
    arc4_key[16..20].copy_from_slice(iv);

    // Add block index as little-endian bytes (4 bytes)
    arc4_key[20..24].copy_from_slice(&block_index.to_le_bytes());

    debug!(
        "ARC4 key construction: base_key(16) + iv(4) + block_index(4) + padding({}) = {} bytes",
        32 - 24,
        arc4_key.len()
    );

    // Create cipher and decrypt
    Rc4::new(&arc4_key.into())
}

/// Decrypt data in-place using ARC4 cipher with BLTE-specific key construction.
///
/// This is a convenience method for small buffers, which requires the entire
/// stream loaded in memory. Use [`init_arc4()`] instead.
pub fn decrypt_arc4(data: &mut [u8], key: &[u8; 16], iv: &[u8; 4], block_index: u32) -> Result<()> {
    let mut cipher = init_arc4(key, iv, block_index);
    cipher.try_apply_keystream(data)?;

    Ok(())
}

/// Encrypt data using ARC4 cipher (for testing)
///
/// Uses the same algorithm as [decrypt][decrypt_arc4] (stream ciphers are
/// symmetric).
///
/// This is a convenience method for small buffers, which requires the entire
/// stream loaded in memory. Use [`init_arc4()`] instead.
pub fn encrypt_arc4(data: &mut [u8], key: &[u8; 16], iv: &[u8; 4], block_index: u32) -> Result<()> {
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
        let mut encrypted = plaintext.clone();

        // Encrypt
        encrypt_arc4(&mut encrypted, &key, &iv, block_index).unwrap();

        // Should be different from original
        assert_ne!(&encrypted, plaintext);

        // Decrypt
        decrypt_arc4(&mut encrypted, &key, &iv, block_index).unwrap();

        // Should match original
        assert_eq!(&encrypted, plaintext);
    }

    #[test]
    fn test_arc4_block_index_affects_output() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Test data for block index variation";

        // Encrypt with different block indices
        let mut encrypted_0 = plaintext.clone();
        encrypt_arc4(&mut encrypted_0, &key, &iv, 0).unwrap();
        let mut encrypted_1 = plaintext.clone();
        encrypt_arc4(&mut encrypted_1, &key, &iv, 1).unwrap();

        // Should produce different ciphertext due to different keys
        assert_ne!(encrypted_0, encrypted_1);

        // But decrypt correctly with matching indices
        decrypt_arc4(&mut encrypted_0, &key, &iv, 0).unwrap();
        decrypt_arc4(&mut encrypted_1, &key, &iv, 1).unwrap();

        assert_eq!(&encrypted_0, plaintext);
        assert_eq!(&encrypted_1, plaintext);
    }

    #[test]
    fn test_arc4_different_keys_produce_different_output() {
        let key1 = [0x01u8; 16];
        let key2 = [0x02u8; 16];
        let iv = [0x03, 0x04, 0x05, 0x06];
        let block_index = 0;
        let plaintext = b"Sensitive data";

        let mut encrypted_1 = plaintext.clone();
        encrypt_arc4(&mut encrypted_1, &key1, &iv, block_index).unwrap();
        let mut encrypted_2 = plaintext.clone();
        encrypt_arc4(&mut encrypted_2, &key2, &iv, block_index).unwrap();

        // Different keys should produce different ciphertext
        assert_ne!(encrypted_1, encrypted_2);
    }

    #[test]
    fn test_arc4_key_construction() {
        let key = [0xAAu8; 16];
        let iv = [0xBB, 0xCC, 0xDD, 0xEE];
        let block_index = 0x12345678;
        let plaintext = b"test data";

        // This test verifies the key construction is working by ensuring
        // consistent results with the same inputs
        let mut encrypted_1 = plaintext.clone();
        encrypt_arc4(&mut encrypted_1, &key, &iv, block_index).unwrap();
        let mut encrypted_2 = plaintext.clone();
        encrypt_arc4(&mut encrypted_2, &key, &iv, block_index).unwrap();

        // Same inputs should produce identical output
        assert_eq!(&encrypted_1, &encrypted_2);

        // Verify decryption works
        decrypt_arc4(&mut encrypted_1, &key, &iv, block_index).unwrap();
        assert_eq!(&encrypted_1, plaintext);
    }

    #[test]
    fn test_arc4_empty_data() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;
        let mut empty_data = [];

        encrypt_arc4(&mut empty_data, &key, &iv, block_index).unwrap();
        decrypt_arc4(&mut empty_data, &key, &iv, block_index).unwrap();
    }
}

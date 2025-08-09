//! Salsa20 stream cipher implementation for BLTE encryption.

use cipher::{KeyIvInit, StreamCipher};
use salsa20::Salsa20;

use crate::Result;
use crate::error::CryptoError;

/// Decrypt data using Salsa20 stream cipher.
///
/// This implements the specific Salsa20 variant used by BLTE:
/// - 16-byte key is extended to 32 bytes by duplication
/// - 4-byte IV is extended to 8 bytes by duplication
/// - Block index is XORed with first 4 bytes of IV
pub fn decrypt_salsa20(
    data: &[u8],
    key: &[u8; 16],
    iv: &[u8],
    block_index: usize,
) -> Result<Vec<u8>> {
    // Validate IV size
    if iv.len() != 4 {
        return Err(CryptoError::InvalidIvSize {
            expected: 4,
            actual: iv.len(),
        });
    }

    // Extend 16-byte key to 32 bytes by duplication
    let mut extended_key = [0u8; 32];
    extended_key[..16].copy_from_slice(key);
    extended_key[16..].copy_from_slice(key);

    // Extend 4-byte IV to 8 bytes and XOR with block index
    let mut extended_iv = [0u8; 8];

    // Copy IV twice
    extended_iv[..4].copy_from_slice(iv);
    extended_iv[4..].copy_from_slice(iv);

    // XOR block index with first 4 bytes
    let block_bytes = (block_index as u32).to_le_bytes();
    for i in 0..4 {
        extended_iv[i] ^= block_bytes[i];
    }

    // Create cipher and decrypt
    let mut cipher = Salsa20::new(&extended_key.into(), &extended_iv.into());
    let mut output = data.to_vec();
    cipher.apply_keystream(&mut output);

    Ok(output)
}

/// Encrypt data using Salsa20 stream cipher.
///
/// Uses the same algorithm as decrypt (stream ciphers are symmetric).
pub fn encrypt_salsa20(
    data: &[u8],
    key: &[u8; 16],
    iv: &[u8],
    block_index: usize,
) -> Result<Vec<u8>> {
    // Salsa20 is symmetric - encryption and decryption are the same
    decrypt_salsa20(data, key, iv, block_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_salsa20_round_trip() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Hello, World! This is a test message.";
        let block_index = 0;

        // Encrypt
        let ciphertext = encrypt_salsa20(plaintext, &key, &iv, block_index).unwrap();
        assert_ne!(&ciphertext[..], plaintext);

        // Decrypt
        let decrypted = decrypt_salsa20(&ciphertext, &key, &iv, block_index).unwrap();
        assert_eq!(&decrypted[..], plaintext);
    }

    #[test]
    fn test_salsa20_block_index() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Test data";

        // Different block indices should produce different ciphertexts
        let cipher1 = encrypt_salsa20(plaintext, &key, &iv, 0).unwrap();
        let cipher2 = encrypt_salsa20(plaintext, &key, &iv, 1).unwrap();
        let cipher3 = encrypt_salsa20(plaintext, &key, &iv, 100).unwrap();

        assert_ne!(cipher1, cipher2);
        assert_ne!(cipher2, cipher3);
        assert_ne!(cipher1, cipher3);

        // But each should decrypt correctly with the right block index
        let plain1 = decrypt_salsa20(&cipher1, &key, &iv, 0).unwrap();
        let plain2 = decrypt_salsa20(&cipher2, &key, &iv, 1).unwrap();
        let plain3 = decrypt_salsa20(&cipher3, &key, &iv, 100).unwrap();

        assert_eq!(plain1, plaintext);
        assert_eq!(plain2, plaintext);
        assert_eq!(plain3, plaintext);
    }

    #[test]
    fn test_salsa20_invalid_iv() {
        let key = [0x01u8; 16];
        let invalid_iv = [0x02, 0x03]; // Too short
        let plaintext = b"Test";

        let result = encrypt_salsa20(plaintext, &key, &invalid_iv, 0);
        assert!(result.is_err());

        match result.unwrap_err() {
            CryptoError::InvalidIvSize { expected, actual } => {
                assert_eq!(expected, 4);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected InvalidIvSize error"),
        }
    }

    #[test]
    fn test_key_extension() {
        // Test that the key extension works correctly
        let key = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        let iv = [0x01, 0x02, 0x03, 0x04];
        let plaintext = b"Test";

        // This should work without panicking
        let encrypted = encrypt_salsa20(plaintext, &key, &iv, 0).unwrap();
        let decrypted = decrypt_salsa20(&encrypted, &key, &iv, 0).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}

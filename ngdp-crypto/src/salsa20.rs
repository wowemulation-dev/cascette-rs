//! Salsa20 stream cipher implementation for BLTE encryption.

use cipher::{KeyIvInit, StreamCipher};
use salsa20::Salsa20;

use crate::Result;

/// Create BLTE Salsa20 stream cipher.
///
/// This implements the specific Salsa20 variant used by BLTE:
/// - 16-byte key is extended to 32 bytes by duplication
/// - 4-byte IV is extended to 8 bytes by duplication  
/// - Block index is XORed with first 4 bytes of IV
pub fn init_salsa20(key: &[u8; 16], iv: &[u8; 4], block_index: u32) -> Salsa20 {
    // Extend 16-byte key to 32 bytes by duplication
    let mut extended_key = [0u8; 32];
    extended_key[..16].copy_from_slice(key);
    extended_key[16..].copy_from_slice(key);

    // Extend 4-byte IV to 8 bytes
    let mut extended_iv = [0u8; 8];
    extended_iv[..4].copy_from_slice(iv);
    extended_iv[4..].copy_from_slice(iv);

    // XOR block index with first 4 bytes
    let block_bytes = block_index.to_le_bytes();
    for i in 0..block_bytes.len() {
        extended_iv[i] ^= block_bytes[i];
    }

    // Create cipher and decrypt
    Salsa20::new(&extended_key.into(), &extended_iv.into())
}

/// Decrypt an in-memory BLTE Salsa20 buffer in-place.
///
/// This is a convenience method for small buffers, which requires the entire
/// stream loaded in memory. Use [`init_salsa20()`] instead.
pub fn decrypt_salsa20(
    data: &mut [u8],
    key: &[u8; 16],
    iv: &[u8; 4],
    block_index: u32,
) -> Result<()> {
    let mut cipher = init_salsa20(key, iv, block_index);
    cipher.try_apply_keystream(data)?;

    Ok(())
}

/// Encrypt an in-memory buffer using BLTE Salsa20, in-place.
///
/// Uses the same algorithm as [decrypt][decrypt_salsa20] (stream ciphers are
/// symmetric).
///
/// This is a convenience method for small buffers, which requires the entire
/// stream loaded in memory. Use [`init_salsa20()`] instead.
pub fn encrypt_salsa20(
    data: &mut [u8],
    key: &[u8; 16],
    iv: &[u8; 4],
    block_index: u32,
) -> Result<()> {
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
        let mut buf = plaintext.clone();
        let block_index = 0;

        // Encrypt
        encrypt_salsa20(&mut buf, &key, &iv, block_index).unwrap();
        assert_ne!(&buf, plaintext);

        // Decrypt
        decrypt_salsa20(&mut buf, &key, &iv, block_index).unwrap();
        assert_eq!(&buf, plaintext);
    }

    #[test]
    fn test_salsa20_block_index() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Test data";

        // Different block indices should produce different ciphertexts
        let mut cipher1 = plaintext.clone();
        encrypt_salsa20(&mut cipher1, &key, &iv, 0).unwrap();
        let mut cipher2 = plaintext.clone();
        encrypt_salsa20(&mut cipher2, &key, &iv, 1).unwrap();
        let mut cipher3 = plaintext.clone();
        encrypt_salsa20(&mut cipher3, &key, &iv, 100).unwrap();

        assert_ne!(cipher1, cipher2);
        assert_ne!(cipher2, cipher3);
        assert_ne!(cipher1, cipher3);

        // But each should decrypt correctly with the right block index
        decrypt_salsa20(&mut cipher1, &key, &iv, 0).unwrap();
        decrypt_salsa20(&mut cipher2, &key, &iv, 1).unwrap();
        decrypt_salsa20(&mut cipher3, &key, &iv, 100).unwrap();

        assert_eq!(&cipher1, plaintext);
        assert_eq!(&cipher2, plaintext);
        assert_eq!(&cipher3, plaintext);
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
        let mut buf = plaintext.clone();
        encrypt_salsa20(&mut buf, &key, &iv, 0).unwrap();
        decrypt_salsa20(&mut buf, &key, &iv, 0).unwrap();
        assert_eq!(&buf, plaintext);
    }
}

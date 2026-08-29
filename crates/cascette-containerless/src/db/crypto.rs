//! Salsa20 encryption wrapper for SQLite database bytes.
//!
//! The containerless database file is encrypted as a whole using the
//! Salsa20 cipher from `cascette-crypto`. The key comes from `build-file-db`
//! in the build config.

use cascette_crypto::salsa20::{decrypt_salsa20, encrypt_salsa20};

use crate::error::ContainerlessResult;

/// Decrypt an entire SQLite database blob.
///
/// The key is 16 bytes from `build-file-db`. The IV source is configurable:
/// it may be derived from the content key, stored alongside the database,
/// or use a fixed value. Pass the IV explicitly.
pub fn decrypt_db(data: &[u8], key: &[u8; 16], iv: &[u8]) -> ContainerlessResult<Vec<u8>> {
    let decrypted = decrypt_salsa20(data, key, iv, 0)?;
    Ok(decrypted)
}

/// Encrypt an entire SQLite database blob.
pub fn encrypt_db(data: &[u8], key: &[u8; 16], iv: &[u8]) -> ContainerlessResult<Vec<u8>> {
    let encrypted = encrypt_salsa20(data, key, iv, 0)?;
    Ok(encrypted)
}

/// Derive a 4-byte IV from a 16-byte key by taking the first 4 bytes.
///
/// This is the default fallback when no explicit IV is provided.
#[must_use]
pub fn iv_from_key(key: &[u8; 16]) -> [u8; 4] {
    let mut iv = [0u8; 4];
    iv.copy_from_slice(&key[..4]);
    iv
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let key = [0x42u8; 16];
        let iv = iv_from_key(&key);
        let data = b"SQLite format 3\x00some database content here";

        let encrypted = encrypt_db(data, &key, &iv).unwrap();
        assert_ne!(&encrypted[..], &data[..]);

        let decrypted = decrypt_db(&encrypted, &key, &iv).unwrap();
        assert_eq!(&decrypted[..], &data[..]);
    }

    #[test]
    fn test_iv_from_key() {
        let key = [
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xFF, 0x00,
        ];
        let iv = iv_from_key(&key);
        assert_eq!(iv, [0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn test_different_keys_produce_different_output() {
        let key1 = [0x01u8; 16];
        let key2 = [0x02u8; 16];
        let iv1 = iv_from_key(&key1);
        let iv2 = iv_from_key(&key2);
        let data = b"test data";

        let enc1 = encrypt_db(data, &key1, &iv1).unwrap();
        let enc2 = encrypt_db(data, &key2, &iv2).unwrap();
        assert_ne!(enc1, enc2);
    }
}

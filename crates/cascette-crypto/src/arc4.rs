//! ARC4 (RC4-compatible) stream cipher implementation for CASC encryption.
//!
//! This implementation provides ARC4 encryption/decryption for BLTE encrypted blocks
//! that use ARC4 instead of Salsa20. ARC4 is rarely used in modern CASC files but
//! is included for completeness.
//!
//! ## Security Warning
//!
//! ARC4 has known cryptographic weaknesses and should not be used for new applications.
//! This implementation is provided solely for compatibility with legacy CASC encrypted
//! blocks that may use ARC4 encryption.
//!
//! ## Usage
//!
//! ```rust
//! use cascette_crypto::arc4::Arc4Cipher;
//!
//! let key = b"test_key";
//! let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
//!
//! let plaintext = b"Hello, World!";
//! let ciphertext = cipher.encrypt(plaintext);
//!
//! // Reset cipher state for decryption
//! let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
//! let decrypted = cipher.decrypt(&ciphertext);
//! assert_eq!(plaintext, &decrypted[..]);
//! ```

use thiserror::Error;

/// Errors that can occur during ARC4 operations.
#[derive(Error, Debug)]
pub enum Arc4Error {
    /// Invalid key length provided
    #[error("Invalid key length: {0} (must be 1-256 bytes)")]
    InvalidKeyLength(usize),
}

/// ARC4 stream cipher implementation.
///
/// This is a straightforward implementation of the ARC4 algorithm (RC4-compatible)
/// for use with CASC BLTE encrypted blocks. The cipher maintains internal state
/// and produces a keystream that is `XORed` with plaintext/ciphertext.
pub struct Arc4Cipher {
    /// S-box state (256 bytes)
    s: [u8; 256],
    /// Current indices
    i: u8,
    j: u8,
}

impl Arc4Cipher {
    /// Create a new ARC4 cipher with the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The encryption key (1-256 bytes)
    ///
    /// # Errors
    ///
    /// Returns `Arc4Error::InvalidKeyLength` if the key is empty or longer than 256 bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cascette_crypto::arc4::Arc4Cipher;
    ///
    /// let key = b"my_secret_key";
    /// let cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
    /// ```
    pub fn new(key: &[u8]) -> Result<Self, Arc4Error> {
        if key.is_empty() || key.len() > 256 {
            return Err(Arc4Error::InvalidKeyLength(key.len()));
        }

        let mut cipher = Self {
            s: [0; 256],
            i: 0,
            j: 0,
        };

        // Initialize S-box with identity permutation
        #[allow(clippy::cast_possible_truncation)] // i is 0..256, cast is safe
        for i in 0..256 {
            cipher.s[i] = i as u8;
        }

        // Key-scheduling algorithm (KSA)
        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(cipher.s[i]).wrapping_add(key[i % key.len()]);
            cipher.s.swap(i, j as usize);
        }

        Ok(cipher)
    }

    /// Generate the next keystream byte.
    ///
    /// This implements the pseudo-random generation algorithm (PRGA) of ARC4.
    fn next_keystream_byte(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.s[self.i as usize]);

        self.s.swap(self.i as usize, self.j as usize);

        let k = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
        self.s[k as usize]
    }

    /// Encrypt data using ARC4.
    ///
    /// Note: ARC4 encryption and decryption are the same operation (XOR with keystream).
    ///
    /// # Arguments
    ///
    /// * `data` - The data to encrypt
    ///
    /// # Returns
    ///
    /// Encrypted data as a `Vec<u8>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cascette_crypto::arc4::Arc4Cipher;
    ///
    /// let key = b"secret";
    /// let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
    /// let encrypted = cipher.encrypt(b"Hello!");
    /// ```
    pub fn encrypt(&mut self, data: &[u8]) -> Vec<u8> {
        data.iter()
            .map(|&byte| byte ^ self.next_keystream_byte())
            .collect()
    }

    /// Decrypt data using ARC4.
    ///
    /// Note: ARC4 decryption is identical to encryption (XOR with keystream).
    /// You must create a new cipher instance with the same key for decryption.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to decrypt
    ///
    /// # Returns
    ///
    /// Decrypted data as a `Vec<u8>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cascette_crypto::arc4::Arc4Cipher;
    ///
    /// let key = b"secret";
    ///
    /// // Encrypt
    /// let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
    /// let encrypted = cipher.encrypt(b"Hello!");
    ///
    /// // Decrypt (need fresh cipher with same key)
    /// let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
    /// let decrypted = cipher.decrypt(&encrypted);
    /// assert_eq!(b"Hello!", &decrypted[..]);
    /// ```
    pub fn decrypt(&mut self, data: &[u8]) -> Vec<u8> {
        // ARC4 decryption is identical to encryption
        self.encrypt(data)
    }

    /// Apply ARC4 keystream to data in-place.
    ///
    /// This is more memory-efficient for large data since it doesn't allocate
    /// a new Vec. Can be used for both encryption and decryption.
    ///
    /// # Arguments
    ///
    /// * `data` - Mutable slice to encrypt/decrypt in-place
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cascette_crypto::arc4::Arc4Cipher;
    ///
    /// let key = b"secret";
    /// let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
    ///
    /// let mut data = b"Hello, World!".to_vec();
    /// cipher.apply_keystream(&mut data);
    /// // data is now encrypted
    /// ```
    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        for byte in data {
            *byte ^= self.next_keystream_byte();
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_arc4_round_trip() {
        let key = b"test_key";
        let plaintext = b"Hello, ARC4 World!";

        // Encrypt
        let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        let ciphertext = cipher.encrypt(plaintext);

        // Decrypt with fresh cipher
        let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        let decrypted = cipher.decrypt(&ciphertext);

        assert_eq!(plaintext, &decrypted[..]);
        assert_ne!(plaintext, &ciphertext[..]);
    }

    #[test]
    fn test_arc4_known_vector() {
        // Test vector from RC4 specification
        let key = b"Key";
        let plaintext = b"Plaintext";
        let expected = [0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3];

        let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        let ciphertext = cipher.encrypt(plaintext);

        assert_eq!(expected, &ciphertext[..]);
    }

    #[test]
    fn test_arc4_empty_data() {
        let key = b"key";
        let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");

        let result = cipher.encrypt(b"");
        assert!(result.is_empty());
    }

    #[test]
    fn test_arc4_in_place() {
        let key = b"test_key";
        let original = b"Hello, World!";

        let mut data = original.to_vec();
        let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        cipher.apply_keystream(&mut data);

        // Should be encrypted (different from original)
        assert_ne!(original, &data[..]);

        // Decrypt in place with fresh cipher
        let mut cipher = Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        cipher.apply_keystream(&mut data);

        // Should match original
        assert_eq!(original, &data[..]);
    }

    #[test]
    fn test_arc4_different_keys() {
        let plaintext = b"Same plaintext";

        let mut cipher1 =
            Arc4Cipher::new(b"key1").expect("ARC4 cipher creation should succeed in test");
        let ciphertext1 = cipher1.encrypt(plaintext);

        let mut cipher2 =
            Arc4Cipher::new(b"key2").expect("ARC4 cipher creation should succeed in test");
        let ciphertext2 = cipher2.encrypt(plaintext);

        assert_ne!(ciphertext1, ciphertext2);
    }

    #[test]
    fn test_arc4_invalid_key_length() {
        // Empty key
        assert!(Arc4Cipher::new(b"").is_err());

        // Key too long (>256 bytes)
        let long_key = vec![0u8; 257];
        assert!(Arc4Cipher::new(&long_key).is_err());

        // Valid lengths should work
        assert!(Arc4Cipher::new(b"a").is_ok());
        assert!(Arc4Cipher::new(&vec![0u8; 256]).is_ok());
    }

    #[test]
    fn test_arc4_deterministic() {
        let key = b"consistent_key";
        let plaintext = b"Consistent data";

        // Two separate encryptions with same key should produce same result
        let mut cipher1 =
            Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        let result1 = cipher1.encrypt(plaintext);

        let mut cipher2 =
            Arc4Cipher::new(key).expect("ARC4 cipher creation should succeed in test");
        let result2 = cipher2.encrypt(plaintext);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_arc4_casc_compatibility() {
        // Test with typical CASC key size (16 bytes) and TACT key format
        let key = [0x42; 16]; // Typical 16-byte TACT key
        let data = b"CASC encrypted data block";

        let mut cipher =
            Arc4Cipher::new(&key).expect("ARC4 cipher creation should succeed in test");
        let encrypted = cipher.encrypt(data);

        let mut cipher =
            Arc4Cipher::new(&key).expect("ARC4 cipher creation should succeed in test");
        let decrypted = cipher.decrypt(&encrypted);

        assert_eq!(data, &decrypted[..]);
    }
}

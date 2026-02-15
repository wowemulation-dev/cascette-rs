//! Salsa20 stream cipher implementation for CASC/BLTE encryption
//!
//! This implements the specific Salsa20 variant used by CASC:
//! - Uses 16-byte keys with "expand 16-byte k" constant
//! - 4-byte IV extended to 8 bytes by zero-padding
//! - Block index `XORed` with first 4 bytes of IV

use crate::error::CryptoError;

/// Salsa20 cipher for CASC encryption
pub struct Salsa20Cipher {
    state: [u32; 16],
    keystream: [u8; 64],
    keystream_pos: usize,
}

impl Salsa20Cipher {
    /// Create a new Salsa20 cipher for CASC encryption
    ///
    /// # Arguments
    /// * `key` - 16-byte encryption key
    /// * `iv` - 4-byte initialization vector
    /// * `block_index` - Block index for multi-block encryption
    pub fn new(key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Self, CryptoError> {
        if iv.len() != 4 {
            return Err(CryptoError::InvalidIvSize {
                expected: 4,
                actual: iv.len(),
            });
        }

        // Initialize state with Salsa20 constants for 16-byte keys
        // "expand 16-byte k" = 0x61707865, 0x3120646e, 0x79622d36, 0x6b206574
        let mut state = [0u32; 16];

        // Constants for 16-byte key (tau)
        state[0] = 0x6170_7865; // "expa"
        state[5] = 0x3120_646e; // "nd 1"
        state[10] = 0x7962_2d36; // "6-by"
        state[15] = 0x6b20_6574; // "te k"

        // Key (16 bytes used twice)
        state[1] = u32::from_le_bytes([key[0], key[1], key[2], key[3]]);
        state[2] = u32::from_le_bytes([key[4], key[5], key[6], key[7]]);
        state[3] = u32::from_le_bytes([key[8], key[9], key[10], key[11]]);
        state[4] = u32::from_le_bytes([key[12], key[13], key[14], key[15]]);

        // Repeat key for positions 11-14
        state[11] = state[1];
        state[12] = state[2];
        state[13] = state[3];
        state[14] = state[4];

        // IV with block index XOR
        let mut extended_iv = [0u8; 8];
        extended_iv[..4].copy_from_slice(iv);

        // XOR block index with first 4 bytes
        #[allow(clippy::cast_possible_truncation)]
        let block_bytes = (block_index as u32).to_le_bytes();
        for i in 0..4 {
            extended_iv[i] ^= block_bytes[i];
        }

        // Nonce
        state[6] = u32::from_le_bytes([
            extended_iv[0],
            extended_iv[1],
            extended_iv[2],
            extended_iv[3],
        ]);
        state[7] = u32::from_le_bytes([
            extended_iv[4],
            extended_iv[5],
            extended_iv[6],
            extended_iv[7],
        ]);

        // Counter (starts at 0)
        state[8] = 0;
        state[9] = 0;

        let mut cipher = Self {
            state,
            keystream: [0; 64],
            keystream_pos: 64, // Force generation on first use
        };

        cipher.generate_keystream();
        Ok(cipher)
    }

    /// Generate next block of keystream
    fn generate_keystream(&mut self) {
        let mut working_state = self.state;

        // Perform 20 rounds (10 double-rounds)
        for _ in 0..10 {
            // Column round
            Self::quarter_round(&mut working_state, 0, 4, 8, 12);
            Self::quarter_round(&mut working_state, 5, 9, 13, 1);
            Self::quarter_round(&mut working_state, 10, 14, 2, 6);
            Self::quarter_round(&mut working_state, 15, 3, 7, 11);

            // Diagonal round
            Self::quarter_round(&mut working_state, 0, 1, 2, 3);
            Self::quarter_round(&mut working_state, 5, 6, 7, 4);
            Self::quarter_round(&mut working_state, 10, 11, 8, 9);
            Self::quarter_round(&mut working_state, 15, 12, 13, 14);
        }

        // Add initial state to working state
        for (i, working) in working_state.iter_mut().enumerate() {
            *working = working.wrapping_add(self.state[i]);
        }

        // Convert to bytes
        for (i, chunk) in working_state.iter().enumerate() {
            let bytes = chunk.to_le_bytes();
            self.keystream[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
        }

        // Increment counter
        self.state[8] = self.state[8].wrapping_add(1);
        if self.state[8] == 0 {
            self.state[9] = self.state[9].wrapping_add(1);
        }

        self.keystream_pos = 0;
    }

    /// Salsa20 quarter round function
    fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        state[b] ^= state[a].wrapping_add(state[d]).rotate_left(7);
        state[c] ^= state[b].wrapping_add(state[a]).rotate_left(9);
        state[d] ^= state[c].wrapping_add(state[b]).rotate_left(13);
        state[a] ^= state[d].wrapping_add(state[c]).rotate_left(18);
    }

    /// Apply keystream to data (encrypt or decrypt)
    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            if self.keystream_pos >= 64 {
                self.generate_keystream();
            }
            *byte ^= self.keystream[self.keystream_pos];
            self.keystream_pos += 1;
        }
    }
}

/// Decrypt data using CASC Salsa20 variant
pub fn decrypt_salsa20(
    data: &[u8],
    key: &[u8; 16],
    iv: &[u8],
    block_index: usize,
) -> Result<Vec<u8>, CryptoError> {
    let mut cipher = Salsa20Cipher::new(key, iv, block_index)?;
    let mut output = data.to_vec();
    cipher.apply_keystream(&mut output);
    Ok(output)
}

/// Encrypt data using CASC Salsa20 variant (same as decrypt for stream ciphers)
pub fn encrypt_salsa20(
    data: &[u8],
    key: &[u8; 16],
    iv: &[u8],
    block_index: usize,
) -> Result<Vec<u8>, CryptoError> {
    decrypt_salsa20(data, key, iv, block_index)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_salsa20_round_trip() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Hello, World! This is a test message.";
        let block_index = 0;

        // Encrypt
        let ciphertext =
            encrypt_salsa20(plaintext, &key, &iv, block_index).expect("Operation should succeed");
        assert_ne!(&ciphertext[..], plaintext);

        // Decrypt
        let decrypted =
            decrypt_salsa20(&ciphertext, &key, &iv, block_index).expect("Operation should succeed");
        assert_eq!(&decrypted[..], plaintext);
    }

    #[test]
    fn test_salsa20_block_index() {
        let key = [0x42u8; 16];
        let iv = [0x11, 0x22, 0x33, 0x44];
        let plaintext = b"Test data";

        // Different block indices should produce different ciphertexts
        let cipher1 = encrypt_salsa20(plaintext, &key, &iv, 0).expect("Operation should succeed");
        let cipher2 = encrypt_salsa20(plaintext, &key, &iv, 1).expect("Operation should succeed");
        assert_ne!(cipher1, cipher2);
    }

    #[test]
    fn test_salsa20_invalid_iv() {
        let key = [0x01u8; 16];
        let invalid_iv = [0x02, 0x03]; // Too short
        let plaintext = b"Test";

        let result = encrypt_salsa20(plaintext, &key, &invalid_iv, 0);
        assert!(matches!(result, Err(CryptoError::InvalidIvSize { .. })));
    }
}

//! BLTE encryption support
//!
//! Uses expect in binrw map functions where Result types cannot be used.
#![allow(clippy::expect_used)]

use binrw::{BinRead, BinWrite};

/// Encryption type for BLTE chunks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncryptionType {
    /// `Salsa20` stream cipher
    Salsa20 = b'S',
    /// `ARC4` stream cipher
    Arc4 = b'A',
}

impl EncryptionType {
    /// Parse from byte
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'S' => Some(Self::Salsa20),
            b'A' => Some(Self::Arc4),
            _ => None,
        }
    }

    /// Get byte representation
    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

/// Encrypted chunk header
#[derive(Debug, Clone, BinRead, BinWrite)]
pub struct EncryptedHeader {
    /// Key name size (usually 8)
    pub key_name_size: u8,

    /// Key name/identifier (64-bit)
    #[br(count = key_name_size)]
    pub key_name: Vec<u8>,

    /// IV size (usually 4)
    pub iv_size: u8,

    /// Initialization vector
    #[br(count = iv_size)]
    pub iv: Vec<u8>,

    /// Encryption type
    #[br(map = |x: u8| EncryptionType::from_byte(x).expect("valid encryption type byte"))]
    #[bw(map = |x: &EncryptionType| x.as_byte())]
    pub encryption_type: EncryptionType,
}

impl EncryptedHeader {
    /// Get the key identifier as u64
    #[allow(clippy::expect_used)] // Length is checked before try_into
    pub fn key_id(&self) -> u64 {
        if self.key_name.len() == 8 {
            u64::from_le_bytes(
                self.key_name[..8]
                    .try_into()
                    .expect("key_name should be exactly 8 bytes when len == 8"),
            )
        } else {
            0
        }
    }

    /// Modify IV for chunk index (XOR with chunk index)
    #[allow(clippy::cast_possible_truncation)]
    pub fn modify_iv_for_chunk(&mut self, chunk_index: usize) {
        for i in 0..self.iv.len().min(4) {
            // Safe cast: we're masking to get only the bottom 8 bits
            self.iv[i] ^= ((chunk_index >> (i * 8)) & 0xFF) as u8;
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_type_conversion() {
        assert_eq!(
            EncryptionType::from_byte(b'S'),
            Some(EncryptionType::Salsa20)
        );
        assert_eq!(EncryptionType::from_byte(b'A'), Some(EncryptionType::Arc4));
        assert_eq!(EncryptionType::from_byte(b'X'), None);

        assert_eq!(EncryptionType::Salsa20.as_byte(), b'S');
        assert_eq!(EncryptionType::Arc4.as_byte(), b'A');
    }

    #[test]
    fn test_iv_modification() {
        let mut header = EncryptedHeader {
            key_name_size: 8,
            key_name: vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
            iv_size: 4,
            iv: vec![0x00, 0x00, 0x00, 0x00],
            encryption_type: EncryptionType::Salsa20,
        };

        header.modify_iv_for_chunk(0x1234_5678);
        assert_eq!(header.iv, vec![0x78, 0x56, 0x34, 0x12]);
    }
}

//! MD5 hashing for content and encoding keys

use binrw::{BinRead, BinWrite};
use md5::{Digest, Md5};
#[cfg(feature = "file-store")]
use serde::{Deserialize, Serialize};
use std::fmt;

/// Content key (MD5 hash) used to identify content
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "file-store", derive(Serialize, Deserialize))]
pub struct ContentKey([u8; 16]);

impl ContentKey {
    /// Create content key from raw bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Create content key from data by computing MD5 hash
    pub fn from_data(data: &[u8]) -> Self {
        let mut hasher = Md5::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Parse content key from hex string
    pub fn from_hex(hex: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(hex, &mut bytes)?;
        Ok(Self(bytes))
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for ContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Encoding key (MD5 hash) used to identify encoded content
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncodingKey([u8; 16]);

impl EncodingKey {
    /// Create encoding key from raw bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Create encoding key from data by computing MD5 hash
    pub fn from_data(data: &[u8]) -> Self {
        let mut hasher = Md5::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Parse encoding key from hex string
    pub fn from_hex(hex: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(hex, &mut bytes)?;
        Ok(Self(bytes))
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Get first 9 bytes for archive path construction
    pub fn first_9(&self) -> [u8; 9] {
        let mut result = [0u8; 9];
        result.copy_from_slice(&self.0[..9]);
        result
    }
}

impl fmt::Display for EncodingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// File data ID used to identify files in CASC
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[brw(little)] // FileDataIDs are typically little-endian in file structures
pub struct FileDataId(pub u32);

impl FileDataId {
    /// Create a new `FileDataId`
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw ID value
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for FileDataId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for FileDataId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<FileDataId> for u32 {
    fn from(fdid: FileDataId) -> Self {
        fdid.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_key_from_data() {
        let data = b"Hello, World!";
        let key = ContentKey::from_data(data);
        assert_eq!(key.to_hex(), "65a8e27d8879283831b664bd8b7f0ad4");
    }

    #[test]
    fn test_content_key_from_hex() {
        let hex = "65a8e27d8879283831b664bd8b7f0ad4";
        let key = ContentKey::from_hex(hex).expect("Valid hex string for ContentKey");
        assert_eq!(key.to_hex(), hex);
    }

    #[test]
    fn test_encoding_key_first_9() {
        let key = EncodingKey::from_bytes([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ]);
        let first_9 = key.first_9();
        assert_eq!(
            first_9,
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]
        );
    }

    #[test]
    fn test_round_trip() {
        let original = ContentKey::from_bytes([
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]);
        let hex = original.to_hex();
        let restored =
            ContentKey::from_hex(&hex).expect("Valid hex string for ContentKey restoration");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_file_data_id() {
        let fdid = FileDataId::new(12345);
        assert_eq!(fdid.get(), 12345);
        assert_eq!(format!("{fdid}"), "12345");

        let from_u32: FileDataId = 67890u32.into();
        assert_eq!(from_u32.get(), 67890);

        let to_u32: u32 = from_u32.into();
        assert_eq!(to_u32, 67890);
    }

    #[test]
    fn test_file_data_id_ordering() {
        let fdid1 = FileDataId::new(100);
        let fdid2 = FileDataId::new(200);

        assert!(fdid1 < fdid2);
        assert!(fdid2 > fdid1);
        assert_eq!(fdid1, fdid1);
    }

    #[test]
    fn test_file_data_id_binrw() {
        use binrw::io::Cursor;

        let original = FileDataId::new(0x1234_5678);

        // Test serialization
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_le(&mut cursor)
            .expect("Write to in-memory buffer should succeed");

        // Should be little-endian: 78 56 34 12
        assert_eq!(buffer, vec![0x78, 0x56, 0x34, 0x12]);

        // Test deserialization
        let mut cursor = Cursor::new(&buffer);
        let deserialized: FileDataId =
            FileDataId::read_le(&mut cursor).expect("Read from in-memory buffer should succeed");

        assert_eq!(original, deserialized);
    }
}

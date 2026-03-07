//! 30-byte local BLTE entry header.
//!
//! CASC writes a 30-byte header before each BLTE blob in archive
//! `.data` files. The encoding key is stored with bytes reversed.
//! Without this header, data written by cascette-rs is unreadable by
//! the official client.
//!
//! Layout (30 bytes total):
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0x00   | 16   | Encoding key (reversed byte order) |
//! | 0x10   | 4    | Encoded size of BLTE data, LE (matches IDX `encoded_size`) |
//! | 0x14   | 2    | Flags |
//! | 0x16   | 4    | ChecksumA (Jenkins hash of bytes 0..22) |
//! | 0x1A   | 4    | ChecksumB (XOR accumulation of bytes 0..26) |

use cascette_crypto::jenkins::hashlittle;

/// Size of the local header in bytes.
pub const LOCAL_HEADER_SIZE: usize = 0x1E; // 30 bytes

/// Jenkins hash seed for computing header checksum_a.
const CHECKSUM_A_SEED: u32 = 0x3D6B_E971;

/// 30-byte local header preceding each BLTE entry in `.data` archives.
#[derive(Debug, Clone)]
pub struct LocalHeader {
    /// Encoding key (16 bytes, reversed byte order).
    pub encoding_key: [u8; 16],
    /// Encoded size of the BLTE data (little-endian on disk).
    /// Matches the `encoded_size` field in the IDX entry.
    pub encoded_size: u32,
    /// Flags (2 bytes).
    pub flags: u16,
    /// Checksum A (4 bytes).
    pub checksum_a: u32,
    /// Checksum B (4 bytes).
    pub checksum_b: u32,
}

impl LocalHeader {
    /// Create a new local header for BLTE data.
    ///
    /// Create a new local header for BLTE data:
    /// - Only the first 9 bytes of the encoding key are reversed and stored;
    ///   the remaining 7 bytes are zero-padded.
    /// - `encoded_size` = BLTE data size (LE on disk).
    /// - `status` = 1 (normal entry).
    /// - `jenkins_hash` and `xor_checksum` are both 0 during initial write.
    ///   They are computed later during header reconstruction.
    pub fn new(encoding_key: [u8; 16], blte_size: u32, _base_offset: usize) -> Self {
        // Reverse only the first 9 bytes; zero-pad the rest.
        // Reference: dynamic.cpp:160-164
        let mut reversed_key = [0u8; 16];
        for i in 0..9 {
            reversed_key[i] = encoding_key[8 - i];
        }
        // bytes 9..16 stay zero

        Self {
            encoding_key: reversed_key,
            encoded_size: blte_size,
            flags: 1, // status = 1 (normal entry)
            checksum_a: 0, // computed during RepairReconstructionHeaders
            checksum_b: 0, // computed during RepairReconstructionHeaders
        }
    }

    /// Compute checksum_a: Jenkins hash of the first 22 bytes with seed 0x3D6BE971.
    pub fn compute_checksum_a(header_bytes: &[u8; LOCAL_HEADER_SIZE]) -> u32 {
        hashlittle(&header_bytes[..0x16], CHECKSUM_A_SEED)
    }

    /// Compute checksum_b: XOR accumulation of the first 26 bytes.
    ///
    /// Uses rotating 4-byte index `(base_offset + i) & 3` to distribute
    /// bytes across the 4-byte checksum.
    pub fn compute_checksum_b(header_bytes: &[u8; LOCAL_HEADER_SIZE], base_offset: usize) -> u32 {
        let mut checksum = [0u8; 4];
        for (i, &byte) in header_bytes[..0x1A].iter().enumerate() {
            checksum[(base_offset + i) & 3] ^= byte;
        }
        u32::from_le_bytes(checksum)
    }

    /// Validate both checksums against the header contents.
    pub fn validate_checksums(&self, base_offset: usize) -> bool {
        let bytes = self.to_bytes();
        let expected_a = Self::compute_checksum_a(&bytes);
        let expected_b = Self::compute_checksum_b(&bytes, base_offset);
        self.checksum_a == expected_a && self.checksum_b == expected_b
    }

    /// Serialize the header to 30 bytes.
    pub fn to_bytes(&self) -> [u8; LOCAL_HEADER_SIZE] {
        let mut buf = [0u8; LOCAL_HEADER_SIZE];

        // Encoding key (16 bytes, already reversed in constructor)
        buf[0x00..0x10].copy_from_slice(&self.encoding_key);

        // Encoded size (4 bytes, little-endian)
        buf[0x10..0x14].copy_from_slice(&self.encoded_size.to_le_bytes());

        // Flags (2 bytes)
        buf[0x14..0x16].copy_from_slice(&self.flags.to_le_bytes());

        // ChecksumA (4 bytes)
        buf[0x16..0x1A].copy_from_slice(&self.checksum_a.to_le_bytes());

        // ChecksumB (4 bytes)
        buf[0x1A..0x1E].copy_from_slice(&self.checksum_b.to_le_bytes());

        buf
    }

    /// Parse a local header from 30 bytes.
    ///
    /// Returns `None` if the slice is too short.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < LOCAL_HEADER_SIZE {
            return None;
        }

        let mut encoding_key = [0u8; 16];
        encoding_key.copy_from_slice(&data[0x00..0x10]);

        let encoded_size = u32::from_le_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]);
        let flags = u16::from_le_bytes([data[0x14], data[0x15]]);
        let checksum_a = u32::from_le_bytes([data[0x16], data[0x17], data[0x18], data[0x19]]);
        let checksum_b = u32::from_le_bytes([data[0x1A], data[0x1B], data[0x1C], data[0x1D]]);

        Some(Self {
            encoding_key,
            encoded_size,
            flags,
            checksum_a,
            checksum_b,
        })
    }

    /// Get the original encoding key (first 9 bytes un-reversed).
    ///
    /// Only the first 9 bytes are meaningful; bytes 9-15 are zero.
    pub fn original_encoding_key(&self) -> [u8; 16] {
        let mut key = [0u8; 16];
        for i in 0..9 {
            key[i] = self.encoding_key[8 - i];
        }
        key
    }

    /// Get the BLTE data size (same as encoded_size).
    pub const fn blte_size(&self) -> u32 {
        self.encoded_size
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_local_header_round_trip() {
        let key = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        let blte_size = 1234;

        let header = LocalHeader::new(key, blte_size, 0);

        // First 9 bytes of key reversed, rest zero-padded
        assert_eq!(header.encoding_key[0], 0x09); // key[8] reversed to [0]
        assert_eq!(header.encoding_key[8], 0x01); // key[0] reversed to [8]
        assert_eq!(header.encoding_key[9], 0x00); // zero-padded
        assert_eq!(header.encoding_key[15], 0x00);

        // Size is the BLTE data size
        assert_eq!(header.encoded_size, blte_size);

        // Status/flags should be 1 (normal entry)
        assert_eq!(header.flags, 1);

        // Checksums are 0 during initial write (computed later by repair)
        assert_eq!(header.checksum_a, 0);
        assert_eq!(header.checksum_b, 0);

        // Round-trip through bytes
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), LOCAL_HEADER_SIZE);

        let parsed = LocalHeader::from_bytes(&bytes).expect("parse");
        assert_eq!(parsed.encoding_key, header.encoding_key);
        assert_eq!(parsed.encoded_size, header.encoded_size);
        assert_eq!(parsed.flags, header.flags);
        assert_eq!(parsed.checksum_a, header.checksum_a);
        assert_eq!(parsed.checksum_b, header.checksum_b);
    }

    #[test]
    fn test_original_key_recovery() {
        let key = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];

        let header = LocalHeader::new(key, 100, 0);
        let recovered = header.original_encoding_key();
        // Only first 9 bytes are recoverable
        assert_eq!(&recovered[..9], &key[..9]);
        // Rest is zero (original bytes 9-15 are lost)
        assert_eq!(&recovered[9..], &[0u8; 7]);
    }

    #[test]
    fn test_blte_size() {
        let header = LocalHeader::new([0u8; 16], 500, 0);
        assert_eq!(header.blte_size(), 500);
    }

    #[test]
    fn test_too_short_data_rejected() {
        let short = [0u8; 20];
        assert!(LocalHeader::from_bytes(&short).is_none());
    }
}

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
//! | 0x10   | 4    | Size including this 30-byte header (BE) |
//! | 0x14   | 2    | Flags |
//! | 0x16   | 4    | ChecksumA |
//! | 0x1A   | 4    | ChecksumB |

/// Size of the local header in bytes.
pub const LOCAL_HEADER_SIZE: usize = 0x1E; // 30 bytes

/// 30-byte local header preceding each BLTE entry in `.data` archives.
#[derive(Debug, Clone)]
pub struct LocalHeader {
    /// Encoding key (16 bytes, reversed byte order).
    pub encoding_key: [u8; 16],
    /// Total size including this 30-byte header (big-endian on disk).
    pub size_with_header: u32,
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
    /// `encoding_key` is the MD5 of the BLTE-encoded data.
    /// `blte_size` is the size of the BLTE data (without header).
    pub fn new(encoding_key: [u8; 16], blte_size: u32) -> Self {
        // Reverse the encoding key for on-disk storage
        let mut reversed_key = encoding_key;
        reversed_key.reverse();

        Self {
            encoding_key: reversed_key,
            size_with_header: blte_size + LOCAL_HEADER_SIZE as u32,
            flags: 0,
            checksum_a: 0,
            checksum_b: 0,
        }
    }

    /// Serialize the header to 30 bytes.
    pub fn to_bytes(&self) -> [u8; LOCAL_HEADER_SIZE] {
        let mut buf = [0u8; LOCAL_HEADER_SIZE];

        // Encoding key (16 bytes, already reversed in constructor)
        buf[0x00..0x10].copy_from_slice(&self.encoding_key);

        // Size including header (4 bytes, big-endian)
        buf[0x10..0x14].copy_from_slice(&self.size_with_header.to_be_bytes());

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

        let size_with_header = u32::from_be_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]);
        let flags = u16::from_le_bytes([data[0x14], data[0x15]]);
        let checksum_a = u32::from_le_bytes([data[0x16], data[0x17], data[0x18], data[0x19]]);
        let checksum_b = u32::from_le_bytes([data[0x1A], data[0x1B], data[0x1C], data[0x1D]]);

        Some(Self {
            encoding_key,
            size_with_header,
            flags,
            checksum_a,
            checksum_b,
        })
    }

    /// Get the original (non-reversed) encoding key.
    pub fn original_encoding_key(&self) -> [u8; 16] {
        let mut key = self.encoding_key;
        key.reverse();
        key
    }

    /// Get the BLTE data size (total size minus header).
    pub const fn blte_size(&self) -> u32 {
        self.size_with_header - LOCAL_HEADER_SIZE as u32
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

        let header = LocalHeader::new(key, blte_size);

        // Key should be reversed
        assert_eq!(header.encoding_key[0], 0x10);
        assert_eq!(header.encoding_key[15], 0x01);

        // Size includes header
        assert_eq!(
            header.size_with_header,
            blte_size + LOCAL_HEADER_SIZE as u32
        );

        // Round-trip through bytes
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), LOCAL_HEADER_SIZE);

        let parsed = LocalHeader::from_bytes(&bytes).expect("parse");
        assert_eq!(parsed.encoding_key, header.encoding_key);
        assert_eq!(parsed.size_with_header, header.size_with_header);
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

        let header = LocalHeader::new(key, 100);
        assert_eq!(header.original_encoding_key(), key);
    }

    #[test]
    fn test_blte_size() {
        let header = LocalHeader::new([0u8; 16], 500);
        assert_eq!(header.blte_size(), 500);
    }

    #[test]
    fn test_too_short_data_rejected() {
        let short = [0u8; 20];
        assert!(LocalHeader::from_bytes(&short).is_none());
    }
}

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

/// Lookup table for checksum_b phase 2 XOR mask.
/// Extracted from WoW.exe 1.13.2.31650 at address 0x14217D690.
/// 16 entries indexed by `(global_offset + 0x1E) & 0xF`.
const CHECKSUM_B_LUT: [u32; 16] = [
    0x0493_96B8,
    0x72A8_2A9B,
    0xEE62_6CCA,
    0x9917_754F,
    0x15DE_40B1,
    0xF5A8_A9B6,
    0x421E_AC7E,
    0xA9D5_5C9A,
    0x317F_D40C,
    0x04FA_F80D,
    0x3D6B_E971,
    0x5293_3CFD,
    0x27F6_4B7D,
    0xC6F5_C11B,
    0xD575_7E3A,
    0x6C38_8745,
];

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
    /// - The full 16-byte encoding key is reversed and stored, matching
    ///   Agent.exe's `BuildLocalFileHeader` writes (verified byte-identical
    ///   against a client-written entry: key field = full key reversed).
    /// - `encoded_size` = total entry size including this 30-byte header
    ///   (i.e., `LOCAL_HEADER_SIZE + blte_payload_size`). Matches what the
    ///   Blizzard agent stores in both the local header and the IDX entry.
    /// - `flags` = 0 for normal data entries, 1 for segment reconstruction
    ///   headers. Verified against a client-written 1.13.2.31650 store:
    ///   614/632 data-entry headers carry flags=0, while the 16 segment
    ///   reconstruction headers carry flags=1.
    /// - `global_offset` is the byte position of this header across all
    ///   data files; used for both checksum_a and checksum_b computation.
    pub fn new(
        encoding_key: [u8; 16],
        encoded_size: u32,
        global_offset: usize,
        flags: u16,
    ) -> Self {
        // Reverse the full 16 bytes, matching Agent.exe writes. The client
        // stores the complete reversed key (not a 9-byte truncated form);
        // cascette-rs previously stored only 9 bytes reversed + zero pad,
        // which produced headers that differ from the agent's.
        let mut reversed_key = [0u8; 16];
        for i in 0..16 {
            reversed_key[i] = encoding_key[15 - i];
        }
        // bytes 9..16 stay zero

        let mut header = Self {
            encoding_key: reversed_key,
            encoded_size,
            flags,
            checksum_a: 0,
            checksum_b: 0,
        };

        // Compute and set both checksums.
        // checksum_a must be computed first (it occupies bytes 0x16-0x19
        // which are read during checksum_b accumulation).
        let mut bytes = header.to_bytes();
        let checksum_a = Self::compute_checksum_a(&bytes);
        bytes[0x16..0x1A].copy_from_slice(&checksum_a.to_le_bytes());
        header.checksum_a = checksum_a;

        let checksum_b = Self::compute_checksum_b(&bytes, global_offset);
        header.checksum_b = checksum_b;

        header
    }

    /// Compute checksum_a: Jenkins hash of the first 22 bytes with seed 0x3D6BE971.
    pub fn compute_checksum_a(header_bytes: &[u8; LOCAL_HEADER_SIZE]) -> u32 {
        hashlittle(&header_bytes[..0x16], CHECKSUM_A_SEED)
    }

    /// Compute checksum_b for the local file header.
    ///
    /// Two-phase algorithm matching the binary at `BuildLocalFileHeader`
    /// (0x140cccdc0):
    ///
    /// 1. XOR-accumulate bytes 0..26 into a 4-byte register, rotating
    ///    by `(global_offset + i) & 3`.
    /// 2. Derive a 4-byte XOR mask from `CHECKSUM_B_LUT` and the
    ///    global offset, then XOR the accumulator with it.
    ///
    /// `header_bytes` must have checksum_a already written at 0x16-0x19.
    /// `global_offset` is the entry's position across all data files.
    pub fn compute_checksum_b(header_bytes: &[u8; LOCAL_HEADER_SIZE], global_offset: usize) -> u32 {
        // Phase 1: XOR accumulate bytes 0..26
        let mut accum = [0u8; 4];
        for (i, &byte) in header_bytes[..0x1A].iter().enumerate() {
            accum[(global_offset + i) & 3] ^= byte;
        }

        // Phase 2: derive XOR mask from LUT
        // mask = LUT[(global_offset + 0x1E) & 0xF] ^ (global_offset + 0x1E)
        let offset_plus_header = global_offset.wrapping_add(LOCAL_HEADER_SIZE);
        let lut_index = offset_plus_header & 0xF;
        let mask_u32 = CHECKSUM_B_LUT[lut_index] ^ (offset_plus_header as u32);
        let mask = mask_u32.to_le_bytes();

        // XOR accumulator with mask, using the same rotation
        let mut result = [0u8; 4];
        for i in 0..4u8 {
            let j = (global_offset.wrapping_add(0x1A).wrapping_add(i as usize)) & 3;
            result[i as usize] = accum[j] ^ mask[j];
        }

        u32::from_le_bytes(result)
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

    /// Get the original encoding key (full 16 bytes un-reversed).
    ///
    /// Recovers the complete key the header was built from. The first
    /// 9 bytes are the truncated EKey stored in the IDX; bytes 9-15 are
    /// the remainder of the full 16-byte key.
    pub fn original_encoding_key(&self) -> [u8; 16] {
        let mut key = [0u8; 16];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = self.encoding_key[15 - i];
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

        let header = LocalHeader::new(key, blte_size, 0, 0);

        // Full 16-byte key reversed (byte-identical to Agent.exe writes)
        assert_eq!(header.encoding_key[0], 0x10); // key[15] reversed to [0]
        assert_eq!(header.encoding_key[8], 0x08); // key[7] reversed to [8]
        assert_eq!(header.encoding_key[15], 0x01); // key[0] reversed to [15]

        // Size is the BLTE data size
        assert_eq!(header.encoded_size, blte_size);

        // Flags default to 0 for normal data entries
        assert_eq!(header.flags, 0);

        // Checksums are computed during construction
        assert_ne!(header.checksum_a, 0, "checksum_a must be non-zero");
        // checksum_b can be zero for specific offsets, so just verify round-trip
        assert!(header.validate_checksums(0), "checksums must validate");

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

        let header = LocalHeader::new(key, 100, 0, 0);
        let recovered = header.original_encoding_key();
        // Full key recoverable (byte-identical to Agent.exe writes)
        assert_eq!(&recovered[..], &key[..]);
    }

    #[test]
    fn test_blte_size() {
        let header = LocalHeader::new([0u8; 16], 500, 0, 0);
        assert_eq!(header.blte_size(), 500);
    }

    #[test]
    fn test_too_short_data_rejected() {
        let short = [0u8; 20];
        assert!(LocalHeader::from_bytes(&short).is_none());
    }
}

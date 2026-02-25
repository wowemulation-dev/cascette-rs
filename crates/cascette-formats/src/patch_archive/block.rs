//! Block-based data types for Patch Archive (PA) format
//!
//! The PA format uses a block-based structure where file entries are grouped
//! into blocks. Each block has metadata in the block table (last CKey, MD5
//! hash, offset) and contains variable-length file entries with patches.
//!
//! Binary layout:
//! - Block table entry: last_file_ckey + block_md5(16) + block_offset(u32 BE)
//! - File entry: num_patches(u8) + target_ckey + decoded_size(uint40 BE)
//!   + N patches
//! - Patch: source_ekey + source_decoded_size(uint40 BE) + patch_ekey
//!   + patch_size(u32 BE) + patch_index(u8)
//! - End of block: 0x00 sentinel byte

use std::io::{self, Read, Write};

/// Encoding information from extended header (when flags bit 1 is set)
///
/// Contains metadata about the encoding file for this patch manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchArchiveEncodingInfo {
    /// Content key of the encoding file
    pub encoding_ckey: [u8; 16],
    /// Encoding key of the encoding file
    pub encoding_ekey: [u8; 16],
    /// Decoded (decompressed) size of the encoding file
    pub decoded_size: u32,
    /// Encoded (compressed) size of the encoding file
    pub encoded_size: u32,
    /// ESpec compression specification string
    pub espec: String,
}

/// Block table entry with metadata and parsed file entries
///
/// Each block in the block table has:
/// - `last_file_ckey`: Last (highest) file CKey in this block, enabling
///   binary search across blocks
/// - `block_md5`: MD5 hash of the block data for integrity verification
/// - `block_offset`: Absolute byte offset where block data starts in the file
/// - `file_entries`: Parsed file entries within this block
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchBlock {
    /// Last (highest) file CKey in this block, for binary search
    pub last_file_ckey: [u8; 16],
    /// MD5 hash of block data
    pub block_md5: [u8; 16],
    /// Absolute byte offset of block data in the file
    pub block_offset: u32,
    /// Parsed file entries within this block
    pub file_entries: Vec<PatchFileEntry>,
}

/// A file entry within a block, containing one or more patches
///
/// Each file entry represents a target file that can be patched from one
/// or more source versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchFileEntry {
    /// Content key of the target (new) file
    pub target_ckey: [u8; 16],
    /// Decoded size of the target file (stored as uint40 big-endian)
    pub decoded_size: u64,
    /// Available patches for this target file
    pub patches: Vec<FilePatch>,
}

/// An individual patch that transforms a source file into the target
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    /// Encoding key of the source (old) file
    pub source_ekey: [u8; 16],
    /// Decoded size of the source file (stored as uint40 big-endian)
    pub source_decoded_size: u64,
    /// Encoding key of the patch data on CDN
    pub patch_ekey: [u8; 16],
    /// Size of the patch data in bytes
    pub patch_size: u32,
    /// Patch index (ordering hint)
    pub patch_index: u8,
}

/// Read a 5-byte big-endian unsigned integer (uint40)
pub fn read_uint40_be(reader: &mut impl Read) -> io::Result<u64> {
    let mut buf = [0u8; 5];
    reader.read_exact(&mut buf)?;
    Ok(u64::from(buf[0]) << 32
        | u64::from(buf[1]) << 24
        | u64::from(buf[2]) << 16
        | u64::from(buf[3]) << 8
        | u64::from(buf[4]))
}

/// Write a 5-byte big-endian unsigned integer (uint40)
pub fn write_uint40_be(writer: &mut impl Write, value: u64) -> io::Result<()> {
    let bytes = [
        (value >> 32) as u8,
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ];
    writer.write_all(&bytes)
}

/// Read a fixed-size key, zero-padded to 16 bytes
pub fn read_key(reader: &mut impl Read, key_size: u8) -> io::Result<[u8; 16]> {
    let mut key = [0u8; 16];
    reader.read_exact(&mut key[..key_size as usize])?;
    Ok(key)
}

/// Write a key, truncated to key_size bytes
pub fn write_key(writer: &mut impl Write, key: &[u8; 16], key_size: u8) -> io::Result<()> {
    writer.write_all(&key[..key_size as usize])
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_uint40_round_trip() {
        let values: &[u64] = &[0, 1, 255, 65535, 0xFF_FFFF, 0xFFFF_FFFF, 0xFF_FFFF_FFFF];
        for &val in values {
            let mut buf = Vec::new();
            write_uint40_be(&mut buf, val).unwrap();
            assert_eq!(buf.len(), 5);

            let mut cursor = std::io::Cursor::new(&buf);
            let parsed = read_uint40_be(&mut cursor).unwrap();
            assert_eq!(parsed, val, "round-trip failed for {val}");
        }
    }

    #[test]
    fn test_uint40_known_value() {
        // 5 bytes big-endian: 0x00 0x00 0x01 0x00 0x00 = 65536
        let data = [0x00, 0x00, 0x01, 0x00, 0x00];
        let mut cursor = std::io::Cursor::new(&data[..]);
        assert_eq!(read_uint40_be(&mut cursor).unwrap(), 65536);
    }

    #[test]
    fn test_key_read_write() {
        let key: [u8; 16] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];

        // Full 16-byte key
        let mut buf = Vec::new();
        write_key(&mut buf, &key, 16).unwrap();
        assert_eq!(buf.len(), 16);
        let mut cursor = std::io::Cursor::new(&buf);
        assert_eq!(read_key(&mut cursor, 16).unwrap(), key);

        // Truncated 9-byte key
        let mut buf = Vec::new();
        write_key(&mut buf, &key, 9).unwrap();
        assert_eq!(buf.len(), 9);
        let mut cursor = std::io::Cursor::new(&buf);
        let truncated = read_key(&mut cursor, 9).unwrap();
        assert_eq!(&truncated[..9], &key[..9]);
        assert_eq!(&truncated[9..], &[0; 7]);
    }
}

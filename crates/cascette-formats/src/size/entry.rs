//! Size manifest entry with variable-width esize field

use crate::size::error::{Result, SizeError};
use crate::size::header::SizeHeader;
use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Read, Seek, Write};

/// A single entry in the Size manifest
///
/// Each entry maps an encoding key to an estimated file size (esize).
/// The key length and esize width are determined by the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeEntry {
    /// Encoding key bytes (length determined by header key_size_bytes)
    pub key: Vec<u8>,
    /// 16-bit hash/identifier (big-endian, not 0x0000 or 0xFFFF)
    pub key_hash: u16,
    /// Estimated file size (variable width, stored as u64)
    pub esize: u64,
}

impl SizeEntry {
    /// Create a new size entry
    pub fn new(key: Vec<u8>, key_hash: u16, esize: u64) -> Self {
        Self {
            key,
            key_hash,
            esize,
        }
    }

    /// Validate this entry against the header
    pub fn validate(&self, header: &SizeHeader) -> Result<()> {
        if self.key_hash == 0x0000 || self.key_hash == 0xFFFF {
            return Err(SizeError::InvalidKeyHash(self.key_hash));
        }

        if self.key.len() != header.key_size_bytes() {
            return Err(SizeError::TruncatedData {
                expected: header.key_size_bytes(),
                actual: self.key.len(),
            });
        }

        Ok(())
    }

    /// Calculate serialized size of this entry for the given header
    pub fn serialized_size(header: &SizeHeader) -> usize {
        header.key_size_bytes() + 2 + header.esize_bytes() as usize
    }
}

impl BinRead for SizeEntry {
    type Args<'a> = &'a SizeHeader;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        header: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read key bytes
        let key_len = header.key_size_bytes();
        let mut key = vec![0u8; key_len];
        reader.read_exact(&mut key)?;

        // Read key_hash (u16 BE)
        let mut buf2 = [0u8; 2];
        reader.read_exact(&mut buf2)?;
        let key_hash = u16::from_be_bytes(buf2);

        // Validate key_hash sentinels
        if key_hash == 0x0000 || key_hash == 0xFFFF {
            return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(SizeError::InvalidKeyHash(key_hash)),
            });
        }

        // Read esize (variable width, big-endian, zero-extend to u64)
        let esize_bytes = header.esize_bytes() as usize;
        let mut esize_buf = vec![0u8; esize_bytes];
        reader.read_exact(&mut esize_buf)?;

        let mut esize: u64 = 0;
        for &b in &esize_buf {
            esize = (esize << 8) | u64::from(b);
        }

        Ok(Self {
            key,
            key_hash,
            esize,
        })
    }
}

impl BinWrite for SizeEntry {
    type Args<'a> = &'a SizeHeader;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        header: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write key bytes
        writer.write_all(&self.key)?;

        // Write key_hash (u16 BE)
        writer.write_all(&self.key_hash.to_be_bytes())?;

        // Write esize (variable width, big-endian)
        let esize_bytes = header.esize_bytes() as usize;
        let mut esize_buf = vec![0u8; esize_bytes];
        for i in 0..esize_bytes {
            esize_buf[esize_bytes - 1 - i] = (self.esize >> (i * 8)) as u8;
        }
        writer.write_all(&esize_buf)?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    fn v1_header(esize_bytes: u8) -> SizeHeader {
        SizeHeader::new_v1(0, 1, 128, 0, esize_bytes)
    }

    fn v2_header() -> SizeHeader {
        SizeHeader::new_v2(0, 1, 128, 0)
    }

    #[test]
    fn test_parse_entry_v1_context() {
        let header = v1_header(4);
        let key = vec![0xAB; 16];
        let key_hash: u16 = 0x1234;
        let esize: u32 = 0x0000_5678;

        let mut data = Vec::new();
        data.extend_from_slice(&key);
        data.extend_from_slice(&key_hash.to_be_bytes());
        data.extend_from_slice(&esize.to_be_bytes());

        let mut cursor = Cursor::new(&data);
        let entry = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");

        assert_eq!(entry.key, key);
        assert_eq!(entry.key_hash, 0x1234);
        assert_eq!(entry.esize, 0x5678);
    }

    #[test]
    fn test_parse_entry_v2_context() {
        let header = v2_header();
        let key = vec![0xCD; 16];
        let key_hash: u16 = 0x5678;
        let esize: u32 = 0x0001_0000;

        let mut data = Vec::new();
        data.extend_from_slice(&key);
        data.extend_from_slice(&key_hash.to_be_bytes());
        data.extend_from_slice(&esize.to_be_bytes());

        let mut cursor = Cursor::new(&data);
        let entry = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");

        assert_eq!(entry.key, key);
        assert_eq!(entry.key_hash, 0x5678);
        assert_eq!(entry.esize, 0x0001_0000);
    }

    #[test]
    fn test_entry_round_trip() {
        let header = v1_header(4);
        let entry = SizeEntry::new(vec![0x11; 16], 0x4321, 1024);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");

        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_reject_key_hash_0x0000() {
        let header = v1_header(4);

        let mut data = Vec::new();
        data.extend_from_slice(&[0xAA; 16]); // key
        data.extend_from_slice(&0x0000u16.to_be_bytes()); // invalid sentinel
        data.extend_from_slice(&100u32.to_be_bytes()); // esize

        let mut cursor = Cursor::new(&data);
        let result = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header);
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_key_hash_0xffff() {
        let header = v1_header(4);

        let mut data = Vec::new();
        data.extend_from_slice(&[0xBB; 16]); // key
        data.extend_from_slice(&0xFFFFu16.to_be_bytes()); // invalid sentinel
        data.extend_from_slice(&100u32.to_be_bytes()); // esize

        let mut cursor = Cursor::new(&data);
        let result = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header);
        assert!(result.is_err());
    }

    #[test]
    fn test_esize_width_1_byte() {
        let header = v1_header(1);
        let entry = SizeEntry::new(vec![0x00; 16], 0x0001, 255);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(16) + hash(2) + esize(1) = 19
        assert_eq!(buf.len(), 19);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed.esize, 255);
    }

    #[test]
    fn test_esize_width_2_bytes() {
        let header = v1_header(2);
        let entry = SizeEntry::new(vec![0x00; 16], 0x0001, 0xABCD);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(16) + hash(2) + esize(2) = 20
        assert_eq!(buf.len(), 20);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed.esize, 0xABCD);
    }

    #[test]
    fn test_esize_width_8_bytes() {
        let header = v1_header(8);
        let big_size: u64 = 0x0123_4567_89AB_CDEF;
        let entry = SizeEntry::new(vec![0x00; 16], 0x0001, big_size);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(16) + hash(2) + esize(8) = 26
        assert_eq!(buf.len(), 26);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed.esize, big_size);
    }

    #[test]
    fn test_serialized_size() {
        // V1 with 4-byte esize: 16 + 2 + 4 = 22
        let header = v1_header(4);
        assert_eq!(SizeEntry::serialized_size(&header), 22);

        // V1 with 1-byte esize: 16 + 2 + 1 = 19
        let header = v1_header(1);
        assert_eq!(SizeEntry::serialized_size(&header), 19);

        // V2: 16 + 2 + 4 = 22
        let header = v2_header();
        assert_eq!(SizeEntry::serialized_size(&header), 22);
    }

    #[test]
    fn test_validate_entry() {
        let header = v1_header(4);

        let valid = SizeEntry::new(vec![0x00; 16], 0x1234, 100);
        assert!(valid.validate(&header).is_ok());

        let bad_hash_zero = SizeEntry::new(vec![0x00; 16], 0x0000, 100);
        assert!(bad_hash_zero.validate(&header).is_err());

        let bad_hash_ffff = SizeEntry::new(vec![0x00; 16], 0xFFFF, 100);
        assert!(bad_hash_ffff.validate(&header).is_err());

        let bad_key_len = SizeEntry::new(vec![0x00; 8], 0x1234, 100);
        assert!(bad_key_len.validate(&header).is_err());
    }
}

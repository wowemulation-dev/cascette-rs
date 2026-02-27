//! Size manifest entry with variable-width esize field

use crate::size::error::Result;
use crate::size::header::SizeHeader;
use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Read, Seek, Write};

/// A single entry in the Size manifest
///
/// Each entry maps a (possibly truncated) encoding key to an estimated file
/// size. The key length is `ekey_size` bytes from the header, and the esize
/// width is `esize_bytes`.
///
/// Binary layout per entry: `ekey[ekey_size] + esize[esize_bytes]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeEntry {
    /// Encoding key bytes (length determined by header ekey_size)
    pub key: Vec<u8>,
    /// Estimated file size (variable width, stored as u64)
    pub esize: u64,
}

impl SizeEntry {
    /// Create a new size entry
    pub fn new(key: Vec<u8>, esize: u64) -> Self {
        Self { key, esize }
    }

    /// Validate this entry against the header
    pub fn validate(&self, header: &SizeHeader) -> Result<()> {
        if self.key.len() != header.ekey_size() as usize {
            return Err(crate::size::error::SizeError::TruncatedData {
                expected: header.ekey_size() as usize,
                actual: self.key.len(),
            });
        }

        Ok(())
    }

    /// Calculate serialized size of an entry for the given header
    pub fn serialized_size(header: &SizeHeader) -> usize {
        header.ekey_size() as usize + header.esize_bytes() as usize
    }
}

impl BinRead for SizeEntry {
    type Args<'a> = &'a SizeHeader;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        header: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read key bytes (ekey_size bytes)
        let key_len = header.ekey_size() as usize;
        let mut key = vec![0u8; key_len];
        reader.read_exact(&mut key)?;

        // Read esize (variable width, big-endian, zero-extend to u64)
        let esize_bytes = header.esize_bytes() as usize;
        let mut esize_buf = vec![0u8; esize_bytes];
        reader.read_exact(&mut esize_buf)?;

        let mut esize: u64 = 0;
        for &b in &esize_buf {
            esize = (esize << 8) | u64::from(b);
        }

        Ok(Self { key, esize })
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
        SizeHeader::new_v1(9, 1, 0, 0, esize_bytes)
    }

    fn v2_header() -> SizeHeader {
        SizeHeader::new_v2(9, 1, 0, 0)
    }

    #[test]
    fn test_parse_entry_v1_context() {
        let header = v1_header(4);
        let key = vec![0xAB; 9];
        let esize: u32 = 0x0000_5678;

        let mut data = Vec::new();
        data.extend_from_slice(&key);
        data.extend_from_slice(&esize.to_be_bytes());

        let mut cursor = Cursor::new(&data);
        let entry = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");

        assert_eq!(entry.key, key);
        assert_eq!(entry.esize, 0x5678);
    }

    #[test]
    fn test_parse_entry_v2_context() {
        let header = v2_header();
        let key = vec![0xCD; 9];
        let esize: u32 = 0x0001_0000;

        let mut data = Vec::new();
        data.extend_from_slice(&key);
        data.extend_from_slice(&esize.to_be_bytes());

        let mut cursor = Cursor::new(&data);
        let entry = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");

        assert_eq!(entry.key, key);
        assert_eq!(entry.esize, 0x0001_0000);
    }

    #[test]
    fn test_entry_round_trip() {
        let header = v1_header(4);
        let entry = SizeEntry::new(vec![0x11; 9], 1024);

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
    fn test_esize_width_1_byte() {
        let header = v1_header(1);
        let entry = SizeEntry::new(vec![0x00; 9], 255);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(9) + esize(1) = 10
        assert_eq!(buf.len(), 10);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed.esize, 255);
    }

    #[test]
    fn test_esize_width_2_bytes() {
        let header = v1_header(2);
        let entry = SizeEntry::new(vec![0x00; 9], 0xABCD);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(9) + esize(2) = 11
        assert_eq!(buf.len(), 11);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed.esize, 0xABCD);
    }

    #[test]
    fn test_esize_width_8_bytes() {
        let header = v1_header(8);
        let big_size: u64 = 0x0123_4567_89AB_CDEF;
        let entry = SizeEntry::new(vec![0x00; 9], big_size);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(9) + esize(8) = 17
        assert_eq!(buf.len(), 17);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed.esize, big_size);
    }

    #[test]
    fn test_serialized_size() {
        // V1 with 4-byte esize: 9 + 4 = 13
        let header = v1_header(4);
        assert_eq!(SizeEntry::serialized_size(&header), 13);

        // V1 with 1-byte esize: 9 + 1 = 10
        let header = v1_header(1);
        assert_eq!(SizeEntry::serialized_size(&header), 10);

        // V2: 9 + 4 = 13
        let header = v2_header();
        assert_eq!(SizeEntry::serialized_size(&header), 13);
    }

    #[test]
    fn test_validate_entry() {
        let header = v1_header(4);

        let valid = SizeEntry::new(vec![0x00; 9], 100);
        assert!(valid.validate(&header).is_ok());

        let bad_key_len = SizeEntry::new(vec![0x00; 16], 100);
        assert!(bad_key_len.validate(&header).is_err());
    }

    #[test]
    fn test_full_ekey_size() {
        // Test with 16-byte EKey (full, non-truncated)
        let header = SizeHeader::new_v1(16, 1, 0, 0, 4);
        let entry = SizeEntry::new(vec![0xAA; 16], 42);

        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        entry
            .write_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should write entry");

        // key(16) + esize(4) = 20
        assert_eq!(buf.len(), 20);

        let mut cursor = Cursor::new(&buf);
        let parsed = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
            .expect("Should parse entry");
        assert_eq!(parsed, entry);
    }
}

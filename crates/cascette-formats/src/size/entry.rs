//! Size manifest file entry
//!
//! Source: <https://wowdev.wiki/TACT#Size_manifest>
//!
//! Binary layout per entry:
//!
//! ```text
//! [ekey: ekey_size bytes]   Partial encoding key
//! [esize: 4 bytes BE]       Estimated file size (u32)
//! ```
//!
//! Total on-disk stride per entry: `ekey_size + 4`

use crate::size::error::{Result, SizeError};
use crate::size::header::SizeHeader;
use std::io::Read;

/// A single entry in the Size manifest.
///
/// Maps a partial encoding key to an estimated file size. Entries are sorted
/// descending by `esize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeEntry {
    /// Partial encoding key bytes (length = `header.ekey_size` bytes).
    pub key: Vec<u8>,
    /// Estimated file size in bytes (big-endian u32 on disk).
    pub esize: u32,
}

impl SizeEntry {
    /// Construct a new entry.
    pub fn new(key: Vec<u8>, esize: u32) -> Self {
        Self { key, esize }
    }

    /// On-disk byte count for one entry given the header.
    pub fn serialized_size(header: &SizeHeader) -> usize {
        header.ekey_size as usize + 4
    }

    /// Read one entry from a `Read` source.
    pub fn read_entry<R: Read>(reader: &mut R, header: &SizeHeader) -> Result<Self> {
        let key_len = header.ekey_size as usize;

        let mut key = vec![0u8; key_len];
        reader.read_exact(&mut key)?;

        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let esize = u32::from_be_bytes(buf);

        Ok(Self { key, esize })
    }

    /// Write one entry to a `Write` sink.
    pub fn write_entry<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.key)?;
        writer.write_all(&self.esize.to_be_bytes())?;
        Ok(())
    }

    /// Validate this entry against the header.
    pub fn validate(&self, header: &SizeHeader) -> Result<()> {
        if self.key.len() != header.ekey_size as usize {
            return Err(SizeError::TruncatedData {
                expected: header.ekey_size as usize,
                actual: self.key.len(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_header(ekey_size: u8) -> SizeHeader {
        SizeHeader::new(1, ekey_size, 1, 0, 0)
    }

    #[test]
    fn test_parse_entry() {
        let header = make_header(9);
        let key = vec![0xABu8; 9];
        let esize: u32 = 0x0001_0000;

        let mut data = key.clone();
        data.extend_from_slice(&esize.to_be_bytes());

        let entry = SizeEntry::read_entry(&mut data.as_slice(), &header).expect("Should parse");
        assert_eq!(entry.key, key);
        assert_eq!(entry.esize, esize);
    }

    #[test]
    fn test_round_trip() {
        let header = make_header(9);
        let entry = SizeEntry::new(vec![0x12u8; 9], 151_928_563);

        let mut buf = Vec::new();
        entry.write_entry(&mut buf).expect("Should write");
        assert_eq!(buf.len(), SizeEntry::serialized_size(&header));

        let parsed = SizeEntry::read_entry(&mut buf.as_slice(), &header).expect("Should parse");
        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_serialized_size() {
        // 9-byte key + 4-byte esize = 13
        let header = make_header(9);
        assert_eq!(SizeEntry::serialized_size(&header), 13);

        // 16-byte key + 4 = 20
        let header = make_header(16);
        assert_eq!(SizeEntry::serialized_size(&header), 20);
    }

    #[test]
    fn test_validate_key_length() {
        let header = make_header(9);
        let good = SizeEntry::new(vec![0u8; 9], 0);
        assert!(good.validate(&header).is_ok());

        let bad = SizeEntry::new(vec![0u8; 5], 0);
        assert!(bad.validate(&header).is_err());
    }
}

//! Size manifest implementation
//!
//! Source: <https://wowdev.wiki/TACT#Size_manifest>
//!
//! Binary layout: Header (15 bytes) → Tags → File entries

use crate::install::{InstallTag, TagType};
use crate::size::entry::SizeEntry;
use crate::size::error::{Result, SizeError};
use crate::size::header::SizeHeader;

/// Complete size manifest with header, tags, and file entries.
///
/// The Size manifest maps partial encoding keys to estimated file sizes. Files
/// are sorted descending by `esize`. The manifest was introduced in build 27547.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeManifest {
    /// Parsed header.
    pub header: SizeHeader,
    /// Tags (same structure as install/download manifest tags).
    pub tags: Vec<InstallTag>,
    /// File entries sorted descending by `esize`.
    pub entries: Vec<SizeEntry>,
}

impl SizeManifest {
    /// Parse a size manifest from raw binary data.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < SizeHeader::SIZE {
            return Err(SizeError::TruncatedData {
                expected: SizeHeader::SIZE,
                actual: data.len(),
            });
        }

        let header = SizeHeader::parse(data)?;
        let mut pos = SizeHeader::SIZE;

        // Parse tags — same layout as InstallManifest tags:
        //   null-terminated name + u16 BE type + bitfield of ceil(num_files/8) bytes
        let bitfield_len = (header.num_files as usize).div_ceil(8);
        let mut tags = Vec::with_capacity(header.num_tags as usize);
        for _ in 0..header.num_tags {
            // Read null-terminated name
            let name_start = pos;
            while pos < data.len() && data[pos] != 0 {
                pos += 1;
            }
            if pos >= data.len() {
                return Err(SizeError::TruncatedData {
                    expected: pos + 1,
                    actual: data.len(),
                });
            }
            let name = std::str::from_utf8(&data[name_start..pos])
                .unwrap_or("")
                .to_string();
            pos += 1; // skip null

            // Read u16 tag type
            if pos + 2 > data.len() {
                return Err(SizeError::TruncatedData {
                    expected: pos + 2,
                    actual: data.len(),
                });
            }
            let tag_type_raw = u16::from_be_bytes([data[pos], data[pos + 1]]);
            pos += 2;

            // Read bitfield
            if pos + bitfield_len > data.len() {
                return Err(SizeError::TruncatedData {
                    expected: pos + bitfield_len,
                    actual: data.len(),
                });
            }
            let bit_mask = data[pos..pos + bitfield_len].to_vec();
            pos += bitfield_len;

            let tag_type = TagType::from_u16(tag_type_raw);
            let mut tag = InstallTag::new(name, tag_type, header.num_files as usize);
            tag.bit_mask = bit_mask;
            tags.push(tag);
        }

        // Parse file entries
        let stride = SizeEntry::serialized_size(&header);
        let needed = pos + stride * header.num_files as usize;
        if data.len() < needed {
            return Err(SizeError::TruncatedData {
                expected: needed,
                actual: data.len(),
            });
        }

        let mut entries = Vec::with_capacity(header.num_files as usize);
        let mut slice = &data[pos..];
        for _ in 0..header.num_files {
            let entry = SizeEntry::read_entry(&mut slice, &header)?;
            entries.push(entry);
        }

        Ok(Self {
            header,
            tags,
            entries,
        })
    }

    /// Serialize the manifest to binary data.
    pub fn build(&self) -> Result<Vec<u8>> {
        let bitfield_len = (self.header.num_files as usize).div_ceil(8);
        let mut buf = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut buf);
            self.header.write(&mut cursor)?;
        }

        // Write tags
        for tag in &self.tags {
            buf.extend_from_slice(tag.name.as_bytes());
            buf.push(0);
            let type_raw: u16 = tag.tag_type.as_u16();
            buf.extend_from_slice(&type_raw.to_be_bytes());
            let mut mask = tag.bit_mask.clone();
            mask.resize(bitfield_len, 0);
            buf.extend_from_slice(&mask);
        }

        // Write entries
        for entry in &self.entries {
            entry.write_entry(&mut buf)?;
        }

        Ok(buf)
    }

    /// Validate manifest consistency.
    pub fn validate(&self) -> Result<()> {
        self.header.validate()?;

        if self.tags.len() != self.header.num_tags as usize {
            return Err(SizeError::TagCountMismatch {
                expected: self.header.num_tags,
                actual: self.tags.len(),
            });
        }

        if self.entries.len() != self.header.num_files as usize {
            return Err(SizeError::EntryCountMismatch {
                expected: self.header.num_files,
                actual: self.entries.len(),
            });
        }

        let computed: u64 = self.entries.iter().map(|e| u64::from(e.esize)).sum();
        if computed != self.header.total_size {
            return Err(SizeError::TotalSizeMismatch {
                expected: self.header.total_size,
                actual: computed,
            });
        }

        for entry in &self.entries {
            entry.validate(&self.header)?;
        }

        Ok(())
    }
}

impl crate::CascFormat for SizeManifest {
    fn parse(data: &[u8]) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::CascFormat;

    fn make_manifest_bytes(ekey_size: u8, entries: &[(&[u8], u32)]) -> Vec<u8> {
        let num_files = entries.len() as u32;
        let total_size: u64 = entries.iter().map(|(_, s)| u64::from(*s)).sum();
        let mut data = Vec::new();

        // Header (15 bytes)
        data.extend_from_slice(b"DS");
        data.push(1); // version
        data.push(ekey_size);
        data.extend_from_slice(&num_files.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes()); // num_tags
        data.push((total_size >> 32) as u8);
        data.push((total_size >> 24) as u8);
        data.push((total_size >> 16) as u8);
        data.push((total_size >> 8) as u8);
        data.push(total_size as u8);

        // File entries
        for (key, esize) in entries {
            data.extend_from_slice(key);
            data.extend_from_slice(&esize.to_be_bytes());
        }

        data
    }

    #[test]
    fn test_parse_basic_manifest() {
        let key1 = [0xAAu8; 9];
        let key2 = [0xBBu8; 9];
        let data = make_manifest_bytes(9, &[(&key1, 1000), (&key2, 500)]);

        let manifest = SizeManifest::parse(&data).expect("Should parse");
        assert_eq!(manifest.header.version, 1);
        assert_eq!(manifest.header.ekey_size, 9);
        assert_eq!(manifest.header.num_files, 2);
        assert_eq!(manifest.header.num_tags, 0);
        assert_eq!(manifest.header.total_size, 1500);
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries[0].key, key1);
        assert_eq!(manifest.entries[0].esize, 1000);
        assert_eq!(manifest.entries[1].key, key2);
        assert_eq!(manifest.entries[1].esize, 500);
    }

    #[test]
    fn test_round_trip() {
        let key1 = [0x11u8; 9];
        let key2 = [0x22u8; 9];
        let data = make_manifest_bytes(9, &[(&key1, 151_928_563), (&key2, 102_906_605)]);

        let manifest = SizeManifest::parse(&data).expect("Should parse");
        let rebuilt = manifest.build().expect("Should build");
        assert_eq!(data, rebuilt);
    }

    #[test]
    fn test_casc_format_trait() {
        let key = [0xCCu8; 9];
        let data = make_manifest_bytes(9, &[(&key, 42)]);

        let manifest = <SizeManifest as CascFormat>::parse(&data).expect("CascFormat parse");
        let rebuilt = CascFormat::build(&manifest).expect("CascFormat build");
        assert_eq!(data, rebuilt);
    }

    #[test]
    fn test_empty_manifest() {
        let data = make_manifest_bytes(9, &[]);
        let manifest = SizeManifest::parse(&data).expect("Should parse empty manifest");
        assert_eq!(manifest.entries.len(), 0);
        assert_eq!(manifest.header.total_size, 0);
    }

    #[test]
    fn test_truncated_header() {
        let data = vec![0x44, 0x53, 0x01]; // Only 3 bytes
        let result = SizeManifest::parse(&data);
        assert!(matches!(
            result,
            Err(SizeError::TruncatedData {
                expected: 15,
                actual: 3
            })
        ));
    }

    #[test]
    fn test_validate_count_mismatch() {
        let manifest = SizeManifest {
            header: SizeHeader::new(1, 9, 5, 0, 0),
            tags: vec![],
            entries: vec![],
        };
        assert!(matches!(
            manifest.validate(),
            Err(SizeError::EntryCountMismatch {
                expected: 5,
                actual: 0
            })
        ));
    }

    #[test]
    fn test_validate_total_size_mismatch() {
        let manifest = SizeManifest {
            header: SizeHeader::new(1, 9, 1, 0, 9999),
            tags: vec![],
            entries: vec![SizeEntry::new(vec![0u8; 9], 100)],
        };
        assert!(matches!(
            manifest.validate(),
            Err(SizeError::TotalSizeMismatch {
                expected: 9999,
                actual: 100
            })
        ));
    }

    #[test]
    fn test_parse_real_build_entry_values() {
        // First two entries from actual WoW Classic 1.13.2 size manifest
        let data = make_manifest_bytes(
            9,
            &[
                (
                    &[0x64, 0x7d, 0x17, 0xbb, 0x12, 0x2b, 0x5c, 0xb6, 0xf9],
                    151_928_563,
                ),
                (
                    &[0x69, 0x90, 0x1b, 0xdf, 0x9e, 0xb8, 0x13, 0xc0, 0xc1],
                    102_906_605,
                ),
            ],
        );
        let manifest = SizeManifest::parse(&data).expect("Should parse");
        assert_eq!(manifest.entries[0].esize, 151_928_563);
        assert_eq!(manifest.entries[1].esize, 102_906_605);
    }
}

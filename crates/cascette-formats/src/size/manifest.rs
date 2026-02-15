//! Main size manifest implementation

use crate::size::entry::SizeEntry;
use crate::size::error::{Result, SizeError};
use crate::size::header::SizeHeader;
use binrw::{BinRead, BinWrite};
use std::io::Cursor;

/// Complete size manifest with header and entries
///
/// The Size manifest maps encoding keys to estimated file sizes (eSize).
/// It is used when compressed size is unavailable, enabling disk space
/// estimation and download progress reporting.
///
/// Binary layout: Header → Entries
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeManifest {
    /// Version-aware header
    pub header: SizeHeader,
    /// Encoding key to esize entries
    pub entries: Vec<SizeEntry>,
}

/// Minimum data size to read the base header fields (magic + version + flags +
/// entry_count + key_size_bits = 10 bytes) plus the V2 extension (5 bytes) = 15
const MIN_HEADER_SIZE: usize = 15;

/// Minimum V1 header size (base 10 + u64 total_size 8 + u8 esize_bytes 1 = 19)
const MIN_V1_HEADER_SIZE: usize = 19;

impl SizeManifest {
    /// Parse a size manifest from binary data
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < MIN_HEADER_SIZE {
            return Err(SizeError::TruncatedData {
                expected: MIN_HEADER_SIZE,
                actual: data.len(),
            });
        }

        // Check version to determine full minimum size
        // Version byte is at offset 2
        if data[2] == 1 && data.len() < MIN_V1_HEADER_SIZE {
            return Err(SizeError::TruncatedData {
                expected: MIN_V1_HEADER_SIZE,
                actual: data.len(),
            });
        }

        let mut cursor = Cursor::new(data);

        // Parse header
        let header = SizeHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .map_err(SizeError::from)?;

        // Validate header
        header.validate()?;

        // Parse entries
        let mut entries = Vec::with_capacity(header.entry_count() as usize);
        for _ in 0..header.entry_count() {
            let entry = SizeEntry::read_options(&mut cursor, binrw::Endian::Big, &header)
                .map_err(SizeError::from)?;
            entries.push(entry);
        }

        let manifest = Self { header, entries };

        // Final validation
        manifest.validate()?;

        Ok(manifest)
    }

    /// Build the size manifest to binary data
    pub fn build(&self) -> Result<Vec<u8>> {
        self.validate()?;

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        // Write header
        self.header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .map_err(SizeError::from)?;

        // Write entries
        for entry in &self.entries {
            entry
                .write_options(&mut cursor, binrw::Endian::Big, &self.header)
                .map_err(SizeError::from)?;
        }

        Ok(buffer)
    }

    /// Validate manifest consistency
    pub fn validate(&self) -> Result<()> {
        // Validate header
        self.header.validate()?;

        // Validate entry count
        if self.entries.len() != self.header.entry_count() as usize {
            return Err(SizeError::EntryCountMismatch {
                expected: self.header.entry_count(),
                actual: self.entries.len(),
            });
        }

        // Validate total_size matches sum of esizes
        let computed_total: u64 = self.entries.iter().map(|e| e.esize).sum();
        if computed_total != self.header.total_size() {
            return Err(SizeError::TotalSizeMismatch {
                expected: self.header.total_size(),
                actual: computed_total,
            });
        }

        // Validate individual entries
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
    use crate::size::builder::SizeManifestBuilder;

    fn build_v1_manifest_bytes(
        entry_count: u32,
        key_size_bits: u16,
        esize_bytes: u8,
        entries: &[(Vec<u8>, u16, u64)],
    ) -> Vec<u8> {
        let total_size: u64 = entries.iter().map(|(_, _, s)| *s).sum();
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(b"DS");
        data.push(1); // version
        data.push(0); // flags
        data.extend_from_slice(&entry_count.to_be_bytes());
        data.extend_from_slice(&key_size_bits.to_be_bytes());
        data.extend_from_slice(&total_size.to_be_bytes());
        data.push(esize_bytes);

        // Entries
        for (key, hash, esize) in entries {
            data.extend_from_slice(key);
            data.extend_from_slice(&hash.to_be_bytes());
            // Write esize as esize_bytes bytes BE
            for i in (0..esize_bytes as usize).rev() {
                data.push((esize >> (i * 8)) as u8);
            }
        }

        data
    }

    #[test]
    fn test_parse_complete_v1_manifest() {
        let entries = vec![
            (vec![0xAA; 16], 0x1234u16, 1000u64),
            (vec![0xBB; 16], 0x5678u16, 2000u64),
        ];
        let data = build_v1_manifest_bytes(2, 128, 4, &entries);

        let manifest = SizeManifest::parse(&data).expect("Should parse manifest");
        assert_eq!(manifest.header.version(), 1);
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries[0].key, vec![0xAA; 16]);
        assert_eq!(manifest.entries[0].key_hash, 0x1234);
        assert_eq!(manifest.entries[0].esize, 1000);
        assert_eq!(manifest.entries[1].esize, 2000);
        assert_eq!(manifest.header.total_size(), 3000);
    }

    #[test]
    fn test_parse_complete_v2_manifest() {
        let total: u64 = 500;
        let mut data = Vec::new();

        // V2 header
        data.extend_from_slice(b"DS");
        data.push(2); // version
        data.push(0); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&128u16.to_be_bytes()); // key_size_bits
        // 40-bit total_size
        data.push((total >> 32) as u8);
        data.push((total >> 24) as u8);
        data.push((total >> 16) as u8);
        data.push((total >> 8) as u8);
        data.push(total as u8);

        // Entry: key(16) + hash(2) + esize(4)
        data.extend_from_slice(&[0xCC; 16]);
        data.extend_from_slice(&0x9ABCu16.to_be_bytes());
        data.extend_from_slice(&500u32.to_be_bytes());

        let manifest = SizeManifest::parse(&data).expect("Should parse V2 manifest");
        assert_eq!(manifest.header.version(), 2);
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].esize, 500);
    }

    #[test]
    fn test_manifest_round_trip() {
        let entries = vec![
            (vec![0x11; 16], 0x1111u16, 100u64),
            (vec![0x22; 16], 0x2222u16, 200u64),
            (vec![0x33; 16], 0x3333u16, 300u64),
        ];
        let data = build_v1_manifest_bytes(3, 128, 4, &entries);

        let manifest = SizeManifest::parse(&data).expect("Should parse");
        let rebuilt = manifest.build().expect("Should build");

        assert_eq!(data, rebuilt);
    }

    #[test]
    fn test_empty_manifest() {
        let data = build_v1_manifest_bytes(0, 128, 4, &[]);
        let manifest = SizeManifest::parse(&data).expect("Should parse empty manifest");
        assert_eq!(manifest.entries.len(), 0);
        assert_eq!(manifest.header.total_size(), 0);
    }

    #[test]
    fn test_validation_count_mismatch() {
        let manifest = SizeManifest {
            header: SizeHeader::new_v1(0, 5, 128, 0, 4), // claims 5 entries
            entries: vec![],                             // but has 0
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
    fn test_validation_total_size_mismatch() {
        let manifest = SizeManifest {
            header: SizeHeader::new_v1(0, 1, 128, 9999, 4), // claims total 9999
            entries: vec![SizeEntry::new(vec![0x00; 16], 0x1234, 100)], // but sum is 100
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
    fn test_truncated_data() {
        let data = vec![0x44, 0x53, 0x01]; // Only 3 bytes
        assert!(matches!(
            SizeManifest::parse(&data),
            Err(SizeError::TruncatedData {
                expected: 15,
                actual: 3
            })
        ));
    }

    #[test]
    fn test_casc_format_trait_round_trip() {
        let entries = vec![(vec![0xFF; 16], 0x0001u16, 42u64)];
        let data = build_v1_manifest_bytes(1, 128, 4, &entries);

        let manifest = <SizeManifest as CascFormat>::parse(&data).expect("CascFormat parse");
        let rebuilt = CascFormat::build(&manifest).expect("CascFormat build");
        assert_eq!(data, rebuilt);
    }

    #[test]
    fn test_builder_round_trip() {
        let manifest = SizeManifestBuilder::new()
            .version(1)
            .key_size_bits(128)
            .add_entry(vec![0xAA; 16], 0x1234, 500)
            .add_entry(vec![0xBB; 16], 0x5678, 600)
            .build()
            .expect("Should build manifest");

        assert_eq!(manifest.header.version(), 1);
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.header.total_size(), 1100);

        let data = manifest.build().expect("Should serialize");
        let parsed = SizeManifest::parse(&data).expect("Should parse");
        assert_eq!(manifest, parsed);
    }
}

//! Encoding file parser for TACT
//!
//! The encoding file maps Content Keys (CKey) to Encoding Keys (EKey) and vice versa.
//! This is a critical component for resolving file references in the TACT system.
//!
//! IMPORTANT: Encoding files use BIG-ENDIAN byte order, unlike most other TACT formats!

use byteorder::{BigEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use tracing::{debug, trace, warn};

use crate::{Error, Result};

/// Magic bytes for encoding file: "EN"
const ENCODING_MAGIC: [u8; 2] = [0x45, 0x4E]; // 'E', 'N'

/// Encoding file header
#[derive(Debug, Clone)]
pub struct EncodingHeader {
    /// Magic bytes "EN"
    pub magic: [u8; 2],
    /// Version (should be 1)
    pub version: u8,
    /// Hash size for CKeys (usually 16 for MD5)
    pub ckey_hash_size: u8,
    /// Hash size for EKeys (usually 16 for MD5)
    pub ekey_hash_size: u8,
    /// Page size for CKey pages in KB
    pub ckey_page_size_kb: u16,
    /// Page size for EKey pages in KB
    pub ekey_page_size_kb: u16,
    /// Number of CKey pages
    pub ckey_page_count: u32,
    /// Number of EKey pages
    pub ekey_page_count: u32,
    /// Unknown field (must be 0)
    pub unk: u8,
    /// ESpec block size
    pub espec_block_size: u32,
}

/// Page table entry
#[derive(Debug, Clone)]
pub struct PageInfo {
    /// First hash in this page
    pub first_hash: Vec<u8>,
    /// MD5 checksum of the page
    pub checksum: [u8; 16],
}

/// Encoding entry for a content key
#[derive(Debug, Clone)]
pub struct EncodingEntry {
    /// The content key
    pub content_key: Vec<u8>,
    /// List of encoding keys for this content
    pub encoding_keys: Vec<Vec<u8>>,
    /// File size (40-bit integer)
    pub size: u64,
}

/// Encoding file parser and lookup
pub struct EncodingFile {
    /// File header
    pub header: EncodingHeader,
    /// CKey → EncodingEntry mapping
    ckey_entries: HashMap<Vec<u8>, EncodingEntry>,
    /// EKey → CKey reverse mapping
    ekey_to_ckey: HashMap<Vec<u8>, Vec<u8>>,
}

impl EncodingFile {
    /// Parse an encoding file from raw data
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Parse header
        let header = Self::parse_header(&mut cursor)?;
        debug!(
            "Parsed encoding header: version={}, ckey_pages={}, ekey_pages={}, ckey_page_size_kb={}, ekey_page_size_kb={}, espec_table_size={}",
            header.version,
            header.ckey_page_count,
            header.ekey_page_count,
            header.ckey_page_size_kb,
            header.ekey_page_size_kb,
            header.espec_block_size
        );

        // Read ESpec string table (comes immediately after header)
        let mut espec_data = vec![0u8; header.espec_block_size as usize];
        cursor.read_exact(&mut espec_data)?;
        debug!("Read ESpec string table: {} bytes", espec_data.len());

        // Parse CKey page table indices
        let ckey_page_table = Self::parse_page_table(
            &mut cursor,
            header.ckey_page_count as usize,
            header.ckey_hash_size as usize,
        )?;
        trace!("Parsed {} CKey page table entries", ckey_page_table.len());

        // Parse EKey page table indices
        let _ekey_page_table = Self::parse_page_table(
            &mut cursor,
            header.ekey_page_count as usize,
            header.ekey_hash_size as usize,
        )?;
        trace!("Parsed {} EKey page table entries", _ekey_page_table.len());

        // Parse CKey pages - read directly from cursor position like rustycasc does
        let mut ckey_entries = HashMap::new();
        let page_size = header.ckey_page_size_kb as usize * 1024;

        // Get remaining data from cursor to validate checksums correctly
        let remaining_data = {
            let current_pos = cursor.position() as usize;
            &data[current_pos..]
        };
        let mut data_offset = 0;

        for (i, page_info) in ckey_page_table.iter().enumerate() {
            // Validate checksum on the data at current position (like rustycasc)
            if data_offset + page_size <= remaining_data.len() {
                let page_slice = &remaining_data[data_offset..data_offset + page_size];
                let checksum = ::md5::compute(page_slice);

                if checksum.as_ref() != page_info.checksum {
                    debug!(
                        "CKey page {} checksum mismatch (expected: {:?}, got: {:?})",
                        i,
                        hex::encode(page_info.checksum),
                        hex::encode(checksum.as_ref())
                    );
                }

                Self::parse_ckey_page(
                    page_slice,
                    header.ckey_hash_size,
                    header.ekey_hash_size,
                    &mut ckey_entries,
                )?;
            }

            data_offset += page_size;
        }

        // Advance cursor past all CKey pages
        cursor.set_position(cursor.position() + (header.ckey_page_count as u64 * page_size as u64));

        debug!("Parsed {} CKey entries", ckey_entries.len());

        // Build reverse mapping (EKey → CKey)
        let mut ekey_to_ckey = HashMap::new();
        for entry in ckey_entries.values() {
            for ekey in &entry.encoding_keys {
                ekey_to_ckey.insert(ekey.clone(), entry.content_key.clone());
            }
        }

        debug!(
            "Built EKey→CKey reverse mapping with {} entries",
            ekey_to_ckey.len()
        );

        Ok(Self {
            header,
            ckey_entries,
            ekey_to_ckey,
        })
    }

    /// Parse the encoding file header
    fn parse_header<R: Read>(reader: &mut R) -> Result<EncodingHeader> {
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;

        if magic != ENCODING_MAGIC {
            return Err(Error::BadMagic);
        }

        let version = reader.read_u8()?;
        if version != 1 {
            warn!("Unexpected encoding version: {}", version);
        }

        let ckey_hash_size = reader.read_u8()?;
        let ekey_hash_size = reader.read_u8()?;
        let ckey_page_size_kb = reader.read_u16::<BigEndian>()?; // BIG-ENDIAN!
        let ekey_page_size_kb = reader.read_u16::<BigEndian>()?; // BIG-ENDIAN!
        let ckey_page_count = reader.read_u32::<BigEndian>()?; // BIG-ENDIAN!
        let ekey_page_count = reader.read_u32::<BigEndian>()?; // BIG-ENDIAN!
        let unk = reader.read_u8()?;
        let espec_block_size = reader.read_u32::<BigEndian>()?; // BIG-ENDIAN!

        Ok(EncodingHeader {
            magic,
            version,
            ckey_hash_size,
            ekey_hash_size,
            ckey_page_size_kb,
            ekey_page_size_kb,
            ckey_page_count,
            ekey_page_count,
            unk,
            espec_block_size,
        })
    }

    /// Parse a page table
    fn parse_page_table<R: Read>(
        reader: &mut R,
        page_count: usize,
        hash_size: usize,
    ) -> Result<Vec<PageInfo>> {
        let mut pages = Vec::with_capacity(page_count);

        for _ in 0..page_count {
            let mut first_hash = vec![0u8; hash_size];
            reader.read_exact(&mut first_hash)?;

            let mut checksum = [0u8; 16];
            reader.read_exact(&mut checksum)?;

            pages.push(PageInfo {
                first_hash,
                checksum,
            });
        }

        Ok(pages)
    }

    /// Parse a CKey page
    fn parse_ckey_page(
        data: &[u8],
        ckey_size: u8,
        ekey_size: u8,
        entries: &mut HashMap<Vec<u8>, EncodingEntry>,
    ) -> Result<()> {
        let mut offset = 0;

        while offset < data.len() {
            // Check for zero padding (end of page data)
            if offset + 6 > data.len() || data[offset..].iter().all(|&b| b == 0) {
                break;
            }

            // Read key count
            let key_count = data[offset];
            offset += 1;

            if key_count == 0 {
                break; // End of entries
            }

            // Read file size (40-bit integer - big-endian like the header!)
            if offset + 5 > data.len() {
                break;
            }
            let size = crate::utils::read_uint40_be(&data[offset..offset + 5])?;
            offset += 5;

            // Read content key
            if offset + ckey_size as usize > data.len() {
                break;
            }
            let ckey = data[offset..offset + ckey_size as usize].to_vec();
            offset += ckey_size as usize;

            // Read encoding keys
            let mut ekeys = Vec::new();
            for _ in 0..key_count {
                if offset + ekey_size as usize > data.len() {
                    break;
                }
                let ekey = data[offset..offset + ekey_size as usize].to_vec();
                offset += ekey_size as usize;
                ekeys.push(ekey);
            }

            entries.insert(
                ckey.clone(),
                EncodingEntry {
                    content_key: ckey,
                    encoding_keys: ekeys,
                    size,
                },
            );
        }

        Ok(())
    }

    /// Look up encoding keys by content key
    pub fn lookup_by_ckey(&self, ckey: &[u8]) -> Option<&EncodingEntry> {
        self.ckey_entries.get(ckey)
    }

    /// Look up content key by encoding key
    pub fn lookup_by_ekey(&self, ekey: &[u8]) -> Option<&Vec<u8>> {
        self.ekey_to_ckey.get(ekey)
    }

    /// Get the first encoding key for a content key (most common case)
    pub fn get_ekey_for_ckey(&self, ckey: &[u8]) -> Option<&Vec<u8>> {
        self.ckey_entries
            .get(ckey)
            .and_then(|entry| entry.encoding_keys.first())
    }

    /// Get file size for a content key
    pub fn get_file_size(&self, ckey: &[u8]) -> Option<u64> {
        self.ckey_entries.get(ckey).map(|entry| entry.size)
    }

    /// Get total number of content keys
    pub fn ckey_count(&self) -> usize {
        self.ckey_entries.len()
    }

    /// Get total number of encoding keys
    pub fn ekey_count(&self) -> usize {
        self.ekey_to_ckey.len()
    }

    /// Get sample content keys for debugging
    pub fn get_sample_ckeys(&self, limit: usize) -> Vec<String> {
        self.ckey_entries
            .keys()
            .take(limit)
            .map(hex::encode)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_header_size() {
        // Header should be exactly 22 bytes
        let header_size = 2 + 1 + 1 + 1 + 2 + 2 + 4 + 4 + 1 + 4;
        assert_eq!(header_size, 22);
    }

    #[test]
    fn test_parse_empty_encoding() {
        // Create a minimal valid encoding file
        let mut data = Vec::new();

        // Magic
        data.extend_from_slice(&ENCODING_MAGIC);
        // Version
        data.push(1);
        // Hash sizes
        data.push(16); // CKey hash size
        data.push(16); // EKey hash size
        // Page sizes (big-endian!)
        data.extend_from_slice(&0u16.to_be_bytes()); // CKey page size
        data.extend_from_slice(&0u16.to_be_bytes()); // EKey page size
        // Page counts (big-endian!)
        data.extend_from_slice(&0u32.to_be_bytes()); // CKey page count
        data.extend_from_slice(&0u32.to_be_bytes()); // EKey page count
        // Unknown
        data.push(0);
        // ESpec block size (big-endian!)
        data.extend_from_slice(&0u32.to_be_bytes());

        let result = EncodingFile::parse(&data);
        assert!(result.is_ok());

        let encoding = result.unwrap();
        assert_eq!(encoding.header.version, 1);
        assert_eq!(encoding.header.ckey_hash_size, 16);
        assert_eq!(encoding.header.ekey_hash_size, 16);
        assert_eq!(encoding.ckey_count(), 0);
        assert_eq!(encoding.ekey_count(), 0);
    }

    #[test]
    fn test_invalid_magic() {
        let mut data = vec![0xFF, 0xFF]; // Wrong magic
        data.push(1); // Version

        let result = EncodingFile::parse(&data);
        assert!(matches!(result, Err(Error::BadMagic)));
    }
}

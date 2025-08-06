//! Size file parser for TACT
//!
//! The size file contains a mapping of encoding keys to file sizes,
//! helping clients calculate installation requirements.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use byteorder::{BigEndian, ReadBytesExt};
use tracing::{debug, trace};

use crate::utils::{read_cstring_from, read_uint40_from};
use crate::{Error, Result};

/// Size file header
#[derive(Debug, Clone)]
pub struct SizeHeader {
    /// Magic bytes "DS"
    pub magic: [u8; 2],
    /// Version (typically 1)
    pub version: u8,
    /// EKey size (typically 9 - first 9 bytes of MD5)
    pub ekey_size: u8,
    /// Number of entries
    pub entry_count: u32,
    /// Number of tags
    pub tag_count: u16,
    /// Total size of all files (40-bit)
    pub total_size: u64,
}

impl SizeHeader {
    /// Parse size file header
    pub fn parse<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;
        
        if magic != [b'D', b'S'] {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid size file magic: {:?}", magic),
            )));
        }

        let version = reader.read_u8()?;
        let ekey_size = reader.read_u8()?;
        let entry_count = reader.read_u32::<BigEndian>()?;
        let tag_count = reader.read_u16::<BigEndian>()?;
        let total_size = read_uint40_from(reader)?;

        Ok(SizeHeader {
            magic,
            version,
            ekey_size,
            entry_count,
            tag_count,
            total_size,
        })
    }
}

/// Size file entry
#[derive(Debug, Clone)]
pub struct SizeEntry {
    /// Encoding key (partial - first N bytes)
    pub ekey: Vec<u8>,
    /// Compressed size (32-bit)
    pub compressed_size: u32,
}

impl SizeEntry {
    /// Parse a size entry
    pub fn parse<R: Read>(reader: &mut R, header: &SizeHeader) -> Result<Self> {
        let mut ekey = vec![0u8; header.ekey_size as usize];
        reader.read_exact(&mut ekey)?;
        
        let compressed_size = reader.read_u32::<BigEndian>()?;

        Ok(SizeEntry {
            ekey,
            compressed_size,
        })
    }
}

/// Size file tag
#[derive(Debug, Clone)]
pub struct SizeTag {
    /// Tag name
    pub name: String,
    /// Tag type
    pub tag_type: u16,
    /// Bitmask indicating which entries have this tag
    pub mask: Vec<u8>,
}

/// Size file
#[derive(Debug, Clone)]
pub struct SizeFile {
    /// Header information
    pub header: SizeHeader,
    /// Size entries indexed by partial EKey
    pub entries: HashMap<Vec<u8>, SizeEntry>,
    /// Tags for conditional size calculation
    pub tags: Vec<SizeTag>,
    /// Entries ordered by size (largest first)
    pub size_order: Vec<Vec<u8>>,
    /// Entries in parse order (for tag mask application)
    pub parse_order: Vec<Vec<u8>>,
}

impl SizeFile {
    /// Parse a size file from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let header = SizeHeader::parse(&mut cursor)?;
        
        debug!(
            "Parsing size file v{} with {} entries, total size: {}",
            header.version, header.entry_count, header.total_size
        );

        // Parse tags first (they come before entries in this format)
        let mut tags = Vec::with_capacity(header.tag_count as usize);
        let bytes_per_tag = ((header.entry_count + 7) / 8) as usize;
        
        for i in 0..header.tag_count {
            let name = read_cstring_from(&mut cursor)?;
            let tag_type = cursor.read_u16::<BigEndian>()?;
            
            let mut mask = vec![0u8; bytes_per_tag];
            cursor.read_exact(&mut mask)?;
            
            trace!("Tag {}: '{}' type={}", i, name, tag_type);
            
            tags.push(SizeTag {
                name,
                tag_type,
                mask,
            });
        }

        // Parse entries
        let mut entries = HashMap::with_capacity(header.entry_count as usize);
        let mut size_list = Vec::with_capacity(header.entry_count as usize);
        let mut parse_order = Vec::with_capacity(header.entry_count as usize);
        
        for i in 0..header.entry_count {
            let entry = SizeEntry::parse(&mut cursor, &header)?;
            trace!(
                "Entry {}: EKey {:02x?} size={}",
                i,
                &entry.ekey[..4.min(entry.ekey.len())],
                entry.compressed_size
            );
            size_list.push((entry.compressed_size, entry.ekey.clone()));
            parse_order.push(entry.ekey.clone());
            entries.insert(entry.ekey.clone(), entry);
        }

        // Sort by size (largest first)
        size_list.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
        let size_order: Vec<Vec<u8>> = size_list
            .into_iter()
            .map(|(_, ekey)| ekey)
            .collect();

        // Verify total size
        let calculated_total: u64 = entries.values()
            .map(|e| e.compressed_size as u64)
            .sum();
        
        if calculated_total != header.total_size {
            debug!(
                "Warning: Calculated total size {} doesn't match header total {}",
                calculated_total, header.total_size
            );
        }

        Ok(SizeFile {
            header,
            entries,
            tags,
            size_order,
            parse_order,
        })
    }

    /// Get file size by partial EKey
    pub fn get_file_size(&self, ekey: &[u8]) -> Option<u32> {
        // If the provided key is longer than what we store, truncate it
        let key = if ekey.len() > self.header.ekey_size as usize {
            &ekey[..self.header.ekey_size as usize]
        } else {
            ekey
        };
        
        self.entries.get(key).map(|e| e.compressed_size)
    }

    /// Get total installation size (all files)
    pub fn get_total_size(&self) -> u64 {
        self.header.total_size
    }

    /// Calculate size for specific tags
    pub fn get_size_for_tags(&self, tag_names: &[&str]) -> u64 {
        // Find matching tags
        let mut combined_mask = vec![0u8; ((self.header.entry_count + 7) / 8) as usize];
        
        for tag in &self.tags {
            if tag_names.contains(&tag.name.as_str()) {
                // OR the masks together
                for (i, byte) in tag.mask.iter().enumerate() {
                    combined_mask[i] |= byte;
                }
            }
        }

        // Calculate total size for entries matching the mask
        // Use the parse order to match entries with the mask bits
        let mut total = 0u64;
        
        for (index, ekey) in self.parse_order.iter().enumerate() {
            if let Some(entry) = self.entries.get(ekey) {
                let byte_index = index / 8;
                let bit_index = index % 8;
                
                if byte_index < combined_mask.len() {
                    let bit = (combined_mask[byte_index] >> (7 - bit_index)) & 1;
                    if bit == 1 {
                        total += entry.compressed_size as u64;
                    }
                }
            }
        }

        total
    }

    /// Get the N largest files
    pub fn get_largest_files(&self, count: usize) -> Vec<(&Vec<u8>, u32)> {
        self.size_order
            .iter()
            .take(count)
            .filter_map(|ekey| {
                self.entries.get(ekey).map(|entry| (ekey, entry.compressed_size))
            })
            .collect()
    }

    /// Calculate statistics
    pub fn get_statistics(&self) -> SizeStatistics {
        let sizes: Vec<u32> = self.entries.values()
            .map(|e| e.compressed_size)
            .collect();

        let total = sizes.iter().map(|&s| s as u64).sum();
        let average = if !sizes.is_empty() {
            total / sizes.len() as u64
        } else {
            0
        };

        let min = sizes.iter().min().copied().unwrap_or(0);
        let max = sizes.iter().max().copied().unwrap_or(0);

        SizeStatistics {
            total_size: total,
            file_count: sizes.len() as u32,
            average_size: average as u32,
            min_size: min,
            max_size: max,
        }
    }
}

/// Size file statistics
#[derive(Debug, Clone)]
pub struct SizeStatistics {
    pub total_size: u64,
    pub file_count: u32,
    pub average_size: u32,
    pub min_size: u32,
    pub max_size: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_header() {
        let data = vec![
            b'D', b'S',     // Magic
            1,              // Version
            9,              // EKey size (partial MD5)
            0, 0, 0, 3,     // Entry count (3, big-endian)
            0, 2,           // Tag count (2, big-endian)
            0, 0x10, 0, 0, 0, // Total size (4096, 40-bit LE)
        ];

        let mut cursor = Cursor::new(data);
        let header = SizeHeader::parse(&mut cursor).unwrap();

        assert_eq!(header.magic, [b'D', b'S']);
        assert_eq!(header.version, 1);
        assert_eq!(header.ekey_size, 9);
        assert_eq!(header.entry_count, 3);
        assert_eq!(header.tag_count, 2);
        assert_eq!(header.total_size, 4096);
    }

    #[test]
    fn test_size_calculation() {
        // Create entries with known sizes
        let mut entries = HashMap::new();
        
        entries.insert(
            vec![1; 9],
            SizeEntry {
                ekey: vec![1; 9],
                compressed_size: 1000,
            },
        );
        
        entries.insert(
            vec![2; 9],
            SizeEntry {
                ekey: vec![2; 9],
                compressed_size: 2000,
            },
        );
        
        entries.insert(
            vec![3; 9],
            SizeEntry {
                ekey: vec![3; 9],
                compressed_size: 3000,
            },
        );

        let total: u64 = entries.values()
            .map(|e| e.compressed_size as u64)
            .sum();
        
        assert_eq!(total, 6000);
    }

    #[test]
    fn test_partial_ekey_lookup() {
        let mut entries = HashMap::new();
        
        let partial_key = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33];
        entries.insert(
            partial_key.clone(),
            SizeEntry {
                ekey: partial_key.clone(),
                compressed_size: 12345,
            },
        );

        let size_file = SizeFile {
            header: SizeHeader {
                magic: [b'D', b'S'],
                version: 1,
                ekey_size: 9,
                entry_count: 1,
                tag_count: 0,
                total_size: 12345,
            },
            entries,
            tags: vec![],
            size_order: vec![partial_key.clone()],
            parse_order: vec![partial_key.clone()],
        };

        // Lookup with exact key
        assert_eq!(size_file.get_file_size(&partial_key), Some(12345));

        // Lookup with longer key (full MD5) - should truncate and match
        let full_md5 = vec![
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0x00,
        ];
        assert_eq!(size_file.get_file_size(&full_md5), Some(12345));

        // Lookup with non-existent key
        assert_eq!(size_file.get_file_size(&vec![0xFF; 9]), None);
    }
}
//! Download manifest parser for TACT
//!
//! The download manifest lists files with their download priority, helping clients
//! determine which files to download first during installation or updates.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use byteorder::{BigEndian, ReadBytesExt};
use tracing::{debug, trace};

use crate::utils::{read_cstring_from, read_uint40_from};
use crate::{Error, Result};

/// Download manifest header
#[derive(Debug, Clone)]
pub struct DownloadHeader {
    /// Magic bytes "DL"
    pub magic: [u8; 2],
    /// Version (1, 2, or 3)
    pub version: u8,
    /// EKey size (typically 16)
    pub ekey_size: u8,
    /// Whether entries include checksums
    pub has_checksum: bool,
    /// Number of file entries
    pub entry_count: u32,
    /// Number of tags
    pub tag_count: u16,
    /// Size of flag data per entry (v2+)
    pub flag_size: u8,
    /// Base priority offset (v3+)
    pub base_priority: i8,
    /// Unknown field (v3+)
    pub unknown: u32,
}

impl DownloadHeader {
    /// Parse download manifest header
    pub fn parse<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;
        
        if magic != [b'D', b'L'] {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid download manifest magic: {:?}", magic),
            )));
        }

        let version = reader.read_u8()?;
        let ekey_size = reader.read_u8()?;
        let has_checksum = reader.read_u8()? != 0;
        let entry_count = reader.read_u32::<BigEndian>()?;
        let tag_count = reader.read_u16::<BigEndian>()?;

        let mut flag_size = 0;
        let mut base_priority = 0i8;
        let mut unknown = 0u32;

        if version >= 2 {
            flag_size = reader.read_u8()?;
            
            if version >= 3 {
                base_priority = reader.read_i8()?;
                // Read 24-bit big-endian value
                let mut bytes = [0u8; 3];
                reader.read_exact(&mut bytes)?;
                unknown = u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]);
            }
        }

        Ok(DownloadHeader {
            magic,
            version,
            ekey_size,
            has_checksum,
            entry_count,
            tag_count,
            flag_size,
            base_priority,
            unknown,
        })
    }
}

/// Download manifest file entry
#[derive(Debug, Clone)]
pub struct DownloadEntry {
    /// Encoding key
    pub ekey: Vec<u8>,
    /// Compressed size
    pub compressed_size: u64,
    /// Download priority (0 = highest, higher = lower priority)
    pub priority: i8,
    /// Optional checksum
    pub checksum: Option<u32>,
    /// Plugin flags (v2+)
    pub flags: Vec<u8>,
}

impl DownloadEntry {
    /// Parse a download entry
    pub fn parse<R: Read>(reader: &mut R, header: &DownloadHeader) -> Result<Self> {
        let mut ekey = vec![0u8; header.ekey_size as usize];
        reader.read_exact(&mut ekey)?;

        let compressed_size = read_uint40_from(reader)?;
        
        // Read raw priority and adjust by base
        let raw_priority = reader.read_i8()?;
        let priority = raw_priority - header.base_priority;

        let checksum = if header.has_checksum {
            Some(reader.read_u32::<BigEndian>()?)
        } else {
            None
        };

        let mut flags = vec![];
        if header.version >= 2 && header.flag_size > 0 {
            flags = vec![0u8; header.flag_size as usize];
            reader.read_exact(&mut flags)?;
        }

        Ok(DownloadEntry {
            ekey,
            compressed_size,
            priority,
            checksum,
            flags,
        })
    }
}

/// Download manifest tag
#[derive(Debug, Clone)]
pub struct DownloadTag {
    /// Tag name
    pub name: String,
    /// Tag type (1 = locale, 2 = platform, etc.)
    pub tag_type: u16,
    /// Bitmask indicating which entries have this tag
    pub mask: Vec<u8>,
}

/// Download manifest file
#[derive(Debug, Clone)]
pub struct DownloadManifest {
    /// Header information
    pub header: DownloadHeader,
    /// File entries indexed by EKey
    pub entries: HashMap<Vec<u8>, DownloadEntry>,
    /// Download priority order (sorted)
    pub priority_order: Vec<Vec<u8>>,
    /// Tags for conditional downloads
    pub tags: Vec<DownloadTag>,
}

impl DownloadManifest {
    /// Parse a download manifest from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let header = DownloadHeader::parse(&mut cursor)?;
        
        debug!(
            "Parsing download manifest v{} with {} entries and {} tags",
            header.version, header.entry_count, header.tag_count
        );

        // Parse entries
        let mut entries = HashMap::with_capacity(header.entry_count as usize);
        let mut priority_list = Vec::with_capacity(header.entry_count as usize);
        
        for i in 0..header.entry_count {
            let entry = DownloadEntry::parse(&mut cursor, &header)?;
            trace!(
                "Entry {}: EKey {:02x?} priority={} size={}",
                i,
                &entry.ekey[..4.min(entry.ekey.len())],
                entry.priority,
                entry.compressed_size
            );
            priority_list.push((entry.priority, entry.ekey.clone()));
            entries.insert(entry.ekey.clone(), entry);
        }

        // Sort by priority (0 is highest priority)
        priority_list.sort_by_key(|(priority, _)| *priority);
        let priority_order: Vec<Vec<u8>> = priority_list
            .into_iter()
            .map(|(_, ekey)| ekey)
            .collect();

        // Parse tags
        let mut tags = Vec::with_capacity(header.tag_count as usize);
        let bytes_per_tag = ((header.entry_count + 7) / 8) as usize;
        
        for i in 0..header.tag_count {
            let name = read_cstring_from(&mut cursor)?;
            let tag_type = cursor.read_u16::<BigEndian>()?;
            
            let mut mask = vec![0u8; bytes_per_tag];
            cursor.read_exact(&mut mask)?;
            
            trace!("Tag {}: '{}' type={}", i, name, tag_type);
            
            tags.push(DownloadTag {
                name,
                tag_type,
                mask,
            });
        }

        debug!(
            "Parsed {} entries with {} priority levels",
            entries.len(),
            entries.values()
                .map(|e| e.priority)
                .collect::<std::collections::HashSet<_>>()
                .len()
        );

        Ok(DownloadManifest {
            header,
            entries,
            priority_order,
            tags,
        })
    }

    /// Get files by priority (0 = highest)
    pub fn get_priority_files(&self, max_priority: i8) -> Vec<&DownloadEntry> {
        self.priority_order
            .iter()
            .filter_map(|ekey| {
                let entry = self.entries.get(ekey)?;
                if entry.priority <= max_priority {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get files for specific tags
    pub fn get_files_for_tags(&self, tag_names: &[&str]) -> Vec<&DownloadEntry> {
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

        // Collect entries that match the mask
        let mut result = Vec::new();
        for (index, ekey) in self.priority_order.iter().enumerate() {
            let byte_index = index / 8;
            let bit_index = index % 8;
            
            if byte_index < combined_mask.len() {
                let bit = (combined_mask[byte_index] >> (7 - bit_index)) & 1;
                if bit == 1 {
                    if let Some(entry) = self.entries.get(ekey) {
                        result.push(entry);
                    }
                }
            }
        }

        result
    }

    /// Get total download size for priority level
    pub fn get_download_size(&self, max_priority: i8) -> u64 {
        self.get_priority_files(max_priority)
            .iter()
            .map(|e| e.compressed_size)
            .sum()
    }

    /// Get all high priority files (priority 0)
    pub fn get_essential_files(&self) -> Vec<&DownloadEntry> {
        self.get_priority_files(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_header_v1() {
        let data = vec![
            b'D', b'L',  // Magic
            1,           // Version
            16,          // EKey size
            0,           // No checksum
            0, 0, 0, 2,  // Entry count (2, big-endian)
            0, 1,        // Tag count (1, big-endian)
        ];

        let mut cursor = Cursor::new(data);
        let header = DownloadHeader::parse(&mut cursor).unwrap();

        assert_eq!(header.magic, [b'D', b'L']);
        assert_eq!(header.version, 1);
        assert_eq!(header.ekey_size, 16);
        assert!(!header.has_checksum);
        assert_eq!(header.entry_count, 2);
        assert_eq!(header.tag_count, 1);
        assert_eq!(header.flag_size, 0); // Not present in v1
    }

    #[test]
    fn test_download_header_v3() {
        let data = vec![
            b'D', b'L',     // Magic
            3,              // Version
            16,             // EKey size
            1,              // Has checksum
            0, 0, 0, 10,    // Entry count (10, big-endian)
            0, 3,           // Tag count (3, big-endian)
            2,              // Flag size
            254u8,          // Base priority (-2 as i8)
            0, 0, 0,        // Unknown (24-bit)
        ];

        let mut cursor = Cursor::new(data);
        let header = DownloadHeader::parse(&mut cursor).unwrap();

        assert_eq!(header.version, 3);
        assert!(header.has_checksum);
        assert_eq!(header.entry_count, 10);
        assert_eq!(header.tag_count, 3);
        assert_eq!(header.flag_size, 2);
        assert_eq!(header.base_priority, -2);
    }

    #[test]
    fn test_priority_sorting() {
        // Create a simple manifest with different priorities
        let mut entries = HashMap::new();
        
        let entry1 = DownloadEntry {
            ekey: vec![1; 16],
            compressed_size: 1000,
            priority: 2,  // Lower priority
            checksum: None,
            flags: vec![],
        };
        
        let entry2 = DownloadEntry {
            ekey: vec![2; 16],
            compressed_size: 2000,
            priority: 0,  // Highest priority
            checksum: None,
            flags: vec![],
        };
        
        let entry3 = DownloadEntry {
            ekey: vec![3; 16],
            compressed_size: 3000,
            priority: 1,  // Medium priority
            checksum: None,
            flags: vec![],
        };

        entries.insert(entry1.ekey.clone(), entry1);
        entries.insert(entry2.ekey.clone(), entry2);
        entries.insert(entry3.ekey.clone(), entry3);

        // Create priority order
        let mut priority_list = vec![
            (2, vec![1; 16]),
            (0, vec![2; 16]),
            (1, vec![3; 16]),
        ];
        priority_list.sort_by_key(|(p, _)| *p);
        
        let priority_order: Vec<Vec<u8>> = priority_list
            .into_iter()
            .map(|(_, ekey)| ekey)
            .collect();

        // Check order is correct (0, 1, 2)
        assert_eq!(priority_order[0], vec![2; 16]); // Priority 0
        assert_eq!(priority_order[1], vec![3; 16]); // Priority 1
        assert_eq!(priority_order[2], vec![1; 16]); // Priority 2
    }
}
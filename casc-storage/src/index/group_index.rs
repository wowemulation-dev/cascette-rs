//! Parser for .index group index files

use crate::error::{CascError, Result};
use crate::types::{ArchiveLocation, EKey, IndexEntry};
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use tracing::{debug, trace};

/// Header for group index files
#[derive(Debug)]
struct GroupIndexHeader {
    version: u16,
    bucket_index: u8,
    extra_bytes: u8,
    span_size_bytes: u8,
    span_offset_bytes: u8,
    ekey_bytes: u8,
    archive_bytes: u8,
    archive_total_size: u64,
}

/// Parser for .index group index files
pub struct GroupIndex {
    entries: HashMap<EKey, ArchiveLocation>,
    header: GroupIndexHeader,
}

impl GroupIndex {
    /// Parse a group index file from disk
    pub fn parse_file(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::parse(&mut reader)
    }

    /// Parse a group index file from a reader
    pub fn parse<R: Read>(reader: &mut R) -> Result<Self> {
        // Read header
        let header = Self::read_header(reader)?;
        
        debug!(
            "Parsing group index: version={}, bucket={:02x}, ekey_size={}",
            header.version, header.bucket_index, header.ekey_bytes
        );

        // Calculate entry size
        let entry_size = header.ekey_bytes + header.archive_bytes + 
                        header.span_offset_bytes + header.span_size_bytes;

        // Read all remaining data
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        // Parse entries
        let entry_count = data.len() / entry_size as usize;
        debug!("Parsing {} entries of {} bytes each", entry_count, entry_size);

        let mut entries = HashMap::new();
        let mut offset = 0;

        for i in 0..entry_count {
            let entry = Self::parse_entry(&data[offset..], &header)?;
            offset += entry_size as usize;

            if i < 5 {
                trace!(
                    "Entry {}: ekey={}, archive={}, offset={:x}, size={}",
                    i, entry.ekey, entry.location.archive_id,
                    entry.location.offset, entry.location.size
                );
            }

            entries.insert(entry.ekey, entry.location);
        }

        debug!("Parsed {} entries", entries.len());

        Ok(Self { entries, header })
    }

    fn read_header<R: Read>(reader: &mut R) -> Result<GroupIndexHeader> {
        Ok(GroupIndexHeader {
            version: reader.read_u16::<LittleEndian>()?,
            bucket_index: reader.read_u8()?,
            extra_bytes: reader.read_u8()?,
            span_size_bytes: reader.read_u8()?,
            span_offset_bytes: reader.read_u8()?,
            ekey_bytes: reader.read_u8()?,
            archive_bytes: reader.read_u8()?,
            archive_total_size: reader.read_u64::<LittleEndian>()?,
        })
    }

    fn parse_entry(data: &[u8], header: &GroupIndexHeader) -> Result<IndexEntry> {
        let mut offset = 0;

        // Read truncated EKey
        let ekey = if header.ekey_bytes == 9 {
            let mut full_key = [0u8; 16];
            full_key[0..9].copy_from_slice(&data[offset..offset + 9]);
            offset += 9;
            EKey::new(full_key)
        } else if header.ekey_bytes == 16 {
            let key_bytes = &data[offset..offset + 16];
            offset += 16;
            EKey::from_slice(key_bytes)
                .ok_or_else(|| CascError::InvalidIndexFormat("Invalid key size".into()))?
        } else {
            return Err(CascError::InvalidIndexFormat(
                format!("Unsupported ekey size: {}", header.ekey_bytes)
            ));
        };

        // Read archive ID
        let archive_id = match header.archive_bytes {
            1 => {
                let id = data[offset];
                offset += 1;
                id as u16
            }
            2 => {
                let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
                offset += 2;
                id
            }
            _ => return Err(CascError::InvalidIndexFormat(
                format!("Unsupported archive bytes: {}", header.archive_bytes)
            )),
        };

        // Read offset
        let file_offset = match header.span_offset_bytes {
            4 => {
                let bytes = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
                offset += 4;
                u32::from_le_bytes(bytes) as u64
            }
            5 => {
                let mut bytes = [0u8; 8];
                bytes[0..5].copy_from_slice(&data[offset..offset + 5]);
                offset += 5;
                u64::from_le_bytes(bytes)
            }
            6 => {
                let mut bytes = [0u8; 8];
                bytes[0..6].copy_from_slice(&data[offset..offset + 6]);
                offset += 6;
                u64::from_le_bytes(bytes)
            }
            _ => return Err(CascError::InvalidIndexFormat(
                format!("Unsupported offset bytes: {}", header.span_offset_bytes)
            )),
        };

        // Read size
        let size = match header.span_size_bytes {
            4 => {
                let bytes = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
                u32::from_le_bytes(bytes)
            }
            3 => {
                let mut bytes = [0u8; 4];
                bytes[0..3].copy_from_slice(&data[offset..offset + 3]);
                u32::from_le_bytes(bytes)
            }
            2 => {
                let bytes = [data[offset], data[offset + 1]];
                u16::from_le_bytes(bytes) as u32
            }
            _ => return Err(CascError::InvalidIndexFormat(
                format!("Unsupported size bytes: {}", header.span_size_bytes)
            )),
        };

        Ok(IndexEntry {
            ekey,
            location: ArchiveLocation {
                archive_id,
                offset: file_offset,
                size,
            },
        })
    }

    /// Get the bucket index for this group
    pub fn bucket_index(&self) -> u8 {
        self.header.bucket_index
    }

    /// Get the version of this index
    pub fn version(&self) -> u16 {
        self.header.version
    }

    /// Look up an entry by EKey
    pub fn lookup(&self, ekey: &EKey) -> Option<&ArchiveLocation> {
        self.entries.get(ekey)
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries
    pub fn entries(&self) -> impl Iterator<Item = (&EKey, &ArchiveLocation)> {
        self.entries.iter()
    }

    /// Get total archive size
    pub fn archive_total_size(&self) -> u64 {
        self.header.archive_total_size
    }
}
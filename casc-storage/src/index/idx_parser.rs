//! Parser for .idx index files (bucket-based indices)

use crate::error::{CascError, Result};
use crate::types::{ArchiveLocation, EKey, IndexEntry};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use tracing::{debug, trace};

/// Header for .idx files
#[derive(Debug)]
struct IdxHeader {
    data_size: u32,
    data_hash: u32,
    version: u16,
    bucket: u8,
    length_field_size: u8,
    location_field_size: u8,
    key_field_size: u8,
    segment_bits: u8,
}

/// Parser for .idx index files
pub struct IdxParser {
    entries: HashMap<EKey, ArchiveLocation>,
    bucket: u8,
    version: u16,
}

impl IdxParser {
    /// Parse an .idx file from disk
    pub fn parse_file(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::parse(&mut reader)
    }

    /// Parse an .idx file from a reader
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        // Read header hash section
        let data_size = reader.read_u32::<LittleEndian>()?;
        let data_hash = reader.read_u32::<LittleEndian>()?;

        debug!("Parsing .idx file: size={}, hash={:08x}", data_size, data_hash);

        // TODO: Verify header hash using Jenkins lookup3
        // For now, skip the hash verification
        
        // Read header fields
        let version = reader.read_u16::<LittleEndian>()?;
        let bucket = reader.read_u8()?;
        let length_field_size = reader.read_u8()?;
        let location_field_size = reader.read_u8()?;
        let key_field_size = reader.read_u8()?;
        let segment_bits = reader.read_u8()?;

        trace!(
            "IDX header: version={}, bucket={:02x}, key_size={}, location_size={}, length_size={}, segment_bits={}",
            version, bucket, key_field_size, location_field_size, length_field_size, segment_bits
        );

        // Validate field sizes
        if key_field_size != 9 && key_field_size != 16 {
            return Err(CascError::InvalidIndexFormat(
                format!("Invalid key field size: {}", key_field_size)
            ));
        }

        // Read block table (for now we skip it as we don't use it directly)
        let block_count = (data_size - 8) / 8;
        for _ in 0..block_count {
            let _block_start = reader.read_u32::<BigEndian>()?;
            let _block_end = reader.read_u32::<BigEndian>()?;
        }

        // Align to 16-byte boundary
        let current_pos = 16 + data_size as u64;
        let padding = (16 - (current_pos % 16)) % 16;
        if padding > 0 {
            reader.seek(SeekFrom::Current(padding as i64))?;
        }

        // Read data section
        let data_section_size = reader.read_u32::<LittleEndian>()?;
        let data_section_hash = reader.read_u32::<LittleEndian>()?;

        debug!("Data section: size={}, hash={:08x}", data_section_size, data_section_hash);

        // Calculate entry size and count
        let entry_size = key_field_size + location_field_size + length_field_size;
        let entry_count = data_section_size / entry_size as u32;

        debug!("Parsing {} entries of {} bytes each", entry_count, entry_size);

        // Parse entries
        let mut entries = HashMap::new();
        
        for i in 0..entry_count {
            let entry = Self::parse_entry(
                reader,
                key_field_size,
                location_field_size,
                length_field_size,
                segment_bits
            )?;
            
            if i < 5 {
                trace!("Entry {}: ekey={}, archive={}, offset={:x}, size={}", 
                    i, entry.ekey, entry.location.archive_id, 
                    entry.location.offset, entry.location.size);
            }
            
            entries.insert(entry.ekey, entry.location);
        }

        debug!("Parsed {} entries for bucket {:02x}", entries.len(), bucket);

        Ok(Self {
            entries,
            bucket,
            version,
        })
    }

    fn parse_entry<R: Read>(
        reader: &mut R,
        key_size: u8,
        location_size: u8,
        length_size: u8,
        segment_bits: u8,
    ) -> Result<IndexEntry> {
        // Read key bytes
        let mut key_bytes = vec![0u8; key_size as usize];
        reader.read_exact(&mut key_bytes)?;

        // Expand truncated key to full 16 bytes if needed
        let ekey = if key_size == 9 {
            let mut full_key = [0u8; 16];
            full_key[0..9].copy_from_slice(&key_bytes);
            EKey::new(full_key)
        } else {
            EKey::from_slice(&key_bytes)
                .ok_or_else(|| CascError::InvalidIndexFormat("Invalid key size".into()))?
        };

        // Calculate field sizes
        let offset_size = (segment_bits + 7) / 8;
        let file_size = location_size - offset_size;

        // Read file number (little-endian)
        let mut file_bytes = vec![0u8; file_size as usize];
        reader.read_exact(&mut file_bytes)?;
        let mut archive_id = 0u64;
        for (i, &byte) in file_bytes.iter().enumerate() {
            archive_id |= (byte as u64) << (i * 8);
        }

        // Read offset (big-endian)
        let mut offset_bytes = vec![0u8; offset_size as usize];
        reader.read_exact(&mut offset_bytes)?;
        let mut offset = 0u64;
        for &byte in &offset_bytes {
            offset = (offset << 8) | (byte as u64);
        }

        // Combine file number and offset bits
        let extra_bits = (offset_size * 8) - segment_bits;
        archive_id <<= extra_bits;
        let high_bits = offset >> segment_bits;
        archive_id |= high_bits;
        offset &= (1u64 << segment_bits) - 1;

        // Read size (little-endian)
        let size = match length_size {
            4 => reader.read_u32::<LittleEndian>()?,
            3 => {
                let mut bytes = [0u8; 4];
                reader.read_exact(&mut bytes[0..3])?;
                u32::from_le_bytes(bytes)
            }
            2 => reader.read_u16::<LittleEndian>()? as u32,
            1 => reader.read_u8()? as u32,
            _ => return Err(CascError::InvalidIndexFormat(
                format!("Invalid length field size: {}", length_size)
            )),
        };

        Ok(IndexEntry {
            ekey,
            location: ArchiveLocation {
                archive_id: archive_id as u16,
                offset,
                size,
            },
        })
    }

    /// Get the bucket index for this parser
    pub fn bucket(&self) -> u8 {
        self.bucket
    }

    /// Get the version of this index
    pub fn version(&self) -> u16 {
        self.version
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
}
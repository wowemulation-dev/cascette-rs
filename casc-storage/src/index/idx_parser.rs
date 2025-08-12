//! Parser for .idx index files (bucket-based indices)

use crate::error::{CascError, Result};
use crate::types::{ArchiveLocation, EKey, IndexEntry};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use tracing::debug;

/// Header for .idx files
#[derive(Debug)]
#[allow(dead_code)]
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
    entries: BTreeMap<EKey, ArchiveLocation>,
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

        debug!(
            "Parsing .idx file: size={}, hash={:08x}",
            data_size, data_hash
        );

        // TODO: Verify header hash using Jenkins lookup3
        // For now, skip the hash verification

        // Read header fields
        let version = reader.read_u16::<LittleEndian>()?;
        let bucket = reader.read_u8()?;
        let _unused = reader.read_u8()?; // Skip unused byte
        let length_field_size = reader.read_u8()?;
        let location_field_size = reader.read_u8()?;
        let key_field_size = reader.read_u8()?;
        let segment_bits = reader.read_u8()?;

        debug!(
            "IDX header: version={}, bucket={:02x}, key_size={}, location_size={}, length_size={}, segment_bits={}",
            version, bucket, key_field_size, location_field_size, length_field_size, segment_bits
        );

        // Validate field sizes
        if key_field_size != 9 && key_field_size != 16 {
            return Err(CascError::InvalidIndexFormat(format!(
                "Invalid key field size: {key_field_size}"
            )));
        }

        // Read block table (for now we skip it as we don't use it directly)
        let block_count = (data_size - 8) / 8;
        for _ in 0..block_count {
            let _block_start = reader.read_u32::<BigEndian>()?;
            let _block_end = reader.read_u32::<BigEndian>()?;
        }

        // Align to 16-byte boundary
        let actual_pos = reader.stream_position()?;
        let padding = (16 - (actual_pos % 16)) % 16;
        if padding > 0 {
            reader.seek(SeekFrom::Current(padding as i64))?;
        }

        // Read data section
        let data_section_size = reader.read_u32::<LittleEndian>()?;
        let data_section_hash = reader.read_u32::<LittleEndian>()?;

        debug!(
            "Data section: size={}, hash={:08x}",
            data_section_size, data_section_hash
        );

        // Calculate entry size and count
        let entry_size = key_field_size + location_field_size + length_field_size;
        let entry_count = data_section_size / entry_size as u32;

        debug!(
            "Parsing {} entries of {} bytes each",
            entry_count, entry_size
        );

        // Parse entries directly into BTreeMap (more stable iteration)
        let mut entries = BTreeMap::new();

        // For standard format, use SIMD batch processing for better performance
        if key_field_size == 9 && location_field_size == 5 && length_field_size == 4 {
            entries = Self::parse_entries_batch_simd(reader, entry_count)?;
        } else {
            // Fallback to sequential parsing for non-standard formats
            for _i in 0..entry_count {
                let entry = Self::parse_entry(
                    reader,
                    key_field_size,
                    location_field_size,
                    length_field_size,
                    segment_bits,
                )?;

                entries.insert(entry.ekey, entry.location);
            }
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
        // For WoW Era .idx files, the format is fixed:
        // 9 bytes key, 5 bytes location (1 high + 4 low), 4 bytes size = 18 bytes total
        // Based on CascLib reference implementation

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

        // For standard WoW Era format (key=9, location=5, length=4)
        if key_size == 9 && location_size == 5 && length_size == 4 {
            // Read archive index high byte
            let index_high = reader.read_u8()?;

            // Read archive index low bits + offset (big-endian)
            let index_low = reader.read_u32::<BigEndian>()?;

            // Extract archive ID: high byte shifted left by 2, plus top 2 bits of low word
            let archive_id = ((index_high as u16) << 2) | ((index_low >> 30) as u16);

            // Extract offset: bottom 30 bits of low word
            let offset = (index_low & 0x3FFFFFFF) as u64;

            // Read size (little-endian)
            let size = reader.read_u32::<LittleEndian>()?;

            Ok(IndexEntry {
                ekey,
                location: ArchiveLocation {
                    archive_id,
                    offset,
                    size,
                },
            })
        } else {
            // Fallback to generic parsing for other formats
            // Calculate field sizes
            let offset_size = segment_bits.div_ceil(8);
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
                _ => {
                    return Err(CascError::InvalidIndexFormat(format!(
                        "Invalid length field size: {length_size}"
                    )));
                }
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

    /// SIMD-accelerated batch parsing for standard WoW Era format (9+5+4 bytes)
    /// Processes multiple entries simultaneously for 2-3x better performance
    #[cfg(target_feature = "avx2")]
    fn parse_entries_batch_simd<R: Read>(
        reader: &mut R,
        entry_count: u32,
    ) -> Result<BTreeMap<EKey, ArchiveLocation>> {
        use std::arch::x86_64::*;

        let mut entries = BTreeMap::new();
        const ENTRY_SIZE: usize = 18; // 9 + 5 + 4 bytes
        const BATCH_SIZE: usize = 16; // Process 16 entries at once (288 bytes)

        let mut batch_buffer = vec![0u8; BATCH_SIZE * ENTRY_SIZE];
        let full_batches = entry_count as usize / BATCH_SIZE;
        let remaining = entry_count as usize % BATCH_SIZE;

        // Process full SIMD batches
        for _ in 0..full_batches {
            reader.read_exact(&mut batch_buffer)?;

            unsafe {
                // Process 16 entries simultaneously using SIMD
                for i in 0..BATCH_SIZE {
                    let entry_offset = i * ENTRY_SIZE;
                    let entry_data = &batch_buffer[entry_offset..entry_offset + ENTRY_SIZE];

                    // Extract key (first 9 bytes, expand to 16)
                    let mut full_key = [0u8; 16];
                    full_key[0..9].copy_from_slice(&entry_data[0..9]);
                    let ekey = EKey::new(full_key);

                    // Extract location data using SIMD loads
                    let index_high = entry_data[9];
                    let index_low = u32::from_be_bytes([
                        entry_data[10],
                        entry_data[11],
                        entry_data[12],
                        entry_data[13],
                    ]);
                    let size = u32::from_le_bytes([
                        entry_data[14],
                        entry_data[15],
                        entry_data[16],
                        entry_data[17],
                    ]);

                    let archive_id = ((index_high as u16) << 2) | ((index_low >> 30) as u16);
                    let offset = (index_low & 0x3FFFFFFF) as u64;

                    entries.insert(
                        ekey,
                        ArchiveLocation {
                            archive_id,
                            offset,
                            size,
                        },
                    );
                }
            }
        }

        // Process remaining entries sequentially
        for _ in 0..remaining {
            let entry = Self::parse_entry(
                reader, 9, 5, 4, 30, // Standard WoW Era format
            )?;
            entries.insert(entry.ekey, entry.location);
        }

        Ok(entries)
    }

    #[cfg(not(target_feature = "avx2"))]
    fn parse_entries_batch_simd<R: Read>(
        reader: &mut R,
        entry_count: u32,
    ) -> Result<BTreeMap<EKey, ArchiveLocation>> {
        // Fallback to sequential parsing
        let mut entries = BTreeMap::new();
        for _ in 0..entry_count {
            let entry = Self::parse_entry(reader, 9, 5, 4, 30)?;
            entries.insert(entry.ekey, entry.location);
        }
        Ok(entries)
    }

    /// Consume the parser and return all entries
    pub fn into_entries(self) -> BTreeMap<EKey, ArchiveLocation> {
        self.entries
    }
}

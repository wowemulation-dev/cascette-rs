//! Archive parsing for concatenated BLTE files

use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;
use tracing::{debug, trace, warn};

use super::{ArchiveEntry, ArchiveMetadata, BLTEArchive};
use crate::{BLTE_MAGIC, Error, Result};

/// Parse concatenated BLTE archive from data
pub fn parse_archive(data: Vec<u8>) -> Result<BLTEArchive> {
    debug!("Parsing BLTE archive: {} bytes", data.len());

    let entries = find_blte_files(&data)?;
    debug!("Found {} BLTE files in archive", entries.len());

    let total_compressed = entries.iter().map(|e| e.size as u64).sum();
    let metadata = ArchiveMetadata {
        file_count: entries.len(),
        compressed_size: total_compressed,
        decompressed_size: None, // Will be calculated when files are loaded
        created: None,
    };

    Ok(BLTEArchive {
        files: entries,
        data: Some(data),
        metadata,
    })
}

/// Find all BLTE files in concatenated archive data
fn find_blte_files(data: &[u8]) -> Result<Vec<ArchiveEntry>> {
    let mut files = Vec::new();
    let mut offset = 0;

    while offset + 8 <= data.len() {
        // Look for BLTE magic
        #[allow(clippy::op_ref)]
        if &data[offset..offset + 4] == BLTE_MAGIC {
            trace!("Found BLTE magic at offset {}", offset);

            // Parse header size
            let mut cursor = Cursor::new(&data[offset + 4..offset + 8]);
            let header_size = cursor.read_u32::<BigEndian>().map_err(Error::Io)?;

            // Calculate total BLTE file size
            match calculate_blte_size(&data[offset..], header_size) {
                Ok(total_size) => {
                    if offset + total_size <= data.len() {
                        let mut entry = ArchiveEntry::new(offset, total_size);

                        // Try to extract basic metadata without full parsing
                        if let Ok(metadata) =
                            extract_entry_metadata(&data[offset..offset + total_size])
                        {
                            entry.metadata = metadata;
                        }

                        trace!(
                            "Added BLTE file: offset={}, size={}",
                            entry.offset, entry.size
                        );
                        files.push(entry);
                        offset += total_size;
                    } else {
                        warn!(
                            "BLTE file extends beyond archive bounds: offset={}, size={}, archive_size={}",
                            offset,
                            total_size,
                            data.len()
                        );
                        break;
                    }
                }
                Err(e) => {
                    debug!("Failed to calculate BLTE size at offset {}: {}", offset, e);
                    offset += 1; // Skip this byte and continue searching
                }
            }
        } else {
            offset += 1;
        }
    }

    if files.is_empty() {
        return Err(Error::InvalidMagic([0, 0, 0, 0])); // No BLTE files found
    }

    Ok(files)
}

/// Calculate the total size of a BLTE file from its header
fn calculate_blte_size(data: &[u8], header_size: u32) -> Result<usize> {
    if data.len() < 8 {
        return Err(Error::TruncatedData {
            expected: 8,
            actual: data.len(),
        });
    }

    if header_size == 0 {
        // Single chunk file - the compressed data follows immediately
        // We need to read more data to find where this BLTE ends and the next begins
        // For single chunk, we look for the next BLTE magic or end of data
        find_single_chunk_end(data)
    } else {
        // Multi-chunk file - parse chunk table to get data size
        calculate_multichunk_size(data, header_size)
    }
}

/// Find the end of a single-chunk BLTE file
fn find_single_chunk_end(data: &[u8]) -> Result<usize> {
    // For single chunk files, we need to scan forward to find the next BLTE magic
    // This is because single chunk files don't have a chunk table with size info
    let mut pos = 8; // Skip the BLTE header

    // Look for the next BLTE magic sequence
    while pos + 4 <= data.len() {
        #[allow(clippy::op_ref)]
        if &data[pos..pos + 4] == BLTE_MAGIC {
            return Ok(pos);
        }
        pos += 1;
    }

    // If no next BLTE found, this file goes to the end
    Ok(data.len())
}

/// Calculate size for multi-chunk BLTE file
fn calculate_multichunk_size(data: &[u8], header_size: u32) -> Result<usize> {
    if data.len() < 12 {
        return Err(Error::TruncatedData {
            expected: 12,
            actual: data.len(),
        });
    }

    // Parse chunk count from chunk table
    let chunk_count_bytes = [data[9], data[10], data[11]];
    let chunk_count = u32::from_be_bytes([
        0,
        chunk_count_bytes[0],
        chunk_count_bytes[1],
        chunk_count_bytes[2],
    ]);

    if chunk_count == 0 || chunk_count > 65536 {
        return Err(Error::InvalidChunkCount(chunk_count));
    }

    // Determine data offset based on format detection
    let expected_chunk_table_size = 4 + (chunk_count * 24) as usize; // flags + chunk entries
    let data_offset = if header_size as usize == expected_chunk_table_size {
        // Standard format: header_size = chunk table size
        8 + header_size as usize
    } else if header_size as usize == 8 + expected_chunk_table_size {
        // Archive format: header_size = 8 + chunk table size
        header_size as usize
    } else {
        // Fallback: use standard calculation
        8 + header_size as usize
    };

    // Parse all chunk sizes to get total data size
    let mut total_chunk_size = 0u32;
    let mut chunk_offset = 12; // Start after flags and chunk count

    // Check if we have extended chunk format (40 bytes vs 24 bytes)
    let flags = data[8];
    let chunk_entry_size = match flags {
        0x0F => 24, // Standard chunk info
        0x10 => 40, // Extended chunk info
        _ => return Err(Error::InvalidHeaderSize(flags as u32)),
    };

    for _ in 0..chunk_count {
        if chunk_offset + 4 > data.len() {
            return Err(Error::TruncatedData {
                expected: chunk_offset + 4,
                actual: data.len(),
            });
        }

        // Read compressed size (first 4 bytes of chunk entry)
        let mut cursor = Cursor::new(&data[chunk_offset..chunk_offset + 4]);
        let chunk_compressed_size = cursor.read_u32::<BigEndian>().map_err(Error::Io)?;

        total_chunk_size += chunk_compressed_size;
        chunk_offset += chunk_entry_size;
    }

    Ok(data_offset + total_chunk_size as usize)
}

/// Extract metadata from BLTE file without full parsing
fn extract_entry_metadata(data: &[u8]) -> Result<super::EntryMetadata> {
    use super::EntryMetadata;

    if data.len() < 8 {
        return Err(Error::TruncatedData {
            expected: 8,
            actual: data.len(),
        });
    }

    let mut cursor = Cursor::new(&data[4..8]);
    let header_size = cursor.read_u32::<BigEndian>().map_err(Error::Io)?;

    if header_size == 0 {
        // Single chunk - can't easily determine decompressed size without parsing
        Ok(EntryMetadata {
            compressed_size: data.len(),
            decompressed_size: None,
            chunk_count: 1,
            validated: false,
        })
    } else {
        // Multi-chunk - parse chunk table for info
        if data.len() < 12 {
            return Err(Error::TruncatedData {
                expected: 12,
                actual: data.len(),
            });
        }

        let chunk_count_bytes = [data[9], data[10], data[11]];
        let chunk_count = u32::from_be_bytes([
            0,
            chunk_count_bytes[0],
            chunk_count_bytes[1],
            chunk_count_bytes[2],
        ]);

        // Try to calculate total decompressed size from chunk table
        let mut total_decompressed = 0u32;
        let mut chunk_offset = 12;

        for _ in 0..chunk_count.min(10) {
            // Limit to first 10 chunks for efficiency
            if chunk_offset + 8 > data.len() {
                break;
            }

            // Skip compressed size (4 bytes), read decompressed size
            let mut cursor = Cursor::new(&data[chunk_offset + 4..chunk_offset + 8]);
            if let Ok(decompressed_size) = cursor.read_u32::<BigEndian>() {
                total_decompressed += decompressed_size;
            }

            // Determine chunk entry size based on flags
            let flags = data[8];
            let chunk_entry_size = match flags {
                0x0F => 24,
                0x10 => 40,
                _ => 24, // Default
            };
            chunk_offset += chunk_entry_size;
        }

        Ok(EntryMetadata {
            compressed_size: data.len(),
            decompressed_size: if total_decompressed > 0 {
                Some(total_decompressed as usize)
            } else {
                None
            },
            chunk_count: chunk_count as usize,
            validated: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_single_chunk_end() {
        // Create test data with BLTE magic at position 20
        let mut data = vec![0u8; 30];
        data[0..4].copy_from_slice(b"BLTE"); // First BLTE at 0
        data[20..24].copy_from_slice(b"BLTE"); // Second BLTE at 20

        let end = find_single_chunk_end(&data).unwrap();
        assert_eq!(end, 20);
    }

    #[test]
    fn test_calculate_blte_size_single_chunk() {
        let mut data = vec![0u8; 30];
        data[0..4].copy_from_slice(b"BLTE");
        // header_size = 0 for single chunk
        data[4..8].copy_from_slice(&0u32.to_be_bytes());

        let size = calculate_blte_size(&data, 0).unwrap();
        assert_eq!(size, 30); // Should go to end of data
    }
}

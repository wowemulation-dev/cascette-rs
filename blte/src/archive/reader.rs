//! Streaming archive reader for memory-efficient processing

use super::ArchiveEntry;
use crate::{BLTEFile, Result};
use std::io::{Read, Seek, SeekFrom};
use tracing::{debug, warn};

/// Streaming reader for memory-efficient archive processing
#[derive(Debug)]
pub struct ArchiveReader<R: Read + Seek> {
    reader: R,
    entries: Vec<ArchiveEntry>,
    current_index: usize,
    current_position: u64,
    scanned: bool,
}

impl<R: Read + Seek> ArchiveReader<R> {
    /// Create streaming reader and scan for BLTE file headers
    pub fn new(reader: R) -> Result<Self> {
        let mut archive_reader = Self {
            reader,
            entries: Vec::new(),
            current_index: 0,
            current_position: 0,
            scanned: false,
        };

        archive_reader.scan_archive()?;
        Ok(archive_reader)
    }

    /// Scan the archive to find all BLTE file headers without loading the content
    fn scan_archive(&mut self) -> Result<()> {
        if self.scanned {
            return Ok(());
        }

        self.reader.seek(SeekFrom::Start(0))?;
        self.current_position = 0;
        self.entries.clear();

        let mut buffer = [0u8; 8];
        let mut _entry_index = 0;

        while let Ok(()) = self.reader.read_exact(&mut buffer) {
            // Check for BLTE magic signature
            if &buffer[0..4] == b"BLTE" {
                // Read the size field (4 bytes after BLTE magic)
                let size = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

                debug!(
                    "Found BLTE file at position {} with size {}",
                    self.current_position, size
                );

                let entry = ArchiveEntry {
                    offset: self.current_position as usize,
                    size: size as usize,
                    blte: None,
                    metadata: super::EntryMetadata {
                        compressed_size: size as usize,
                        decompressed_size: None,
                        chunk_count: 0,
                        validated: false,
                    },
                };

                self.entries.push(entry);
                _entry_index += 1;

                // Skip to next potential file (after the current BLTE data)
                let next_position = self.current_position + 8 + size as u64;
                if self.reader.seek(SeekFrom::Start(next_position)).is_ok() {
                    self.current_position = next_position;
                } else {
                    break;
                }
            } else {
                // Not a BLTE header, advance by 1 byte and try again
                self.current_position += 1;
                self.reader.seek(SeekFrom::Start(self.current_position))?;
            }
        }

        self.scanned = true;
        debug!(
            "Archive scan complete: found {} BLTE files",
            self.entries.len()
        );

        // Reset to beginning for reading
        self.reader.seek(SeekFrom::Start(0))?;
        self.current_position = 0;
        self.current_index = 0;

        Ok(())
    }

    /// Read next BLTE file
    pub fn next_file(&mut self) -> Result<Option<BLTEFile>> {
        if self.current_index >= self.entries.len() {
            return Ok(None);
        }

        let entry = &self.entries[self.current_index];

        // Seek to the file position
        self.reader.seek(SeekFrom::Start(entry.offset as u64))?;

        // Read the entire BLTE file data
        let mut file_data = vec![0u8; entry.size + 8]; // +8 for BLTE header
        self.reader.read_exact(&mut file_data)?;

        // Parse the BLTE file
        match BLTEFile::parse(file_data) {
            Ok(blte_file) => {
                self.current_index += 1;
                Ok(Some(blte_file))
            }
            Err(e) => {
                warn!(
                    "Failed to parse BLTE file at index {}: {}",
                    self.current_index, e
                );
                self.current_index += 1;
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Invalid BLTE file at index {}: {}",
                        self.current_index - 1,
                        e
                    ),
                )
                .into())
            }
        }
    }

    /// Skip to specific file index
    pub fn seek_to_file(&mut self, index: usize) -> Result<()> {
        if index >= self.entries.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "File index {} out of range (max: {})",
                    index,
                    self.entries.len()
                ),
            )
            .into());
        }

        self.current_index = index;
        let entry = &self.entries[index];
        self.reader.seek(SeekFrom::Start(entry.offset as u64))?;
        self.current_position = entry.offset as u64;

        Ok(())
    }

    /// Get file entry information without reading the content
    pub fn get_entry(&self, index: usize) -> Option<&ArchiveEntry> {
        self.entries.get(index)
    }

    /// Get all entries
    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    /// Get total number of files discovered
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// Get current file index
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Reset to beginning of archive
    pub fn reset(&mut self) -> Result<()> {
        self.current_index = 0;
        self.reader.seek(SeekFrom::Start(0))?;
        self.current_position = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_archive_reader_empty() {
        let data = vec![0u8; 1024];
        let cursor = Cursor::new(data);
        let reader = ArchiveReader::new(cursor).unwrap();

        assert_eq!(reader.file_count(), 0);
    }

    #[test]
    fn test_archive_reader_with_blte_header() {
        let mut data = Vec::new();
        data.extend_from_slice(b"BLTE"); // Magic
        data.extend_from_slice(&16u32.to_be_bytes()); // Size = 16 bytes of data after header
        data.extend_from_slice(&[0u8; 16]); // 16 bytes of BLTE data

        let cursor = Cursor::new(data);
        let reader = ArchiveReader::new(cursor).unwrap();

        assert_eq!(reader.file_count(), 1);
        assert_eq!(reader.get_entry(0).unwrap().size, 16);
        assert_eq!(reader.get_entry(0).unwrap().offset, 0);
    }

    #[test]
    fn test_archive_reader_multiple_files() {
        let mut data = Vec::new();

        // First BLTE file
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&[1u8; 8]);

        // Second BLTE file
        data.extend_from_slice(b"BLTE");
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(&[2u8; 12]);

        let cursor = Cursor::new(data);
        let reader = ArchiveReader::new(cursor).unwrap();

        assert_eq!(reader.file_count(), 2);
        assert_eq!(reader.get_entry(0).unwrap().size, 8);
        assert_eq!(reader.get_entry(1).unwrap().size, 12);
        assert_eq!(reader.get_entry(1).unwrap().offset, 16); // After first file (8 + 8 header)
    }

    #[test]
    fn test_seek_to_file() {
        let mut data = Vec::new();

        // Multiple BLTE files
        for i in 0..3 {
            data.extend_from_slice(b"BLTE");
            data.extend_from_slice(&4u32.to_be_bytes());
            data.extend_from_slice(&[i as u8; 4]);
        }

        let cursor = Cursor::new(data);
        let mut reader = ArchiveReader::new(cursor).unwrap();

        assert_eq!(reader.file_count(), 3);

        // Test seeking to different files
        reader.seek_to_file(1).unwrap();
        assert_eq!(reader.current_index(), 1);

        reader.seek_to_file(0).unwrap();
        assert_eq!(reader.current_index(), 0);

        // Test seeking beyond range
        assert!(reader.seek_to_file(3).is_err());
    }
}

//! Archive file writer for creating and appending to archives

use crate::error::Result;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use tracing::debug;

/// Writer for CASC archive files
pub struct ArchiveWriter {
    writer: BufWriter<File>,
    current_offset: u64,
    archive_id: u16,
}

impl ArchiveWriter {
    /// Create a new archive file
    pub fn create(path: &Path, archive_id: u16) -> Result<Self> {
        debug!("Creating new archive: {:?}", path);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            current_offset: 0,
            archive_id,
        })
    }

    /// Open an existing archive for appending
    pub fn append(path: &Path, archive_id: u16) -> Result<Self> {
        debug!("Opening archive for append: {:?}", path);

        let file = OpenOptions::new().append(true).open(path)?;

        let current_offset = file.metadata()?.len();

        Ok(Self {
            writer: BufWriter::new(file),
            current_offset,
            archive_id,
        })
    }

    /// Write data to the archive and return the offset
    pub fn write(&mut self, data: &[u8]) -> Result<u64> {
        let offset = self.current_offset;

        self.writer.write_all(data)?;
        self.current_offset += data.len() as u64;

        debug!(
            "Wrote {} bytes to archive {} at offset {:x}",
            data.len(),
            self.archive_id,
            offset
        );

        Ok(offset)
    }

    /// Write data with alignment padding
    pub fn write_aligned(&mut self, data: &[u8], alignment: u64) -> Result<u64> {
        // Align current offset
        let padding_needed = if self.current_offset % alignment != 0 {
            alignment - (self.current_offset % alignment)
        } else {
            0
        };

        if padding_needed > 0 {
            let padding = vec![0u8; padding_needed as usize];
            self.writer.write_all(&padding)?;
            self.current_offset += padding_needed;
        }

        self.write(data)
    }

    /// Flush the writer
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Get the current offset in the archive
    pub fn current_offset(&self) -> u64 {
        self.current_offset
    }

    /// Get the archive ID
    pub fn archive_id(&self) -> u16 {
        self.archive_id
    }

    /// Seek to a specific position in the archive
    pub fn seek(&mut self, pos: u64) -> Result<()> {
        self.writer.seek(SeekFrom::Start(pos))?;
        self.current_offset = pos;
        Ok(())
    }

    /// Get the current size of the archive
    pub fn size(&mut self) -> Result<u64> {
        self.flush()?;
        let metadata = self.writer.get_ref().metadata()?;
        Ok(metadata.len())
    }
}

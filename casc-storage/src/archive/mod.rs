//! Archive file handling for CASC storage

mod archive_reader;
mod archive_writer;

pub use archive_reader::ArchiveReader;
pub use archive_writer::ArchiveWriter;

use crate::error::Result;
use crate::types::ArchiveLocation;
use std::path::{Path, PathBuf};

/// Represents a CASC archive file (data.XXX)
pub struct Archive {
    /// Archive ID (the XXX in data.XXX)
    pub id: u16,
    /// Path to the archive file
    pub path: PathBuf,
    /// Current size of the archive
    pub size: u64,
    /// Reader for this archive
    reader: Option<ArchiveReader>,
}

impl Archive {
    /// Create a new archive reference
    pub fn new(id: u16, path: PathBuf) -> Result<Self> {
        let size = if path.exists() {
            std::fs::metadata(&path)?.len()
        } else {
            0
        };

        Ok(Self {
            id,
            path,
            size,
            reader: None,
        })
    }

    /// Open the archive for reading
    pub fn open(&mut self) -> Result<&mut ArchiveReader> {
        if self.reader.is_none() {
            self.reader = Some(ArchiveReader::open(&self.path)?);
        }
        Ok(self.reader.as_mut().unwrap())
    }

    /// Read data from a specific location in the archive
    pub fn read_at(&mut self, location: &ArchiveLocation) -> Result<Vec<u8>> {
        let reader = self.open()?;
        reader.read_at(location.offset, location.size as usize)
    }

    /// Check if a file exists at the given location
    pub fn contains(&self, location: &ArchiveLocation) -> bool {
        location.archive_id == self.id && location.offset + location.size as u64 <= self.size
    }

    /// Get the archive filename (e.g., "data.001")
    pub fn filename(&self) -> String {
        format!("data.{:03}", self.id)
    }

    /// Get the path to this archive
    pub fn path(&self) -> &Path {
        &self.path
    }
}

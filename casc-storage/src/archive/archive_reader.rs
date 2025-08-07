//! Archive file reader with memory mapping support

use crate::error::{CascError, Result};
use memmap2::{Mmap, MmapOptions};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use tracing::debug;

/// Reader for CASC archive files with memory mapping support
pub struct ArchiveReader {
    /// Memory-mapped file (if available)
    mmap: Option<Mmap>,
    /// Regular file reader (fallback)
    file: Option<BufReader<File>>,
    /// Size of the archive
    size: u64,
}

impl ArchiveReader {
    /// Open an archive file for reading
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let size = metadata.len();

        debug!("Opening archive: {:?} (size: {} bytes)", path, size);

        // Try to memory-map the file if it's not too large
        let mmap = if size > 0 && size < 2_147_483_648 {
            // Limit mmap to 2GB files
            match unsafe { MmapOptions::new().map(&file) } {
                Ok(mmap) => {
                    debug!("Successfully memory-mapped archive");
                    Some(mmap)
                }
                Err(e) => {
                    debug!("Failed to memory-map archive, using file reader: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // If we couldn't mmap, use a regular file reader
        let file = if mmap.is_none() {
            Some(BufReader::new(file))
        } else {
            None
        };

        Ok(Self { mmap, file, size })
    }

    /// Read data at a specific offset
    pub fn read_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>> {
        if offset + length as u64 > self.size {
            return Err(CascError::InvalidArchiveFormat(format!(
                "Read beyond archive bounds: offset={}, length={}, size={}",
                offset, length, self.size
            )));
        }

        if let Some(ref mmap) = self.mmap {
            // Fast path: memory-mapped access
            let data = &mmap[offset as usize..(offset as usize + length)];
            Ok(data.to_vec())
        } else if let Some(ref mut file) = self.file {
            // Slow path: file read
            file.seek(SeekFrom::Start(offset))?;
            let mut buffer = vec![0u8; length];
            file.read_exact(&mut buffer)?;
            Ok(buffer)
        } else {
            Err(CascError::InvalidArchiveFormat(
                "Archive reader not initialized".into()
            ))
        }
    }

    /// Read a slice of data without allocation (only works with mmap)
    pub fn read_slice(&self, offset: u64, length: usize) -> Result<&[u8]> {
        if offset + length as u64 > self.size {
            return Err(CascError::InvalidArchiveFormat(format!(
                "Read beyond archive bounds: offset={}, length={}, size={}",
                offset, length, self.size
            )));
        }

        if let Some(ref mmap) = self.mmap {
            Ok(&mmap[offset as usize..(offset as usize + length)])
        } else {
            Err(CascError::InvalidArchiveFormat(
                "Memory mapping not available for slice access".into()
            ))
        }
    }

    /// Get the size of the archive
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Check if memory mapping is available
    pub fn is_memory_mapped(&self) -> bool {
        self.mmap.is_some()
    }

    /// Prefetch data into memory (hint to OS)
    pub fn prefetch(&self, offset: u64, length: usize) -> Result<()> {
        if let Some(ref mmap) = self.mmap {
            let start = offset as usize;
            let end = (offset as usize).saturating_add(length).min(mmap.len());
            
            // Advise the OS that we'll need this data soon
            #[cfg(unix)]
            {
                use memmap2::Advice;
                let _ = mmap.advise_range(Advice::WillNeed, start, end - start);
            }
        }
        Ok(())
    }
}
//! Archive file reader with memory mapping support

use crate::error::{CascError, Result};
use memmap2::{Mmap, MmapOptions};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
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

/// A section of an archive that can be streamed
pub struct ArchiveSection<'a> {
    data: Cursor<Cow<'a, [u8]>>,
}

impl<'a> ArchiveSection<'a> {
    pub fn new(data: Cow<'a, [u8]>) -> Self {
        Self {
            data: Cursor::new(data),
        }
    }

    /// Create from owned data
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self {
            data: Cursor::new(Cow::Owned(data)),
        }
    }

    /// Create from borrowed data
    pub fn from_slice(data: &'a [u8]) -> Self {
        Self {
            data: Cursor::new(Cow::Borrowed(data)),
        }
    }
}

impl<'a> Read for ArchiveSection<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buf)
    }
}

impl<'a> Seek for ArchiveSection<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(pos)
    }
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

    /// Create a reader at a specific offset for streaming access (zero-copy when possible)
    pub fn reader_at(&self, offset: u64, length: usize) -> Result<ArchiveSection> {
        if offset + length as u64 > self.size {
            return Err(CascError::InvalidArchiveFormat(format!(
                "Read beyond archive bounds: offset={}, length={}, size={}",
                offset, length, self.size
            )));
        }

        if let Some(ref mmap) = self.mmap {
            // Memory-mapped access - zero copy
            let data = &mmap[offset as usize..(offset as usize + length)];
            Ok(ArchiveSection::from_slice(data))
        } else {
            // For regular file access, we still need to read the data
            // This could be improved later to use a file handle with seeking
            Err(CascError::InvalidArchiveFormat(
                "Streaming from non-memory-mapped archives not yet supported".into(),
            ))
        }
    }

    /// Read data at a specific offset (returns Cow for zero-copy when possible)
    pub fn read_at_cow(&self, offset: u64, length: usize) -> Result<Cow<[u8]>> {
        if offset + length as u64 > self.size {
            return Err(CascError::InvalidArchiveFormat(format!(
                "Read beyond archive bounds: offset={}, length={}, size={}",
                offset, length, self.size
            )));
        }

        if let Some(ref mmap) = self.mmap {
            // Fast path: memory-mapped access - zero copy
            let data = &mmap[offset as usize..(offset as usize + length)];
            Ok(Cow::Borrowed(data))
        } else {
            Err(CascError::InvalidArchiveFormat(
                "Non-memory-mapped archives require mutable access".into(),
            ))
        }
    }

    /// Read data at a specific offset (allocates for compatibility)
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
                "Archive reader not initialized".into(),
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
                "Memory mapping not available for slice access".into(),
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

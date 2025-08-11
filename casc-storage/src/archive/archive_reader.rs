//! Archive file reader with memory mapping support

use crate::error::{CascError, Result};
use memmap2::{Mmap, MmapOptions};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::debug;

/// Reader for CASC archive files with memory mapping support
pub struct ArchiveReader {
    /// Memory-mapped file (if available)
    mmap: Option<Mmap>,
    /// Regular file reader (fallback)
    file: Option<BufReader<File>>,
    /// Path to the archive file (for large file fallback)
    path: Arc<PathBuf>,
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

impl Read for ArchiveSection<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buf)
    }
}

impl Seek for ArchiveSection<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(pos)
    }
}

impl ArchiveReader {
    /// Determine if we can memory map a file of this size
    pub fn can_memory_map(size: u64) -> bool {
        // Platform-specific memory mapping limits
        #[cfg(target_pointer_width = "64")]
        {
            // On 64-bit systems, we can handle much larger files
            // Practical limit is around 128GB to avoid excessive virtual memory usage
            const MAX_MMAP_SIZE: u64 = 128 * 1024 * 1024 * 1024; // 128GB
            size <= MAX_MMAP_SIZE
        }

        #[cfg(target_pointer_width = "32")]
        {
            // On 32-bit systems, stick to 2GB limit due to address space constraints
            const MAX_MMAP_SIZE_32BIT: u64 = 2 * 1024 * 1024 * 1024; // 2GB
            size <= MAX_MMAP_SIZE_32BIT
        }
    }

    /// Open an archive file for reading
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let size = metadata.len();
        let path = Arc::new(path.to_path_buf());

        debug!("Opening archive: {:?} (size: {} bytes)", path, size);

        // Try to memory-map the file (support for large archives >2GB)
        let mmap = if size > 0 && Self::can_memory_map(size) {
            // SAFETY: The file handle is valid and will remain open for the lifetime of the mmap.
            // The mmap is read-only and the file won't be modified while mapped.
            match unsafe { MmapOptions::new().map(&file) } {
                Ok(mmap) => {
                    debug!("Successfully memory-mapped archive ({} bytes)", size);
                    Some(mmap)
                }
                Err(e) => {
                    debug!("Failed to memory-map archive, using file reader: {}", e);
                    None
                }
            }
        } else if size > 0 {
            debug!(
                "Archive too large for memory mapping ({} bytes), using file reader",
                size
            );
            None
        } else {
            None
        };

        // If we couldn't mmap, use a regular file reader
        let file = if mmap.is_none() {
            Some(BufReader::new(file))
        } else {
            None
        };

        Ok(Self {
            mmap,
            file,
            path,
            size,
        })
    }

    /// Create a reader at a specific offset for streaming access (zero-copy when possible)
    pub fn reader_at(&self, offset: u64, length: usize) -> Result<ArchiveSection<'_>> {
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
            // For large archives without mmap, read the data into a buffer
            let mut data = vec![0u8; length];
            self.read_at_fallback(offset, &mut data)?;
            Ok(ArchiveSection::from_vec(data))
        }
    }

    /// Read data at a specific offset (returns Cow for zero-copy when possible)
    pub fn read_at_cow(&self, offset: u64, length: usize) -> Result<Cow<'_, [u8]>> {
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
            // For large archives without mmap, read into owned data
            let mut data = vec![0u8; length];
            self.read_at_fallback(offset, &mut data)?;
            Ok(Cow::Owned(data))
        }
    }

    /// Fallback method for reading from non-memory-mapped files
    fn read_at_fallback(&self, offset: u64, buffer: &mut [u8]) -> Result<()> {
        // For large archives that can't be memory-mapped, use platform-specific optimizations

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;

            // Use pread for thread-safe positioned reads without seeking
            let file = File::open(&*self.path)?;
            file.read_exact_at(buffer, offset)?;
            Ok(())
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;

            // Windows positioned read
            let file = File::open(&*self.path)?;
            let bytes_read = file.seek_read(buffer, offset)?;
            if bytes_read != buffer.len() {
                return Err(CascError::InvalidArchiveFormat(
                    "Incomplete read from archive".into(),
                ));
            }
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback for other platforms - not thread-safe but functional
            use std::io::{BufRead, BufReader};

            let file = File::open(&*self.path)?;
            let mut reader = BufReader::new(file);
            reader.seek(SeekFrom::Start(offset))?;
            reader.read_exact(buffer)?;
            Ok(())
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
            // Traditional file read (for smaller files or when mmap failed)
            file.seek(SeekFrom::Start(offset))?;
            let mut buffer = vec![0u8; length];
            file.read_exact(&mut buffer)?;
            Ok(buffer)
        } else {
            // Large archive fallback - use positioned reads
            let mut buffer = vec![0u8; length];
            self.read_at_fallback(offset, &mut buffer)?;
            Ok(buffer)
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
    #[allow(unused_variables)] // `offset` and `length` are only used on Unix
    pub fn prefetch(&self, offset: u64, length: usize) -> Result<()> {
        if let Some(ref mmap) = self.mmap {
            // Advise the OS that we'll need this data soon
            #[cfg(unix)]
            {
                let start = offset as usize;
                let end = (offset as usize).saturating_add(length).min(mmap.len());
                use memmap2::Advice;
                let _ = mmap.advise_range(Advice::WillNeed, start, end - start);
            }
        }
        Ok(())
    }
}

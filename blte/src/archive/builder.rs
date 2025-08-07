//! Archive creation and building

use crate::{BLTEFile, CompressionMode, Result};

/// Builder for creating BLTE archives
#[derive(Debug)]
pub struct ArchiveBuilder {
    files: Vec<BLTEFile>,
    target_size: usize, // Default 256MB
    current_size: usize,
}

impl ArchiveBuilder {
    /// Create new archive builder with 256MB target size
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            target_size: 256 * 1024 * 1024, // 256MB
            current_size: 0,
        }
    }

    /// Set target archive size
    pub fn target_size(mut self, size: usize) -> Self {
        self.target_size = size;
        self
    }

    /// Add BLTE file to archive
    pub fn add_file(&mut self, blte: BLTEFile) -> Result<bool> {
        let file_size = blte.total_size();

        // Check if adding this file would exceed target size
        if self.current_size + file_size > self.target_size && !self.files.is_empty() {
            return Ok(false); // Archive is full
        }

        self.current_size += file_size;
        self.files.push(blte);
        Ok(true) // Successfully added
    }

    /// Add raw data as BLTE file
    pub fn add_data(&mut self, data: Vec<u8>, mode: CompressionMode) -> Result<bool> {
        let blte = crate::compress::compress_data_single(data, mode, None)?;
        let blte_file = BLTEFile::parse(blte)?;
        self.add_file(blte_file)
    }

    /// Build final archive
    pub fn build(self) -> Result<Vec<u8>> {
        let mut archive_data = Vec::with_capacity(self.current_size);

        for blte in self.files {
            archive_data.extend_from_slice(&blte.raw_data());
        }

        Ok(archive_data)
    }

    /// Get current archive size
    pub fn current_size(&self) -> usize {
        self.current_size
    }

    /// Get number of files in archive
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Check if archive has capacity for more data
    pub fn has_capacity(&self, additional_size: usize) -> bool {
        self.current_size + additional_size <= self.target_size
    }
}

impl Default for ArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Split large datasets into multiple 256MB archives
#[derive(Debug)]
pub struct MultiArchiveBuilder {
    current_archive: ArchiveBuilder,
    completed_archives: Vec<Vec<u8>>,
    max_archive_size: usize,
}

impl MultiArchiveBuilder {
    /// Create new multi-archive builder
    pub fn new() -> Self {
        Self {
            current_archive: ArchiveBuilder::new(),
            completed_archives: Vec::new(),
            max_archive_size: 256 * 1024 * 1024,
        }
    }

    /// Set maximum size per archive
    pub fn max_archive_size(mut self, size: usize) -> Self {
        self.max_archive_size = size;
        self.current_archive = self.current_archive.target_size(size);
        self
    }

    /// Add data, automatically splitting into archives as needed
    pub fn add_data(&mut self, data: Vec<u8>, mode: CompressionMode) -> Result<()> {
        let blte = crate::compress::compress_data_single(data, mode, None)?;
        let blte_file = BLTEFile::parse(blte)?;

        // Try to add to current archive
        if !self.current_archive.add_file(blte_file.clone())? {
            // Current archive is full, finalize it and start new one
            let completed_archive = std::mem::replace(
                &mut self.current_archive,
                ArchiveBuilder::new().target_size(self.max_archive_size),
            );

            self.completed_archives.push(completed_archive.build()?);

            // Add to new archive (this should succeed since we just created it)
            self.current_archive.add_file(blte_file)?;
        }

        Ok(())
    }

    /// Finalize all archives
    pub fn finalize(mut self) -> Result<Vec<Vec<u8>>> {
        // Finalize current archive if it has content
        if self.current_archive.file_count() > 0 {
            self.completed_archives.push(self.current_archive.build()?);
        }

        Ok(self.completed_archives)
    }

    /// Get number of completed archives
    pub fn archive_count(&self) -> usize {
        self.completed_archives.len()
            + if self.current_archive.file_count() > 0 {
                1
            } else {
                0
            }
    }
}

impl Default for MultiArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_builder_creation() {
        let builder = ArchiveBuilder::new();
        assert_eq!(builder.target_size, 256 * 1024 * 1024);
        assert_eq!(builder.current_size, 0);
        assert_eq!(builder.file_count(), 0);
    }

    #[test]
    fn test_archive_builder_custom_size() {
        let builder = ArchiveBuilder::new().target_size(1024 * 1024);
        assert_eq!(builder.target_size, 1024 * 1024);
    }

    #[test]
    fn test_multi_archive_builder() {
        let builder = MultiArchiveBuilder::new();
        assert_eq!(builder.max_archive_size, 256 * 1024 * 1024);
        assert_eq!(builder.archive_count(), 0);
    }
}

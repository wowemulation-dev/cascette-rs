//! BLTE Archive support for 256MB CDN files
//!
//! Blizzard's CDN serves game content as 256MB archive files containing
//! multiple concatenated BLTE files. This module provides functionality
//! to parse, extract, and create these archives.

use crate::{BLTEFile, Result};
use std::time::SystemTime;

pub mod builder;
pub mod parser;
pub mod reader;
pub mod recreation;
pub mod validation;

/// Represents a 256MB CDN archive containing multiple BLTE files
#[derive(Debug)]
pub struct BLTEArchive {
    /// Individual BLTE files with their positions
    files: Vec<ArchiveEntry>,
    /// Raw archive data (optional, for in-memory archives)
    data: Option<Vec<u8>>,
    /// Archive metadata
    metadata: ArchiveMetadata,
}

/// Entry in a BLTE archive
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Offset in archive where this BLTE starts
    pub offset: usize,
    /// Size of this BLTE file (header + data)
    pub size: usize,
    /// Parsed BLTE file (lazy loaded)
    blte: Option<BLTEFile>,
    /// Entry metadata
    pub metadata: EntryMetadata,
}

/// Archive-level metadata
#[derive(Debug, Clone, Default)]
pub struct ArchiveMetadata {
    /// Number of BLTE files
    pub file_count: usize,
    /// Total compressed size
    pub compressed_size: u64,
    /// Total decompressed size (calculated when all files are parsed)
    pub decompressed_size: Option<u64>,
    /// Archive creation time (if available)
    pub created: Option<SystemTime>,
}

/// Individual entry metadata
#[derive(Debug, Clone)]
pub struct EntryMetadata {
    /// Compressed size of this BLTE file
    pub compressed_size: usize,
    /// Decompressed size (if known)
    pub decompressed_size: Option<usize>,
    /// Number of chunks in this BLTE
    pub chunk_count: usize,
    /// Whether this entry has been validated
    pub validated: bool,
}

/// Archive statistics
#[derive(Debug, Clone)]
pub struct ArchiveStats {
    /// Total number of files
    pub file_count: usize,
    /// Total archive size
    pub total_size: usize,
    /// Total compressed data size
    pub compressed_size: u64,
    /// Total decompressed data size (if calculated)
    pub decompressed_size: Option<u64>,
    /// Compression ratio as percentage
    pub compression_ratio: Option<f64>,
    /// Distribution of compression modes
    pub compression_modes: CompressionModeStats,
    /// File size distribution
    pub size_distribution: SizeDistribution,
}

/// Statistics about compression modes used
#[derive(Debug, Clone, Default)]
pub struct CompressionModeStats {
    /// Number of files using no compression
    pub none_count: usize,
    /// Number of files using ZLib compression  
    pub zlib_count: usize,
    /// Number of files using LZ4 compression
    pub lz4_count: usize,
    /// Number of files using encryption
    pub encrypted_count: usize,
    /// Number of files with unknown compression
    pub unknown_count: usize,
}

/// File size distribution statistics
#[derive(Debug, Clone)]
pub struct SizeDistribution {
    /// Smallest file size
    pub min_size: usize,
    /// Largest file size  
    pub max_size: usize,
    /// Average file size
    pub avg_size: f64,
    /// Median file size
    pub median_size: usize,
    /// Standard deviation
    pub std_dev: f64,
}

impl BLTEArchive {
    /// Parse concatenated BLTE archive from 256MB CDN file
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        parser::parse_archive(data)
    }

    /// Get number of BLTE files in archive
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get archive statistics
    pub fn stats(&self) -> ArchiveStats {
        self.calculate_stats()
    }

    /// Get file info without loading full BLTE
    pub fn file_info(&self, index: usize) -> Result<&ArchiveEntry> {
        self.files
            .get(index)
            .ok_or(crate::Error::InvalidChunkCount(index as u32))
    }

    /// Get BLTE file by index (loads if not cached)
    pub fn get_file(&mut self, index: usize) -> Result<&BLTEFile> {
        if index >= self.files.len() {
            return Err(crate::Error::InvalidChunkCount(index as u32));
        }

        // Load BLTE if not already cached
        if self.files[index].blte.is_none() {
            let entry = &self.files[index];
            if let Some(ref data) = self.data {
                let blte_data = data[entry.offset..entry.offset + entry.size].to_vec();
                let blte = BLTEFile::parse(blte_data)?;
                self.files[index].blte = Some(blte);
            } else {
                return Err(crate::Error::TruncatedData {
                    expected: entry.size,
                    actual: 0,
                });
            }
        }

        Ok(self.files[index].blte.as_ref().unwrap())
    }

    /// Extract and decompress file by index
    pub fn extract_file(&mut self, index: usize) -> Result<Vec<u8>> {
        let blte = self.get_file(index)?;
        crate::decompress_blte(blte.raw_data(), None)
    }

    /// Extract file with complete metadata preservation for perfect recreation
    pub fn extract_file_with_metadata(
        &mut self,
        index: usize,
    ) -> Result<recreation::ExtractedFile> {
        // Get entry info first (immutable borrow)
        let entry_offset = self.file_info(index)?.offset;
        let entry_size = self.file_info(index)?.size;

        // Then get BLTE file (mutable borrow)
        let blte = self.get_file(index)?;

        recreation::ExtractedFile::from_blte(index, blte, entry_offset, entry_size)
    }

    /// Extract ALL files preserving order and metadata for perfect recreation
    pub fn extract_all_with_metadata(&mut self) -> Result<Vec<recreation::ExtractedFile>> {
        let mut extracted = Vec::with_capacity(self.file_count());

        println!(
            "Extracting {} files with metadata preservation...",
            self.file_count()
        );
        for i in 0..self.file_count() {
            if i % 1000 == 0 {
                println!("  Progress: {}/{} files", i, self.file_count());
            }
            extracted.push(self.extract_file_with_metadata(i)?);
        }
        println!(
            "  Completed: {}/{} files extracted",
            extracted.len(),
            self.file_count()
        );

        Ok(extracted)
    }

    /// Calculate comprehensive archive statistics
    fn calculate_stats(&self) -> ArchiveStats {
        let file_count = self.files.len();
        let total_size = self.data.as_ref().map(|d| d.len()).unwrap_or(0);
        let compressed_size = self.metadata.compressed_size;
        let decompressed_size = self.metadata.decompressed_size;

        let compression_ratio = if let Some(decomp) = decompressed_size {
            if decomp > 0 {
                Some((compressed_size as f64 / decomp as f64) * 100.0)
            } else {
                None
            }
        } else {
            None
        };

        // Calculate file size statistics
        let sizes: Vec<usize> = self.files.iter().map(|f| f.size).collect();
        let min_size = sizes.iter().min().copied().unwrap_or(0);
        let max_size = sizes.iter().max().copied().unwrap_or(0);
        let avg_size = if !sizes.is_empty() {
            sizes.iter().sum::<usize>() as f64 / sizes.len() as f64
        } else {
            0.0
        };

        let median_size = if !sizes.is_empty() {
            let mut sorted_sizes = sizes.clone();
            sorted_sizes.sort_unstable();
            sorted_sizes[sorted_sizes.len() / 2]
        } else {
            0
        };

        let std_dev = if sizes.len() > 1 {
            let variance = sizes
                .iter()
                .map(|&size| (size as f64 - avg_size).powi(2))
                .sum::<f64>()
                / sizes.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // Calculate compression mode statistics from actual files
        let compression_modes = CompressionModeStats::default();

        // For now, compression mode stats are not calculated from files
        // since ArchiveEntry doesn't store chunk information directly
        // This would require parsing each BLTE file, which is expensive
        // TODO: Add optional compression mode analysis

        ArchiveStats {
            file_count,
            total_size,
            compressed_size,
            decompressed_size,
            compression_ratio,
            compression_modes,
            size_distribution: SizeDistribution {
                min_size,
                max_size,
                avg_size,
                median_size,
                std_dev,
            },
        }
    }
}

impl ArchiveEntry {
    /// Create new archive entry
    pub fn new(offset: usize, size: usize) -> Self {
        Self {
            offset,
            size,
            blte: None,
            metadata: EntryMetadata {
                compressed_size: size,
                decompressed_size: None,
                chunk_count: 0,
                validated: false,
            },
        }
    }

    /// Check if BLTE file is loaded
    pub fn is_loaded(&self) -> bool {
        self.blte.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_entry_creation() {
        let entry = ArchiveEntry::new(100, 500);
        assert_eq!(entry.offset, 100);
        assert_eq!(entry.size, 500);
        assert!(!entry.is_loaded());
    }

    #[test]
    fn test_archive_metadata_default() {
        let metadata = ArchiveMetadata::default();
        assert_eq!(metadata.file_count, 0);
        assert_eq!(metadata.compressed_size, 0);
        assert!(metadata.decompressed_size.is_none());
    }
}

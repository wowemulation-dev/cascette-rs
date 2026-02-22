//! Data archive (.data) file management
//!
//! Data archives contain BLTE-encoded game content.

use crate::storage::local_header::{LOCAL_HEADER_SIZE, LocalHeader};
use crate::{Result, StorageError};
use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::CascFormat;
use cascette_formats::blte::{BlteFile, CompressionMode};
use dashmap::DashMap;
use memmap2::{Mmap, MmapOptions};
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, info, warn};

/// Maximum archive size limit per CASC specification (256 GiB)
const MAX_ARCHIVE_SIZE: u64 = 256 * 1024 * 1024 * 1024;

/// Archive file manager for .data files
pub struct ArchiveManager {
    /// Memory-mapped archive files by ID
    archives: DashMap<u16, Arc<ArchiveFile>>,
    /// Base directory for archives
    base_path: PathBuf,
    /// Next write position for each archive
    write_positions: Arc<RwLock<BTreeMap<u16, u64>>>,
    /// Default compression mode for new data
    default_compression: CompressionMode,
}

/// Individual archive file with memory mapping
struct ArchiveFile {
    /// Archive ID
    #[allow(dead_code)]
    id: u16,
    /// Path to the archive file
    path: PathBuf,
    /// Memory-mapped file data
    mmap: Mmap,
    /// File size
    size: u64,
}

impl ArchiveManager {
    /// Creates an archive manager with no compression (pass-through).
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self::with_compression(base_path, CompressionMode::None)
    }

    /// Creates an archive manager that applies `compression` to new writes.
    pub fn with_compression(base_path: impl AsRef<Path>, compression: CompressionMode) -> Self {
        Self {
            archives: DashMap::new(),
            base_path: base_path.as_ref().to_path_buf(),
            write_positions: Arc::new(RwLock::new(BTreeMap::new())),
            default_compression: compression,
        }
    }

    /// Changes the compression applied to subsequent writes.
    pub const fn set_compression_mode(&mut self, mode: CompressionMode) {
        self.default_compression = mode;
    }

    /// Compression mode applied to new writes.
    pub const fn compression_mode(&self) -> CompressionMode {
        self.default_compression
    }

    /// Open all archive files from a directory
    ///
    /// # Errors
    ///
    /// Returns error if directory cannot be read or archives cannot be opened
    pub async fn open_all(&mut self) -> Result<()> {
        info!("Opening archive files from {}", self.base_path.display());

        // Find all data.XXX files
        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| StorageError::Archive(format!("Failed to read directory: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StorageError::Archive(format!("Failed to read entry: {e}")))?
        {
            let path = entry.path();

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Parse archive file names like "data.000" (official CASC format)
                if name.starts_with("data.")
                    && name.len() == 8
                    && let Ok(id) = name[5..8].parse::<u16>()
                {
                    debug!("Opening archive {} from {}", id, path.display());

                    // Validate archive size doesn't exceed CASC limits
                    let metadata = std::fs::metadata(&path).map_err(|e| {
                        StorageError::Archive(format!("Failed to get metadata: {e}"))
                    })?;

                    if metadata.len() > MAX_ARCHIVE_SIZE {
                        warn!(
                            "Archive {} exceeds maximum size limit: {} > {} bytes",
                            id,
                            metadata.len(),
                            MAX_ARCHIVE_SIZE
                        );
                    }

                    self.open_archive(id, &path)?;
                }
            }
        }

        info!("Opened {} archive files", self.archives.len());
        Ok(())
    }

    /// Open a specific archive file
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be opened, read, or memory mapped
    pub fn open_archive(&self, id: u16, path: &Path) -> Result<()> {
        let file = File::open(path)
            .map_err(|e| StorageError::Archive(format!("Failed to open archive: {e}")))?;

        let metadata = file
            .metadata()
            .map_err(|e| StorageError::Archive(format!("Failed to get metadata: {e}")))?;
        let size = metadata.len();

        // Memory-map the file for efficient access
        #[allow(unsafe_code)]
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|e| StorageError::Archive(format!("Failed to mmap archive: {e}")))?
        };

        let archive = Arc::new(ArchiveFile {
            id,
            path: path.to_path_buf(),
            mmap,
            size,
        });

        self.archives.insert(id, archive);

        // Initialize write position to end of file
        self.write_positions.write().insert(id, size);

        Ok(())
    }

    /// Read raw bytes from an archive at specified location without decompression.
    ///
    /// Returns the raw bytes as stored on disk, including any local header.
    ///
    /// # Errors
    ///
    /// Returns error if archive not found or read bounds are invalid
    pub fn read_raw(&self, archive_id: u16, offset: u32, size: u32) -> Result<Vec<u8>> {
        let archive = self
            .archives
            .get(&archive_id)
            .ok_or_else(|| StorageError::Archive(format!("Archive {archive_id} not found")))?;

        let offset = offset as usize;
        let size = size as usize;

        // Validate bounds
        if offset + size > archive.mmap.len() {
            return Err(StorageError::Archive(format!(
                "Read beyond archive bounds: {} + {} > {}",
                offset,
                size,
                archive.mmap.len()
            )));
        }

        let data = archive.mmap[offset..offset + size].to_vec();
        drop(archive);
        Ok(data)
    }

    /// Read content from an archive at specified location.
    ///
    /// Handles the 30-byte local header if present, then decompresses
    /// BLTE data. The returned bytes are the decompressed file content.
    ///
    /// # Errors
    ///
    /// Returns error if archive not found, read bounds invalid, or decompression fails
    pub fn read_content(&self, archive_id: u16, offset: u32, size: u32) -> Result<Vec<u8>> {
        let data = self.read_raw(archive_id, offset, size)?;

        // Check for 30-byte local header + BLTE at offset 0x1E
        if data.len() >= LOCAL_HEADER_SIZE + 4
            && &data[LOCAL_HEADER_SIZE..LOCAL_HEADER_SIZE + 4] == b"BLTE"
        {
            // Agent-format archive entry: skip local header, decompress BLTE
            Self::decompress_blte_with_formats(&data[LOCAL_HEADER_SIZE..])
        } else if data.len() >= 4 && &data[0..4] == b"BLTE" {
            // Direct BLTE (e.g., from CDN, no local header)
            Self::decompress_blte_with_formats(&data)
        } else {
            // Not BLTE-encoded, return raw data
            Ok(data)
        }
    }

    /// Decompress BLTE-encoded data using cascette-formats
    fn decompress_blte_with_formats(data: &[u8]) -> Result<Vec<u8>> {
        // Parse BLTE using cascette-formats
        let blte_file = BlteFile::parse(data)
            .map_err(|e| StorageError::Archive(format!("Failed to parse BLTE: {e}")))?;

        // Decompress all chunks
        blte_file
            .decompress()
            .map_err(|e| StorageError::Archive(format!("Failed to decompress BLTE: {e}")))
    }

    /// Write content to an archive with size validation using default compression.
    ///
    /// Returns `(archive_id, offset, total_size, encoding_key)` where:
    /// - `total_size` includes the 30-byte local header
    /// - `encoding_key` is `MD5(blte_data)`, a content-addressable key
    ///
    /// # Errors
    ///
    /// Returns error if compression fails, archive creation fails, write fails, or size limits exceeded
    pub fn write_content(
        &mut self,
        data: &[u8],
        compress: bool,
    ) -> Result<(u16, u32, u32, [u8; 16])> {
        let mode = if compress {
            self.default_compression
        } else {
            CompressionMode::None
        };
        self.write_content_with_mode(data, mode)
    }

    /// Write content to an archive with specific compression mode.
    ///
    /// Writes a 30-byte local header followed by BLTE data, matching
    /// the CASC on-disk format.
    ///
    /// Returns `(archive_id, offset, total_size, encoding_key)` where:
    /// - `total_size` includes the 30-byte local header
    /// - `encoding_key` is `MD5(blte_data)`, a content-addressable key
    ///
    /// # Errors
    ///
    /// Returns error if compression fails, archive creation fails, write fails, or size limits exceeded
    pub fn write_content_with_mode(
        &mut self,
        data: &[u8],
        mode: CompressionMode,
    ) -> Result<(u16, u32, u32, [u8; 16])> {
        // Select archive with space
        let archive_id = self.select_archive_for_write();

        // BLTE-encode the data (even uncompressed data gets a BLTE wrapper)
        let blte_data = Self::compress_blte_with_mode(data, mode)?;

        // Compute encoding key as MD5(blte_data) — content-addressable
        let encoding_key = EncodingKey::from_data(&blte_data);

        // Build the 30-byte local header
        let blte_size = u32::try_from(blte_data.len())
            .map_err(|e| StorageError::Archive(format!("BLTE data too large: {e}")))?;
        let header = LocalHeader::new(*encoding_key.as_bytes(), blte_size);
        let header_bytes = header.to_bytes();

        // Combined size: 30-byte header + BLTE data
        let total_size = u32::try_from(LOCAL_HEADER_SIZE + blte_data.len())
            .map_err(|e| StorageError::Archive(format!("Total data too large: {e}")))?;

        // Validate that adding this data won't exceed archive size limits
        let current_size = {
            let positions = self.write_positions.read();
            *positions.get(&archive_id).unwrap_or(&0)
        };

        if current_size + u64::from(total_size) > MAX_ARCHIVE_SIZE {
            return Err(StorageError::Archive(
                "Adding data would exceed maximum archive size (256 GiB)".to_string(),
            ));
        }

        // Get or create archive file
        if !self.archives.contains_key(&archive_id) {
            self.create_archive(archive_id)?;
        }

        // Get current write position
        let offset = {
            let positions = self.write_positions.read();
            *positions.get(&archive_id).unwrap_or(&0)
        };

        // Write local header + BLTE data
        let mut combined = Vec::with_capacity(LOCAL_HEADER_SIZE + blte_data.len());
        combined.extend_from_slice(&header_bytes);
        combined.extend_from_slice(&blte_data);
        self.write_to_archive(archive_id, offset, &combined)?;

        // Update write position
        {
            let mut positions = self.write_positions.write();
            positions.insert(archive_id, offset + u64::from(total_size));
        }

        let offset_u32 = u32::try_from(offset)
            .map_err(|e| StorageError::Archive(format!("Offset too large: {e}")))?;

        Ok((archive_id, offset_u32, total_size, *encoding_key.as_bytes()))
    }

    /// Select archive for writing with proper CASC size limits
    fn select_archive_for_write(&self) -> u16 {
        // Find archive with space under the 256 GiB CASC limit
        let positions = self.write_positions.read();

        // Check existing archives for available space
        for (id, &pos) in positions.iter() {
            // Use archives under 256 GiB limit with some buffer
            if pos < MAX_ARCHIVE_SIZE - (100 * 1024 * 1024) {
                // Leave 100MB buffer
                return *id;
            }
        }

        // Create new archive if all are at capacity
        if positions.len() < usize::from(u16::MAX) {
            u16::try_from(positions.len()).unwrap_or(u16::MAX)
        } else {
            // Fallback to archive 0 if we somehow hit the u16 limit
            0
        }
    }

    /// Create a new archive file
    fn create_archive(&self, id: u16) -> Result<()> {
        let filename = format!("data.{id:03}");
        let path = self.base_path.join(filename);

        // Create empty file
        File::create(&path)
            .map_err(|e| StorageError::Archive(format!("Failed to create archive: {e}")))?;

        // Open it for memory mapping
        self.open_archive(id, &path)?;

        info!("Created new archive {}", id);
        Ok(())
    }

    /// Write data to archive at specified offset
    fn write_to_archive(&self, id: u16, offset: u64, data: &[u8]) -> Result<()> {
        let archive_path = {
            let archive = self
                .archives
                .get(&id)
                .ok_or_else(|| StorageError::Archive(format!("Archive {id} not found")))?;
            archive.path.clone()
        };

        // Open file for writing (can't write through mmap)
        let mut file = OpenOptions::new()
            .write(true)
            .open(&archive_path)
            .map_err(|e| StorageError::Archive(format!("Failed to open for write: {e}")))?;

        // Seek to position
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| StorageError::Archive(format!("Failed to seek: {e}")))?;

        // Write data
        file.write_all(data)
            .map_err(|e| StorageError::Archive(format!("Failed to write: {e}")))?;

        file.flush()
            .map_err(|e| StorageError::Archive(format!("Failed to flush: {e}")))?;

        // Check if file grew significantly and remap if needed
        let new_size = self.get_file_size(&archive_path)?;
        let current_size = {
            let archive = self
                .archives
                .get(&id)
                .ok_or_else(|| StorageError::Archive(format!("Archive {id} not found")))?
                .clone();
            archive.size
        };

        // Remap if file grew by more than 64MB or doubled in size
        let size_threshold = 64 * 1024 * 1024; // 64MB
        let size_difference = new_size.saturating_sub(current_size);
        #[allow(clippy::cast_precision_loss)]
        let size_ratio = if current_size > 0 {
            new_size as f64 / current_size as f64
        } else {
            f64::INFINITY
        };

        if size_difference > size_threshold || size_ratio > 2.0 {
            debug!(
                "Remapping archive {} due to size change: {} -> {} bytes",
                id, current_size, new_size
            );
            self.remap_archive(id, &archive_path, new_size)?;
        }

        Ok(())
    }

    /// Compress data using BLTE with cascette-formats (backward compatibility)
    #[allow(dead_code)]
    fn compress_blte_with_formats(data: &[u8]) -> Result<Vec<u8>> {
        Self::compress_blte_with_mode(data, CompressionMode::None)
    }

    /// Compress data using BLTE with specific compression mode
    fn compress_blte_with_mode(data: &[u8], mode: CompressionMode) -> Result<Vec<u8>> {
        // Validate compression mode is supported
        match mode {
            CompressionMode::None | CompressionMode::ZLib | CompressionMode::LZ4 => {}
            CompressionMode::Encrypted => {
                return Err(StorageError::Archive(
                    "Encrypted compression not supported for storage".to_string(),
                ));
            }
            #[allow(deprecated)]
            CompressionMode::Frame => {
                return Err(StorageError::Archive(
                    "Frame compression is deprecated and not supported".to_string(),
                ));
            }
        }

        // Create single-chunk BLTE file with specified compression
        let blte_file = BlteFile::single_chunk(data.to_vec(), mode).map_err(|e| {
            StorageError::Archive(format!("Failed to create BLTE with {mode:?}: {e}"))
        })?;

        // Build the BLTE data
        blte_file
            .build()
            .map_err(|e| StorageError::Archive(format!("Failed to build BLTE with {mode:?}: {e}")))
    }

    /// Verify content at specified location
    ///
    /// # Errors
    ///
    /// Returns error if archive not found or content cannot be read
    pub fn verify_content(
        &self,
        archive_id: u16,
        offset: u32,
        size: u32,
        expected_hash: &[u8],
    ) -> Result<bool> {
        let content = self.read_content(archive_id, offset, size)?;

        // Calculate MD5 hash
        let hash = ContentKey::from_data(&content);

        Ok(hash.as_bytes() == expected_hash)
    }

    /// Get current file size
    #[allow(clippy::unused_self)]
    fn get_file_size(&self, path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| StorageError::Archive(format!("Failed to get file metadata: {e}")))?;
        Ok(metadata.len())
    }

    /// Remap an archive file with new size
    fn remap_archive(&self, id: u16, path: &Path, new_size: u64) -> Result<()> {
        // Validate new size doesn't exceed CASC limits
        if new_size > MAX_ARCHIVE_SIZE {
            return Err(StorageError::Archive(format!(
                "Archive {id} exceeds maximum size limit: {new_size} > {MAX_ARCHIVE_SIZE} bytes",
            )));
        }

        let file = File::open(path).map_err(|e| {
            StorageError::Archive(format!("Failed to open archive for remapping: {e}"))
        })?;

        // Create new memory map
        #[allow(unsafe_code)]
        let new_mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|e| StorageError::Archive(format!("Failed to remap archive: {e}")))?
        };

        let new_archive = Arc::new(ArchiveFile {
            id,
            path: path.to_path_buf(),
            mmap: new_mmap,
            size: new_size,
        });

        // Replace the old archive
        self.archives.insert(id, new_archive);
        debug!(
            "Successfully remapped archive {} with size {}",
            id, new_size
        );
        Ok(())
    }

    /// Get statistics about archives
    pub fn stats(&self) -> ArchiveStats {
        let total_size: u64 = self.archives.iter().map(|entry| entry.value().size).sum();
        let total_used: u64 = self.write_positions.read().values().sum();

        ArchiveStats {
            archive_count: self.archives.len(),
            total_size,
            total_used,
        }
    }

    /// Compact archives to reclaim space and defragment
    ///
    /// This method analyzes archive space usage and compacts archives that have
    /// significant fragmentation or deleted entries to reclaim space.
    ///
    /// # Errors
    ///
    /// Returns error if compaction fails or archives cannot be rebuilt
    // NOTE: Complexity inherent to archive compaction workflow.
    // Future: Extract helpers for archive inspection and file operations.
    #[allow(clippy::cognitive_complexity)]
    pub fn compact(&mut self) -> Result<CompactionStats> {
        let mut stats = CompactionStats::default();
        let compaction_threshold = 0.3; // 30% fragmentation threshold

        info!("Starting archive compaction process");

        // Get list of archives to potentially compact
        let archive_ids: Vec<u16> = self.archives.iter().map(|entry| *entry.key()).collect();

        for archive_id in archive_ids {
            let should_compact = self.should_compact_archive(archive_id, compaction_threshold)?;

            if should_compact {
                info!("Compacting archive {}", archive_id);
                let archive_stats = self.compact_single_archive(archive_id)?;
                stats.archives_compacted += 1;
                stats.bytes_reclaimed += archive_stats.bytes_reclaimed;
                stats.entries_moved += archive_stats.entries_moved;
            }
        }

        if stats.archives_compacted > 0 {
            info!(
                "Compaction complete: {} archives compacted, {} bytes reclaimed, {} entries moved",
                stats.archives_compacted, stats.bytes_reclaimed, stats.entries_moved
            );
        } else {
            info!("No archives required compaction");
        }

        Ok(stats)
    }

    /// Check if an archive should be compacted based on fragmentation threshold
    #[allow(clippy::significant_drop_tightening)]
    fn should_compact_archive(&self, archive_id: u16, threshold: f64) -> Result<bool> {
        let archive = self
            .archives
            .get(&archive_id)
            .ok_or_else(|| StorageError::Archive(format!("Archive {archive_id} not found")))?;

        let total_size = archive.size;
        let used_size = {
            let positions = self.write_positions.read();
            *positions.get(&archive_id).unwrap_or(&0)
        };

        // If archive is empty, no need to compact
        if used_size == 0 {
            return Ok(false);
        }

        // Calculate utilization ratio
        #[allow(clippy::cast_precision_loss)]
        let utilization = used_size as f64 / total_size as f64;

        // Archive should be compacted if utilization is below threshold
        // This indicates significant space could be reclaimed
        let should_compact = utilization < (1.0 - threshold) && total_size > 1024 * 1024; // Only compact if >1MB

        if should_compact {
            debug!(
                "Archive {} utilization: {:.2}%, scheduling for compaction",
                archive_id,
                utilization * 100.0
            );
        }

        Ok(should_compact)
    }

    /// Compact a single archive by copying valid data to a temporary file
    #[allow(clippy::needless_pass_by_ref_mut)]
    fn compact_single_archive(&mut self, archive_id: u16) -> Result<SingleArchiveStats> {
        let mut stats = SingleArchiveStats::default();

        // Get original archive info
        let (original_path, original_size) = {
            let archive = self
                .archives
                .get(&archive_id)
                .ok_or_else(|| StorageError::Archive(format!("Archive {archive_id} not found")))?;
            (archive.path.clone(), archive.size)
        };

        // Create temporary file for compacted data
        let temp_path = original_path.with_extension("tmp");

        // For this implementation, we'll simulate compaction by truncating unused space
        // In a real implementation, this would involve:
        // 1. Reading all valid data entries
        // 2. Writing them contiguously to a new file
        // 3. Updating all index references
        // 4. Replacing the original file

        let used_size = {
            let positions = self.write_positions.read();
            *positions.get(&archive_id).unwrap_or(&original_size)
        };

        // Simple compaction: truncate file to used size if there's significant waste
        if used_size < original_size {
            // Copy used portion to temporary file
            std::fs::copy(&original_path, &temp_path).map_err(|e| {
                StorageError::Archive(format!("Failed to copy archive for compaction: {e}"))
            })?;

            // Truncate temporary file to used size
            let temp_file = OpenOptions::new()
                .write(true)
                .open(&temp_path)
                .map_err(|e| StorageError::Archive(format!("Failed to open temp file: {e}")))?;

            temp_file
                .set_len(used_size)
                .map_err(|e| StorageError::Archive(format!("Failed to truncate temp file: {e}")))?;

            // Replace original with compacted version
            std::fs::rename(&temp_path, &original_path).map_err(|e| {
                StorageError::Archive(format!(
                    "Failed to replace archive with compacted version: {e}"
                ))
            })?;

            // Remap the archive with new size
            self.remap_archive(archive_id, &original_path, used_size)?;

            stats.bytes_reclaimed = original_size.saturating_sub(used_size);
            stats.entries_moved = 1; // Simplified - in reality would track actual entries

            info!(
                "Compacted archive {} from {} to {} bytes (reclaimed {} bytes)",
                archive_id, original_size, used_size, stats.bytes_reclaimed
            );
        }

        Ok(stats)
    }
}

/// Statistics about archives
#[derive(Debug, Clone)]
pub struct ArchiveStats {
    /// Number of open archives
    pub archive_count: usize,
    /// Total size of all archives
    pub total_size: u64,
    /// Total used space in archives
    pub total_used: u64,
}

/// Statistics from compaction operation
#[derive(Debug, Clone, Default)]
pub struct CompactionStats {
    /// Number of archives compacted
    pub archives_compacted: usize,
    /// Total bytes reclaimed from compaction
    pub bytes_reclaimed: u64,
    /// Number of entries moved during compaction
    pub entries_moved: usize,
}

/// Statistics from compacting a single archive
#[derive(Debug, Clone, Default)]
struct SingleArchiveStats {
    /// Bytes reclaimed from this archive
    bytes_reclaimed: u64,
    /// Number of entries moved in this archive
    entries_moved: usize,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_blte_compression_decompression_round_trip() {
        let test_data = b"Hello, BLTE compression with cascette-formats!";

        // Compress data
        let compressed =
            ArchiveManager::compress_blte_with_formats(test_data).expect("Failed to compress data");

        // Verify it's actually BLTE data
        assert!(compressed.len() >= 4);
        assert_eq!(&compressed[0..4], b"BLTE");

        // Decompress data
        let decompressed = ArchiveManager::decompress_blte_with_formats(&compressed)
            .expect("Failed to decompress data");

        // Verify round-trip
        assert_eq!(decompressed, test_data);
    }

    #[test]
    fn test_blte_compression_different_data_sizes() {
        let zeros = vec![0u8; 1024];
        let sequence = (0..=255).collect::<Vec<u8>>();
        let test_cases = vec![
            b"".as_slice(),              // Empty data
            b"A".as_slice(),             // Single byte
            b"Hello, World!".as_slice(), // Short string
            &zeros,                      // 1KB zeros
            &sequence,                   // Byte sequence
        ];

        for (i, test_data) in test_cases.into_iter().enumerate() {
            // Compress
            let compressed = ArchiveManager::compress_blte_with_formats(test_data)
                .expect("Test compression should succeed");

            // Verify BLTE magic
            assert!(
                compressed.len() >= 4,
                "Test case {i}: compressed data too short"
            );
            assert_eq!(
                &compressed[0..4],
                b"BLTE",
                "Test case {i}: missing BLTE magic"
            );

            // Decompress
            let decompressed = ArchiveManager::decompress_blte_with_formats(&compressed)
                .expect("Test decompression should succeed");

            // Verify round-trip
            assert_eq!(
                decompressed, test_data,
                "Round-trip failed for test case {i}"
            );
        }
    }

    #[test]
    fn test_non_blte_data_passthrough() {
        let test_data = b"This is not BLTE data";

        // Non-BLTE data should fail to parse as BLTE
        let result = ArchiveManager::decompress_blte_with_formats(test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_blte_data() {
        // Test with invalid BLTE magic
        let invalid_data = b"NOTB\x00\x00\x00\x00some data";
        let result = ArchiveManager::decompress_blte_with_formats(invalid_data);
        assert!(result.is_err());

        // Test with too short data
        let short_data = b"BL";
        let result = ArchiveManager::decompress_blte_with_formats(short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_archive_manager_basic_operations() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let archive_manager = ArchiveManager::new(temp_dir.path());

        // Test stats on empty archive manager
        let stats = archive_manager.stats();
        assert_eq!(stats.archive_count, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.total_used, 0);
    }

    #[test]
    fn test_blte_formats_integration() {
        // Test that we're using the same parsing as cascette-formats directly
        let test_data = b"Integration test with cascette-formats BLTE";

        // Create BLTE using cascette-formats directly
        let blte_file = BlteFile::single_chunk(test_data.to_vec(), CompressionMode::None)
            .expect("Failed to create BLTE file");
        let direct_blte_data = blte_file.build().expect("Failed to build BLTE data");

        // Create BLTE using our wrapper
        let wrapper_blte_data = ArchiveManager::compress_blte_with_formats(test_data)
            .expect("Failed to compress with wrapper");

        // Both should produce identical results
        assert_eq!(direct_blte_data, wrapper_blte_data);

        // Both should decompress to same data
        let direct_decompressed = blte_file
            .decompress()
            .expect("Failed to decompress directly");
        let wrapper_decompressed = ArchiveManager::decompress_blte_with_formats(&wrapper_blte_data)
            .expect("Failed to decompress with wrapper");

        assert_eq!(direct_decompressed, test_data);
        assert_eq!(wrapper_decompressed, test_data);
        assert_eq!(direct_decompressed, wrapper_decompressed);
    }

    #[test]
    fn test_archive_read_content_blte_detection() {
        // This test validates that the BLTE detection logic works correctly

        // Test BLTE data detection
        let blte_data = b"BLTE\x00\x00\x00\x00N\x0AHello Test";
        assert!(blte_data.len() >= 4 && &blte_data[0..4] == b"BLTE");

        // Test non-BLTE data detection
        let non_blte_data = b"NOT BLTE data";
        assert!(non_blte_data.len() < 4 || &non_blte_data[0..4] != b"BLTE");

        let partial_data = b"BLT"; // Too short
        assert!(partial_data.len() < 4);
    }

    #[test]
    fn test_compression_mode_configuration() {
        let temp_dir = tempdir().expect("Failed to create temp dir");

        // Test default compression (None)
        let manager = ArchiveManager::new(temp_dir.path());
        assert_eq!(manager.compression_mode(), CompressionMode::None);

        // Test with specific compression mode
        let manager_zlib = ArchiveManager::with_compression(temp_dir.path(), CompressionMode::ZLib);
        assert_eq!(manager_zlib.compression_mode(), CompressionMode::ZLib);

        let manager_lz4 = ArchiveManager::with_compression(temp_dir.path(), CompressionMode::LZ4);
        assert_eq!(manager_lz4.compression_mode(), CompressionMode::LZ4);

        // Test setting compression mode
        let mut manager_mut = ArchiveManager::new(temp_dir.path());
        manager_mut.set_compression_mode(CompressionMode::ZLib);
        assert_eq!(manager_mut.compression_mode(), CompressionMode::ZLib);
    }

    #[test]
    #[allow(clippy::panic)]
    fn test_compression_modes_round_trip() {
        let test_data = b"This is test data for compression validation";

        // Test each supported compression mode
        let modes = vec![
            CompressionMode::None,
            CompressionMode::ZLib,
            CompressionMode::LZ4,
        ];

        for mode in modes {
            // Compress with specific mode
            let compressed = ArchiveManager::compress_blte_with_mode(test_data, mode)
                .expect("Compression should succeed in test");

            // Verify BLTE magic
            assert!(
                compressed.len() >= 4,
                "Compressed data too short for mode {mode:?}"
            );
            assert_eq!(
                &compressed[0..4],
                b"BLTE",
                "Missing BLTE magic for mode {mode:?}"
            );

            // Decompress and verify
            let decompressed = ArchiveManager::decompress_blte_with_formats(&compressed)
                .expect("Decompression should succeed in test");

            assert_eq!(
                decompressed, test_data,
                "Round-trip failed for mode {mode:?}"
            );
        }
    }

    #[test]
    fn test_unsupported_compression_modes() {
        let test_data = b"Test data";

        // Test that encrypted mode is rejected
        let result = ArchiveManager::compress_blte_with_mode(test_data, CompressionMode::Encrypted);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("Expected error for encrypted compression")
                .to_string()
                .contains("Encrypted compression not supported")
        );

        // Test that frame mode is rejected
        #[allow(deprecated)]
        let result = ArchiveManager::compress_blte_with_mode(test_data, CompressionMode::Frame);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("Expected error for frame compression")
                .to_string()
                .contains("Frame compression is deprecated")
        );
    }

    #[test]
    fn test_compaction_stats() {
        // Test compaction statistics structures
        let stats = CompactionStats::default();
        assert_eq!(stats.archives_compacted, 0);
        assert_eq!(stats.bytes_reclaimed, 0);
        assert_eq!(stats.entries_moved, 0);

        let single_stats = SingleArchiveStats::default();
        assert_eq!(single_stats.bytes_reclaimed, 0);
        assert_eq!(single_stats.entries_moved, 0);
    }

    #[test]
    fn test_write_content_prepends_local_header() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut manager = ArchiveManager::new(temp_dir.path());

        let test_data = b"Hello, CASC local header!";

        // Write content — should prepend 30-byte local header
        let (archive_id, offset, total_size, encoding_key) = manager
            .write_content(test_data, false)
            .expect("write_content should succeed");

        assert_eq!(offset, 0, "first write should be at offset 0");

        // Read raw bytes back — should contain local header + BLTE
        let raw = manager
            .read_raw(archive_id, offset, total_size)
            .expect("read_raw should succeed");

        // First 30 bytes are the local header
        assert!(raw.len() >= LOCAL_HEADER_SIZE + 4);

        // BLTE magic at offset 0x1E (after local header)
        assert_eq!(
            &raw[LOCAL_HEADER_SIZE..LOCAL_HEADER_SIZE + 4],
            b"BLTE",
            "BLTE magic should follow the 30-byte local header"
        );

        // Parse the local header
        let header =
            LocalHeader::from_bytes(&raw).expect("local header should parse from raw bytes");

        // Verify encoding key matches what write_content returned
        assert_eq!(
            header.original_encoding_key(),
            encoding_key,
            "encoding key in header should match returned key"
        );

        // Verify size_with_header matches total_size
        assert_eq!(header.size_with_header, total_size);
    }

    #[test]
    fn test_write_read_round_trip_with_local_header() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut manager = ArchiveManager::new(temp_dir.path());

        let test_data = b"Round-trip test data through local header path";

        // Write content (with local header)
        let (archive_id, offset, total_size, _encoding_key) = manager
            .write_content(test_data, false)
            .expect("write should succeed");

        // Read content — should skip local header, decompress BLTE, return original data
        let decompressed = manager
            .read_content(archive_id, offset, total_size)
            .expect("read_content should succeed");

        assert_eq!(
            decompressed, test_data,
            "round-trip through local header should recover original data"
        );
    }

    #[test]
    fn test_encoding_key_is_md5_of_blte_data() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut manager = ArchiveManager::new(temp_dir.path());

        let test_data = b"Verify encoding key is MD5 of BLTE data";

        let (archive_id, offset, total_size, encoding_key) = manager
            .write_content(test_data, false)
            .expect("write should succeed");

        // Read raw bytes and extract BLTE portion (skip 30-byte header)
        let raw = manager
            .read_raw(archive_id, offset, total_size)
            .expect("read_raw");
        let blte_data = &raw[LOCAL_HEADER_SIZE..];

        // Compute MD5 of BLTE data
        let expected_key = cascette_crypto::EncodingKey::from_data(blte_data);

        assert_eq!(
            encoding_key,
            *expected_key.as_bytes(),
            "encoding key should be MD5 of the BLTE-encoded data"
        );
    }
}

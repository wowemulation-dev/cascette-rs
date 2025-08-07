//! Common types used throughout the CASC storage system

use std::fmt;

/// Encoding key - 16 bytes that identify content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EKey([u8; 16]);

impl EKey {
    pub fn new(data: [u8; 16]) -> Self {
        Self(data)
    }

    pub fn from_slice(data: &[u8]) -> Option<Self> {
        if data.len() == 16 {
            let mut key = [0u8; 16];
            key.copy_from_slice(data);
            Some(Self(key))
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn truncated(&self) -> [u8; 9] {
        let mut truncated = [0u8; 9];
        truncated.copy_from_slice(&self.0[0..9]);
        truncated
    }

    /// Calculate the bucket index for this EKey using XOR hash
    pub fn bucket_index(&self) -> u8 {
        self.0.iter().fold(0u8, |acc, &byte| acc ^ byte) & 0x0F
    }
}

impl fmt::Display for EKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Location of a file within an archive
#[derive(Debug, Clone, Copy)]
pub struct ArchiveLocation {
    /// Archive file number (data.XXX)
    pub archive_id: u16,
    /// Offset within the archive file
    pub offset: u64,
    /// Size of the compressed data
    pub size: u32,
}

/// Entry in an index file
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// The encoding key for this file
    pub ekey: EKey,
    /// Location in archive
    pub location: ArchiveLocation,
}

/// Shared memory flags for inter-process communication
#[derive(Debug, Clone, Copy)]
pub struct SharedMemoryFlags {
    pub is_ready: bool,
    pub is_updating: bool,
    pub needs_repair: bool,
}

/// Statistics about the storage
#[derive(Debug, Default)]
pub struct StorageStats {
    pub total_archives: u32,
    pub total_indices: u32,
    pub total_size: u64,
    pub file_count: u64,
    pub duplicate_count: u64,
    pub compression_ratio: f32,
}

/// Configuration for CASC storage
#[derive(Debug, Clone)]
pub struct CascConfig {
    /// Base directory for storage
    pub data_path: std::path::PathBuf,
    /// Maximum size for a single archive file (default: 1GB)
    pub max_archive_size: u64,
    /// Enable memory mapping for archives
    pub use_memory_mapping: bool,
    /// Cache size in MB
    pub cache_size_mb: u32,
    /// Enable read-only mode
    pub read_only: bool,
}

impl Default for CascConfig {
    fn default() -> Self {
        Self {
            data_path: std::path::PathBuf::from("Data"),
            max_archive_size: 1024 * 1024 * 1024, // 1GB
            use_memory_mapping: true,
            cache_size_mb: 256,
            read_only: false,
        }
    }
}

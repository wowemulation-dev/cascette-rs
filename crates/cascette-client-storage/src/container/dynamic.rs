//! Dynamic container for read-write CASC archive storage.
//!
//! This is the primary container type. Most read and write operations
//! go through the dynamic container. It manages archive segments, the
//! Key Mapping Table (KMT), and coordinates with shared memory for
//! multi-process access.
//!
//! Configuration struct: `tact_DynamicContainerConfig` (40 bytes).

use std::path::PathBuf;

use crate::container::{AccessMode, Container};
use crate::{Result, StorageError};

/// Dynamic container for read-write CASC archive storage.
///
/// Configuration fields:
/// - `access_mode`: How the container is opened
/// - `shared_memory`: Enable shmem control block
/// - `storage_path`: Base directory for archive files
/// - `segment_limit`: Maximum segments, capped at 0x3FF (1023)
/// - `max_segment_size`: Maximum bytes per segment
/// - `free_space_reclaim`: Enable free space reclamation
pub struct DynamicContainer {
    /// Access mode for this container.
    access_mode: AccessMode,
    /// Base storage directory.
    storage_path: PathBuf,
    /// Enable shared memory coordination.
    shared_memory: bool,
    /// Container is opened read-only (access_mode == ReadOnly).
    read_only: bool,
    /// Maximum number of segments (capped at 0x3FF).
    segment_limit: u16,
    /// Maximum bytes per archive segment.
    max_segment_size: u64,
    /// Enable free space reclamation during writes.
    free_space_reclaim: bool,
}

/// Maximum number of archive segments .
pub const MAX_SEGMENTS: u16 = 0x3FF;

impl DynamicContainer {
    /// Create a new dynamic container.
    ///
    /// Returns `StorageError::Config` if `storage_path` is empty.
    pub fn new(
        access_mode: AccessMode,
        storage_path: PathBuf,
        shared_memory: bool,
        segment_limit: u16,
        max_segment_size: u64,
        free_space_reclaim: bool,
    ) -> Result<Self> {
        if storage_path.as_os_str().is_empty() {
            return Err(StorageError::Config(
                "storage path is required for DynamicContainer".to_string(),
            ));
        }

        let segment_limit = segment_limit.min(MAX_SEGMENTS);
        let read_only = access_mode == AccessMode::ReadOnly;

        Ok(Self {
            access_mode,
            storage_path,
            shared_memory,
            read_only,
            segment_limit,
            max_segment_size,
            free_space_reclaim,
        })
    }

    /// Get the access mode.
    pub const fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    /// Get the storage path.
    pub fn storage_path(&self) -> &PathBuf {
        &self.storage_path
    }

    /// Check if the container is read-only.
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Get the segment limit.
    pub const fn segment_limit(&self) -> u16 {
        self.segment_limit
    }

    /// Get the maximum segment size.
    pub const fn max_segment_size(&self) -> u64 {
        self.max_segment_size
    }

    /// Check if shared memory is enabled.
    pub const fn shared_memory_enabled(&self) -> bool {
        self.shared_memory
    }

    /// Check if free space reclamation is enabled.
    pub const fn free_space_reclaim(&self) -> bool {
        self.free_space_reclaim
    }
}

impl Container for DynamicContainer {
    async fn reserve(&self, _key: &[u8; 16]) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }
        // TODO: Allocate space in current segment
        Ok(())
    }

    async fn read(
        &self,
        _key: &[u8; 16],
        _offset: u64,
        _len: u32,
        _buf: &mut [u8],
    ) -> Result<usize> {
        if !self.access_mode.can_read() {
            return Err(StorageError::AccessDenied(
                "container has no read access".to_string(),
            ));
        }
        // TODO: Look up key in KMT, read from segment, handle truncation
        Err(StorageError::NotFound("not yet implemented".to_string()))
    }

    async fn write(&self, _key: &[u8; 16], _data: &[u8]) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }
        // TODO: Write local header + BLTE data, update KMT
        Ok(())
    }

    async fn remove(&self, _key: &[u8; 16]) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }
        // TODO: Remove key from KMT
        Ok(())
    }

    async fn query(&self, _key: &[u8; 16]) -> Result<bool> {
        // TODO: Check KMT for key existence
        Ok(false)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_dynamic_container_creation() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        assert_eq!(container.access_mode(), AccessMode::ReadWrite);
        assert!(!container.is_read_only());
        assert_eq!(container.segment_limit(), 100);
    }

    #[test]
    fn test_segment_limit_capped() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            2000, // Exceeds MAX_SEGMENTS
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        assert_eq!(container.segment_limit(), MAX_SEGMENTS);
    }

    #[test]
    fn test_empty_path_rejected() {
        let result = DynamicContainer::new(
            AccessMode::ReadWrite,
            PathBuf::new(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_only_rejects_writes() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadOnly,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        let key = [0u8; 16];
        let result = container.write(&key, b"data").await;
        assert!(result.is_err());
    }
}

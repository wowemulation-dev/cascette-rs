//! Hard link container for filesystem hard links.
//!
//! Manages hard links between installations to share content without
//! duplication. Uses a trie directory structure for organizing links.
//!
//! Hard link support is probed at initialization:
//! 1. Create test file `casc_hard_link_test_file` in source directory
//! 2. Attempt to create hard link in target directory
//! 3. Clean up both files
//!
//! If the filesystem doesn't support hard links, falls back to
//! `ResidencyContainer` behavior.

use crate::container::Container;
use crate::{Result, StorageError};

/// Hard link container for filesystem hard links.
///
/// Configuration from `tact_HardLinkContainer`:
/// - Same layout as `ResidencyContainer`
/// - Backed by a CASC trie directory at offset 0x28
///
/// Operations:
/// - `test_support()`: Probe filesystem for hard link support
/// - `create_link()`: Create a hard link (3-retry delete before create)
/// - `validate_links()`: Verify existing hard links
/// - `remove_file()`: Remove a hard-linked file
pub struct HardLinkContainer {
    /// Whether hard links are supported on this filesystem.
    supported: bool,
    /// Whether the container is read-only.
    read_only: bool,
}

impl HardLinkContainer {
    /// Create a new hard link container.
    ///
    /// Call `test_support()` after creation to check filesystem support.
    pub const fn new(read_only: bool) -> Self {
        Self {
            supported: false,
            read_only,
        }
    }

    /// Test if the filesystem supports hard links.
    ///
    /// Creates and removes test files in the given directories.
    pub fn test_support(
        &mut self,
        _source_dir: &std::path::Path,
        _target_dir: &std::path::Path,
    ) -> Result<bool> {
        // TODO: Create test file and attempt hard link
        // For now, assume not supported
        self.supported = false;
        Ok(self.supported)
    }

    /// Check if hard links are supported.
    pub const fn is_supported(&self) -> bool {
        self.supported
    }
}

impl Container for HardLinkContainer {
    async fn reserve(&self, _key: &[u8; 16]) -> Result<()> {
        if !self.supported {
            return Err(StorageError::Config(
                "hard links not supported on this filesystem".to_string(),
            ));
        }
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "hard link container is read-only".to_string(),
            ));
        }
        Ok(())
    }

    async fn read(
        &self,
        _key: &[u8; 16],
        _offset: u64,
        _len: u32,
        _buf: &mut [u8],
    ) -> Result<usize> {
        if !self.supported {
            return Err(StorageError::Config(
                "hard links not supported on this filesystem".to_string(),
            ));
        }
        // TODO: Read through hard link to static container
        Err(StorageError::NotFound("not yet implemented".to_string()))
    }

    async fn write(&self, _key: &[u8; 16], _data: &[u8]) -> Result<()> {
        // Hard link container creates links, not data writes
        Err(StorageError::InvalidFormat(
            "use create_link() for hard link operations".to_string(),
        ))
    }

    async fn remove(&self, _key: &[u8; 16]) -> Result<()> {
        if !self.supported {
            return Err(StorageError::Config(
                "hard links not supported on this filesystem".to_string(),
            ));
        }
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "hard link container is read-only".to_string(),
            ));
        }
        // TODO: Remove hard-linked file
        Ok(())
    }

    async fn query(&self, _key: &[u8; 16]) -> Result<bool> {
        if !self.supported {
            return Ok(false);
        }
        // TODO: Check if hard link exists for key
        Ok(false)
    }
}

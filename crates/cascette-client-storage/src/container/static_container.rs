//! Static container for read-only CASC archive access.
//!
//! Provides read-only access to finalized archives. Used for content
//! that has been fully downloaded and verified.

use crate::container::Container;
use crate::{Result, StorageError};

/// Static container for read-only archive access.
///
/// Operations:
/// - `state_lookup()`: Batch key state lookups returning (has_data, is_resident) flags
/// - Key validation against reference array
/// - No write operations supported
pub struct StaticContainer {
    /// Whether the container has been initialized.
    initialized: bool,
}

impl Default for StaticContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticContainer {
    /// Create a new static container.
    pub const fn new() -> Self {
        Self { initialized: false }
    }

    /// Initialize the container with archive data.
    pub fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    /// Batch key state lookup.
    ///
    /// For each key, returns `(has_data, is_resident)`.
    /// Output entry size is 0x68 (104) bytes .
    pub fn state_lookup(&self, _keys: &[[u8; 16]]) -> Result<Vec<(bool, bool)>> {
        if !self.initialized {
            return Err(StorageError::Config(
                "static container not initialized".to_string(),
            ));
        }
        // TODO: Look up keys against stored archive data
        Ok(Vec::new())
    }
}

impl Container for StaticContainer {
    async fn reserve(&self, _key: &[u8; 16]) -> Result<()> {
        Err(StorageError::AccessDenied(
            "static container is read-only".to_string(),
        ))
    }

    async fn read(
        &self,
        _key: &[u8; 16],
        _offset: u64,
        _len: u32,
        _buf: &mut [u8],
    ) -> Result<usize> {
        if !self.initialized {
            return Err(StorageError::Config(
                "static container not initialized".to_string(),
            ));
        }
        // TODO: Read from finalized archive
        Err(StorageError::NotFound("not yet implemented".to_string()))
    }

    async fn write(&self, _key: &[u8; 16], _data: &[u8]) -> Result<()> {
        Err(StorageError::AccessDenied(
            "static container is read-only".to_string(),
        ))
    }

    async fn remove(&self, _key: &[u8; 16]) -> Result<()> {
        Err(StorageError::AccessDenied(
            "static container is read-only".to_string(),
        ))
    }

    async fn query(&self, _key: &[u8; 16]) -> Result<bool> {
        if !self.initialized {
            return Ok(false);
        }
        // TODO: Check if key exists in archive
        Ok(false)
    }
}

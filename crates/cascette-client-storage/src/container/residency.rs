//! Residency container for download tracking.
//!
//! Tracks which files have been fully downloaded using `.residency`
//! token files. Supports byte-span tracking for partial downloads
//! and provides a scanner API for iterating all entries.
//!
//! CASC checks drive type before initialization and falls back
//! gracefully if the filesystem doesn't support the required operations.

use crate::container::Container;
use crate::{Result, StorageError};

/// Residency container for file-level download tracking.
///
/// Configuration from `tact_ResidencyContainer`:
/// - `product_name`: Product identifier for this container
/// - `read_only`: Whether the container is read-only
///
/// The container is backed by a `.residency` token file whose
/// presence indicates the product has residency tracking enabled.
pub struct ResidencyContainer {
    /// Product name for this container.
    product_name: String,
    /// Whether the container is read-only.
    read_only: bool,
    /// Whether the container has been initialized.
    initialized: bool,
}

impl ResidencyContainer {
    /// Create a new residency container.
    ///
    /// If `product_name` is empty, CASC logs:
    /// "No product provided, continuing without residency container."
    pub fn new(product_name: String, read_only: bool) -> Self {
        Self {
            product_name,
            read_only,
            initialized: false,
        }
    }

    /// Initialize the container.
    pub fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    /// Get the product name.
    pub fn product_name(&self) -> &str {
        &self.product_name
    }

    /// Mark a key as fully resident (downloaded).
    pub fn mark_resident(&self, _key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        // TODO: Update residency database
        Ok(())
    }

    /// Mark a key as non-resident (evicted or not yet downloaded).
    pub fn mark_non_resident(&self, _key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        // TODO: Update residency database
        Ok(())
    }
}

impl Container for ResidencyContainer {
    async fn reserve(&self, _key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
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
        // Residency container doesn't store data, only tracks state
        Err(StorageError::InvalidFormat(
            "residency container does not store file data".to_string(),
        ))
    }

    async fn write(&self, _key: &[u8; 16], _data: &[u8]) -> Result<()> {
        // Residency container doesn't store data
        Err(StorageError::InvalidFormat(
            "residency container does not store file data".to_string(),
        ))
    }

    async fn remove(&self, key: &[u8; 16]) -> Result<()> {
        self.mark_non_resident(key)
    }

    async fn query(&self, _key: &[u8; 16]) -> Result<bool> {
        // TODO: Check if key is marked as resident
        Ok(false)
    }
}

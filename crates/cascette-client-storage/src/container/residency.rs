//! Residency container for download tracking.
//!
//! Tracks which files have been fully downloaded using a `.residency`
//! token file. Supports reserve/mark-resident/remove/query operations.
//!
//! CASC checks drive type before initialization and falls back
//! gracefully if the filesystem doesn't support the required operations.
//!

use std::collections::HashSet;
use std::path::PathBuf;

use parking_lot::RwLock;
use tracing::{debug, warn};

use crate::container::{AccessMode, Container};
use crate::{Result, StorageError};

/// Residency token file name.
///
/// CASC creates this file in the storage directory to indicate
/// residency tracking is active. Missing on existing containers is
/// an error; new containers create it.
const RESIDENCY_TOKEN: &str = ".residency";

/// Residency container for file-level download tracking.
///
/// Configuration from `tact_ResidencyContainer` (0x30 = 48 bytes):
/// - offset 0x0c: product_name
/// - offset 0x28: residency_db (casc::Residency object)
/// - offset 0x2c: read_only flag
///
/// The container tracks which encoding keys are fully resident
/// (downloaded). Partial downloads are tracked via byte spans
/// (deferred — not implemented yet).
pub struct ResidencyContainer {
    /// Product name for this container.
    product_name: String,
    /// Access mode.
    access_mode: AccessMode,
    /// Whether the container is read-only.
    read_only: bool,
    /// Storage directory path.
    storage_path: PathBuf,
    /// Set of resident keys (simplified in-memory tracking).
    ///
    /// CASC uses a flat index at offset +0x40 with SRW locks.
    /// We use a `HashSet` behind a `RwLock` for now.
    resident_keys: RwLock<HashSet<[u8; 16]>>,
    /// Whether the container has been initialized.
    initialized: bool,
}

impl ResidencyContainer {
    /// Create a new residency container.
    ///
    /// If `product_name` is empty, CASC logs:
    /// "No product provided, continuing without residency container."
    pub fn new(product_name: String, access_mode: AccessMode, storage_path: PathBuf) -> Self {
        let read_only = access_mode == AccessMode::ReadOnly;
        Self {
            product_name,
            access_mode,
            read_only,
            storage_path,
            resident_keys: RwLock::new(HashSet::new()),
            initialized: false,
        }
    }

    /// Initialize the container.
    ///
    /// Creates the storage directory and `.residency` token file for
    /// new writable containers. For read-only or existing containers,
    /// verifies the token file exists.
    pub async fn initialize(&mut self) -> Result<()> {
        if self.product_name.is_empty() {
            warn!("no product provided, continuing without residency container");
            self.initialized = true;
            return Ok(());
        }

        let token_path = self.storage_path.join(RESIDENCY_TOKEN);

        if self.access_mode.can_write() {
            // Create directory if needed
            tokio::fs::create_dir_all(&self.storage_path)
                .await
                .map_err(|e| {
                    StorageError::Archive(format!(
                        "failed to create residency directory {}: {e}",
                        self.storage_path.display()
                    ))
                })?;

            // Create token file if it doesn't exist
            if !token_path.exists() {
                tokio::fs::write(&token_path, b"").await.map_err(|e| {
                    StorageError::Archive(format!(
                        "failed to create .residency token file at {}: {e}",
                        token_path.display()
                    ))
                })?;
                debug!("created .residency token file at {}", token_path.display());
            }
        } else if !token_path.exists() {
            // Read-only containers require existing token file
            warn!(
                "missing .residency token file for existing container at {}",
                self.storage_path.display()
            );
        }

        self.initialized = true;
        debug!(
            "residency container initialized for product '{}'",
            self.product_name
        );
        Ok(())
    }

    /// Get the product name.
    pub fn product_name(&self) -> &str {
        &self.product_name
    }

    /// Check if the container is read-only.
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Mark a key as fully resident (downloaded).
    ///
    /// CASC calls `casc::Residency::UpdateResidency` which
    /// updates the flat index at +0x40.
    pub fn mark_resident(&self, key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        self.resident_keys.write().insert(*key);
        Ok(())
    }

    /// Mark a key as non-resident (evicted or not yet downloaded).
    pub fn mark_non_resident(&self, key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        self.resident_keys.write().remove(key);
        Ok(())
    }

    /// Check if a key is resident.
    ///
    /// CASC `casc::Residency::CheckResidency` acquires SRW lock
    /// at +0x74 and checks the data structure at +0x78.
    pub fn is_resident(&self, key: &[u8; 16]) -> bool {
        self.resident_keys.read().contains(key)
    }

    /// Get the number of resident keys.
    pub fn resident_count(&self) -> usize {
        self.resident_keys.read().len()
    }

    /// Iterate over all resident keys.
    ///
    /// CASC's scanner (`casc::Residency::OpenScanner`) uses a
    /// shuffled index for non-sequential access. We return keys in
    /// arbitrary order.
    pub fn scan_keys(&self) -> Vec<[u8; 16]> {
        self.resident_keys.read().iter().copied().collect()
    }
}

impl Container for ResidencyContainer {
    async fn reserve(&self, key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        // Reserve is a no-op until mark_resident is called.
        // CASC tracks reservations separately from residency.
        debug!("reserve: key={}", hex::encode(&key[..9]));
        Ok(())
    }

    async fn read(
        &self,
        _key: &[u8; 16],
        _offset: u64,
        _len: u32,
        _buf: &mut [u8],
    ) -> Result<usize> {
        // Residency container does not store file data
        Err(StorageError::InvalidFormat(
            "residency container does not store file data".to_string(),
        ))
    }

    async fn write(&self, _key: &[u8; 16], _data: &[u8]) -> Result<()> {
        // Residency container does not store file data
        Err(StorageError::InvalidFormat(
            "residency container does not store file data".to_string(),
        ))
    }

    async fn remove(&self, key: &[u8; 16]) -> Result<()> {
        self.mark_non_resident(key)
    }

    async fn query(&self, key: &[u8; 16]) -> Result<bool> {
        Ok(self.is_resident(key))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_residency_creation() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );
        assert!(!container.is_read_only());
        assert_eq!(container.product_name(), "wow");
    }

    #[tokio::test]
    async fn test_residency_init_creates_token() {
        let dir = tempdir().expect("tempdir");
        let mut container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        container.initialize().await.expect("init");
        assert!(dir.path().join(RESIDENCY_TOKEN).exists());
    }

    #[tokio::test]
    async fn test_empty_product_initializes() {
        let dir = tempdir().expect("tempdir");
        let mut container = ResidencyContainer::new(
            String::new(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        container.initialize().await.expect("init");
        assert!(container.initialized);
        // No token file created for empty product
        assert!(!dir.path().join(RESIDENCY_TOKEN).exists());
    }

    #[test]
    fn test_mark_resident_query() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        let key = [0xAA; 16];
        assert!(!container.is_resident(&key));

        container.mark_resident(&key).expect("mark");
        assert!(container.is_resident(&key));

        container.mark_non_resident(&key).expect("unmark");
        assert!(!container.is_resident(&key));
    }

    #[test]
    fn test_read_only_rejects_mutations() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadOnly,
            dir.path().to_path_buf(),
        );

        let key = [0xBB; 16];
        assert!(container.mark_resident(&key).is_err());
        assert!(container.mark_non_resident(&key).is_err());
    }

    #[test]
    fn test_scan_keys() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        let k1 = [0x11; 16];
        let k2 = [0x22; 16];
        container.mark_resident(&k1).expect("mark1");
        container.mark_resident(&k2).expect("mark2");

        let keys = container.scan_keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&k1));
        assert!(keys.contains(&k2));
    }

    #[tokio::test]
    async fn test_container_trait_read_rejected() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        let mut buf = [0u8; 64];
        assert!(container.read(&[0u8; 16], 0, 0, &mut buf).await.is_err());
        assert!(container.write(&[0u8; 16], b"data").await.is_err());
    }
}

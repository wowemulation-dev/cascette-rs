//! Residency container for download tracking.
//!
//! Tracks which files have been fully downloaded using KMT V8
//! file-backed storage with 16 buckets and MurmurHash3 fast-path
//! lookups. Falls back to in-memory tracking when no product name
//! is provided.
//!
//! CASC checks drive type before initialization and falls back
//! gracefully if the filesystem doesn't support the required operations.

use std::path::PathBuf;

use parking_lot::RwLock;
use tracing::{debug, warn};

use crate::container::{AccessMode, Container};
use crate::kmt::key_state::ResidencyDb;
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
/// Tracks which encoding keys are fully resident (downloaded).
/// Partial downloads are tracked via byte spans using
/// `mark_span_non_resident`.
pub struct ResidencyContainer {
    /// Product name for this container.
    product_name: String,
    /// Access mode.
    access_mode: AccessMode,
    /// Whether the container is read-only.
    read_only: bool,
    /// Storage directory path.
    storage_path: PathBuf,
    /// File-backed residency database with 16 buckets and MurmurHash3
    /// fast-path lookups.
    db: RwLock<ResidencyDb>,
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
        let db_path = storage_path.join("key_state_v8");
        Self {
            product_name,
            access_mode,
            read_only,
            storage_path,
            db: RwLock::new(ResidencyDb::new(db_path)),
            initialized: false,
        }
    }

    /// Initialize the container.
    ///
    /// Creates the storage directory and `.residency` token file for
    /// new writable containers. For read-only or existing containers,
    /// verifies the token file exists. Loads existing residency data
    /// from disk.
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

        // Load existing residency data from disk
        {
            let db_path = self.storage_path.join("key_state_v8");
            match ResidencyDb::load(&db_path) {
                Ok(loaded) => *self.db.write() = loaded,
                Err(e) => debug!("no existing residency data (new container): {e}"),
            }
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
    /// updates the KMT V8 entry with update type Set(1).
    pub fn mark_resident(&self, key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        self.db.write().mark_resident(key);
        Ok(())
    }

    /// Mark a key as non-resident (evicted or not yet downloaded).
    pub fn mark_non_resident(&self, key: &[u8; 16]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        self.db.write().mark_non_resident(key);
        Ok(())
    }

    /// Mark a byte span of a key as non-resident (truncation tracking).
    ///
    /// Called when a read detects truncated data. Records the non-resident
    /// range so the LRU cache and compactor know which spans need
    /// re-download.
    pub fn mark_span_non_resident(&self, key: &[u8; 16], offset: i32, length: i32) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        self.db.write().mark_span_non_resident(key, offset, length);
        Ok(())
    }

    /// Check if a key is resident.
    ///
    /// Uses MurmurHash3 fast-path: computes hash of the key, checks
    /// hash index first, then does full bucket scan only if the hash
    /// matches.
    pub fn is_resident(&self, key: &[u8; 16]) -> bool {
        self.db.read().is_resident(key)
    }

    /// Get the number of resident keys.
    pub fn resident_count(&self) -> usize {
        self.db.read().entry_count()
    }

    /// Scan all resident keys using two-pass algorithm.
    ///
    /// Pass 1: count entries across all 16 buckets.
    /// Pass 2: populate the output vector.
    /// Matches Agent.exe's `casc::Residency::OpenScanner` behavior.
    pub fn scan_keys(&self) -> Vec<[u8; 16]> {
        self.db.read().scan_keys()
    }

    /// Delete a batch of keys.
    ///
    /// Uses threshold-based strategy: below 10,000 keys uses
    /// sequential deletion, above switches to batch path with
    /// page-level scanning.
    pub fn delete_keys(&self, keys: &[[u8; 16]]) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "residency container is read-only".to_string(),
            ));
        }
        self.db.write().delete_keys(keys);
        Ok(())
    }

    /// Flush dirty residency data to disk.
    pub fn flush(&self) -> Result<()> {
        if self.read_only {
            return Ok(());
        }
        self.db
            .write()
            .save()
            .map_err(|e| StorageError::Archive(format!("failed to flush residency data: {e}")))
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

    #[test]
    fn test_span_non_resident() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        let key = [0xCC; 16];
        container.mark_resident(&key).expect("mark");
        assert!(container.is_resident(&key));

        // Mark a span as non-resident (truncation detection)
        container
            .mark_span_non_resident(&key, 1024, 512)
            .expect("span");

        // Key should still exist in the DB (span tracking, not removal)
        assert_eq!(container.resident_count(), 1);
    }

    #[test]
    fn test_batch_delete() {
        let dir = tempdir().expect("tempdir");
        let container = ResidencyContainer::new(
            "wow".to_string(),
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
        );

        let keys: Vec<[u8; 16]> = (0..50u8).map(|i| [i; 16]).collect();
        for key in &keys {
            container.mark_resident(key).expect("mark");
        }
        assert_eq!(container.resident_count(), 50);

        // Delete first 20 keys
        container.delete_keys(&keys[..20]).expect("batch delete");
        assert_eq!(container.resident_count(), 30);
    }

    #[test]
    fn test_flush_and_reload() {
        let dir = tempdir().expect("tempdir");
        let storage_path = dir.path().to_path_buf();

        // Create and populate
        {
            let container = ResidencyContainer::new(
                "wow".to_string(),
                AccessMode::ReadWrite,
                storage_path.clone(),
            );

            let k1 = [0x11; 16];
            let k2 = [0x22; 16];
            container.mark_resident(&k1).expect("mark1");
            container.mark_resident(&k2).expect("mark2");
            container.flush().expect("flush");
        }

        // Reload and verify
        {
            let db_path = storage_path.join("key_state_v8");
            let container =
                ResidencyContainer::new("wow".to_string(), AccessMode::ReadOnly, storage_path);

            // Load from disk
            *container.db.write() = ResidencyDb::load(&db_path).expect("load");

            let k1 = [0x11; 16];
            let k2 = [0x22; 16];
            assert!(container.is_resident(&k1));
            assert!(container.is_resident(&k2));
            assert_eq!(container.resident_count(), 2);
        }
    }
}

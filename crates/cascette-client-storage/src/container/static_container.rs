//! Static container for read-only CASC archive access.
//!
//! Provides read-only access to finalized archives. Used for content
//! shared between installations via hard links or when archives are
//! frozen.
//!

use std::path::PathBuf;

use cascette_crypto::EncodingKey;
use parking_lot::RwLock;
use tracing::debug;

use crate::container::Container;
use crate::index::IndexManager;
use crate::storage::archive_file::ArchiveManager;
use crate::{Result, StorageError};

/// File state from a batch key lookup.
///
/// Output entry is 0x68 (104) bytes in CASC. We expose the two
/// flags that matter: whether the archive has the data, and whether
/// the data is resident (fully downloaded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyState {
    /// Archive contains data for this key.
    pub has_data: bool,
    /// Data is fully resident (downloaded).
    pub is_resident: bool,
}

/// Static container for read-only archive access.
///
/// Operations:
/// - `state_lookup()`: Batch key state lookups returning `KeyState` per key
/// - Read through Container trait (read-only)
/// - Write/remove operations return `AccessDenied`
pub struct StaticContainer {
    /// Storage path for the archive directory.
    storage_path: PathBuf,
    /// Index manager for key-to-location mapping.
    index: RwLock<IndexManager>,
    /// Archive manager for data file reads.
    archive: RwLock<ArchiveManager>,
    /// Whether the container has been opened.
    initialized: bool,
}

impl StaticContainer {
    /// Create a new static container.
    ///
    /// Call [`open`](Self::open) to load indices and archive files.
    pub fn new(storage_path: PathBuf) -> Self {
        let index = IndexManager::new(&storage_path);
        let archive = ArchiveManager::new(&storage_path);
        Self {
            storage_path,
            index: RwLock::new(index),
            archive: RwLock::new(archive),
            initialized: false,
        }
    }

    /// Open the container: load index files and open archive data files.
    pub async fn open(&mut self) -> Result<()> {
        let mut index = IndexManager::new(&self.storage_path);
        index.load_all().await?;
        *self.index.write() = index;

        let mut archive = ArchiveManager::new(&self.storage_path);
        archive.open_all().await?;
        *self.archive.write() = archive;

        self.initialized = true;
        debug!(
            "StaticContainer opened: {} entries",
            self.index.read().entry_count()
        );
        Ok(())
    }

    /// Batch key state lookup.
    ///
    /// For each key, returns `KeyState { has_data, is_resident }`.
    /// Output entries are 0x68 (104) bytes each with flags at
    /// specific offsets. We return the two flags directly.
    ///
    /// Zero keys (all bytes zero) are skipped with `has_data = false`.
    pub fn state_lookup(&self, keys: &[[u8; 16]]) -> Result<Vec<KeyState>> {
        if !self.initialized {
            return Err(StorageError::Config(
                "static container not initialized".to_string(),
            ));
        }

        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            // CASC checks for zero keys
            if key == &[0u8; 16] {
                results.push(KeyState {
                    has_data: false,
                    is_resident: false,
                });
                continue;
            }

            let ekey = EncodingKey::from_bytes(*key);
            let has_data = self.index.read().has_entry(&ekey);

            // In a static container, if the data exists it is resident
            // (archives are finalized and fully present).
            results.push(KeyState {
                has_data,
                is_resident: has_data,
            });
        }

        Ok(results)
    }

    /// Get the number of indexed entries.
    pub fn entry_count(&self) -> usize {
        self.index.read().entry_count()
    }
}

impl Container for StaticContainer {
    async fn reserve(&self, _key: &[u8; 16]) -> Result<()> {
        Err(StorageError::AccessDenied(
            "static container is read-only".to_string(),
        ))
    }

    async fn read(&self, key: &[u8; 16], _offset: u64, _len: u32, buf: &mut [u8]) -> Result<usize> {
        if !self.initialized {
            return Err(StorageError::Config(
                "static container not initialized".to_string(),
            ));
        }

        let ekey = EncodingKey::from_bytes(*key);
        let entry = {
            let index = self.index.read();
            index.lookup(&ekey).ok_or_else(|| {
                StorageError::NotFound(format!("key {} not in index", hex::encode(&key[..9])))
            })?
        };

        let data = {
            let archive = self.archive.read();
            archive.read_content(entry.archive_id(), entry.archive_offset(), entry.size)?
        };

        let copy_len = data.len().min(buf.len());
        buf[..copy_len].copy_from_slice(&data[..copy_len]);
        Ok(copy_len)
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

    async fn query(&self, key: &[u8; 16]) -> Result<bool> {
        if !self.initialized {
            return Ok(false);
        }
        let ekey = EncodingKey::from_bytes(*key);
        let index = self.index.read();
        Ok(index.has_entry(&ekey))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_static_container_creation() {
        let dir = tempdir().expect("tempdir");
        let container = StaticContainer::new(dir.path().to_path_buf());
        assert!(!container.initialized);
    }

    #[test]
    fn test_state_lookup_requires_init() {
        let dir = tempdir().expect("tempdir");
        let container = StaticContainer::new(dir.path().to_path_buf());
        let keys = vec![[0xAAu8; 16]];
        assert!(container.state_lookup(&keys).is_err());
    }

    #[tokio::test]
    async fn test_static_rejects_writes() {
        let dir = tempdir().expect("tempdir");
        let container = StaticContainer::new(dir.path().to_path_buf());
        let key = [0u8; 16];
        assert!(container.write(&key, b"data").await.is_err());
        assert!(container.reserve(&key).await.is_err());
        assert!(container.remove(&key).await.is_err());
    }

    #[tokio::test]
    async fn test_state_lookup_zero_key_skipped() {
        let dir = tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path()).expect("mkdir");
        let mut container = StaticContainer::new(dir.path().to_path_buf());
        container.open().await.expect("open");

        let keys = vec![[0u8; 16], [0xAAu8; 16]];
        let results = container.state_lookup(&keys).expect("lookup");

        assert_eq!(results.len(), 2);
        // Zero key → no data
        assert!(!results[0].has_data);
        assert!(!results[0].is_resident);
        // Non-existent key → no data
        assert!(!results[1].has_data);
    }
}

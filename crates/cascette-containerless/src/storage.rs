//! Top-level containerless storage API.
//!
//! Ties together the SQLite database, loose file store, and residency
//! tracker into a single interface for reading, writing, and verifying
//! containerless installations.

use std::collections::HashSet;

use tokio::fs;
use tracing::{debug, info, warn};

use crate::config::ContainerlessConfig;
use crate::db::crypto::iv_from_key;
use crate::db::{FileDatabase, FileEntry};
use crate::eheader::EHeaderCache;
use crate::error::{ContainerlessError, ContainerlessResult};
use crate::loose::LooseFileStore;
use crate::residency::ResidencyTracker;

/// Storage statistics.
#[derive(Debug)]
pub struct StorageStats {
    /// Total file entries in the database.
    pub total_files: usize,
    /// Files that are locally resident.
    pub resident_files: usize,
    /// Total encoded size (bytes) of all files in the database.
    pub total_size_bytes: u64,
    /// Total encoded size (bytes) of resident files.
    pub resident_size_bytes: u64,
}

/// Verification report.
#[derive(Debug)]
pub struct VerifyReport {
    /// Total entries checked.
    pub total: usize,
    /// Entries that passed verification.
    pub valid: usize,
    /// Entries that failed verification.
    pub invalid: usize,
    /// Entries that were missing on disk.
    pub missing: usize,
    /// Hex-encoded ekeys of invalid entries.
    pub invalid_keys: Vec<String>,
}

/// Containerless storage instance.
///
/// Manages loose files on disk with metadata in an SQLite database.
pub struct ContainerlessStorage {
    config: ContainerlessConfig,
    db: FileDatabase,
    loose: LooseFileStore,
    residency: ResidencyTracker,
    eheader_cache: EHeaderCache,
}

impl ContainerlessStorage {
    /// Open or create a containerless storage instance.
    pub async fn open(config: ContainerlessConfig) -> ContainerlessResult<Self> {
        config.validate()?;

        let db = Self::open_db(&config).await?;

        let loose = LooseFileStore::new(config.root.clone());

        // Build initial residency set from database + filesystem.
        let files = db.all_files().await?;
        let mut resident_keys = HashSet::new();
        for f in &files {
            if loose.exists(&f.ekey) {
                resident_keys.insert(f.ekey);
            }
        }
        let resident_count = resident_keys.len();
        let residency = ResidencyTracker::with_keys(resident_keys);

        info!(
            total_files = files.len(),
            resident = resident_count,
            "opened containerless storage"
        );

        Ok(Self {
            config,
            db,
            loose,
            residency,
            eheader_cache: EHeaderCache::new(),
        })
    }

    /// Open (or create) the database based on config.
    async fn open_db(config: &ContainerlessConfig) -> ContainerlessResult<FileDatabase> {
        let db_path = config.resolved_db_path();

        if let Some(ref key) = config.db_key {
            if db_path.exists() {
                let data = fs::read(&db_path).await?;
                let default_iv = iv_from_key(key);
                let iv = config.db_iv.as_deref().unwrap_or(&default_iv);
                FileDatabase::open_encrypted(&data, key, iv).await
            } else {
                debug!("creating new encrypted database");
                FileDatabase::create_new().await
            }
        } else if db_path.exists() {
            FileDatabase::open_plaintext(&db_path).await
        } else {
            debug!("creating new plaintext database at {}", db_path.display());
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            FileDatabase::open_plaintext(&db_path).await
        }
    }

    /// Read a file by encoding key.
    pub async fn read_by_ekey(&self, ekey: &[u8; 16]) -> ContainerlessResult<Vec<u8>> {
        self.loose.read(ekey).await
    }

    /// Read a file by content key.
    ///
    /// Looks up the encoding key in the database, then reads the loose file.
    pub async fn read_by_ckey(&self, ckey: &[u8; 16]) -> ContainerlessResult<Vec<u8>> {
        let entry = self.db.get_file_by_ckey(ckey).await?.ok_or_else(|| {
            ContainerlessError::NotFound(format!("ckey not in database: {}", hex::encode(ckey)))
        })?;
        self.loose.read(&entry.ekey).await
    }

    /// Write a file. Updates both the loose store and the database.
    pub async fn write(&self, entry: &FileEntry, data: &[u8]) -> ContainerlessResult<()> {
        self.loose.write(&entry.ekey, data).await?;
        self.db.upsert_file(entry).await?;
        self.residency.mark_resident(&entry.ekey);
        Ok(())
    }

    /// Remove a file by encoding key from both loose store and database.
    pub async fn remove(&self, ekey: &[u8; 16]) -> ContainerlessResult<()> {
        self.loose.remove(ekey).await?;
        self.db.remove_file(ekey).await?;
        self.residency.mark_absent(ekey);
        self.eheader_cache.remove(ekey);
        Ok(())
    }

    /// Check whether a file is resident on disk.
    #[must_use]
    pub fn is_resident(&self, ekey: &[u8; 16]) -> bool {
        self.residency.is_resident(ekey)
    }

    /// Query a file entry from the database.
    pub async fn query_file(&self, ekey: &[u8; 16]) -> ContainerlessResult<Option<FileEntry>> {
        self.db.get_file(ekey).await
    }

    /// List all file entries.
    pub async fn list_files(&self) -> ContainerlessResult<Vec<FileEntry>> {
        self.db.all_files().await
    }

    /// Compute storage statistics.
    pub async fn stats(&self) -> ContainerlessResult<StorageStats> {
        let files = self.db.all_files().await?;
        let total_files = files.len();
        let mut total_size_bytes = 0u64;
        let mut resident_files = 0usize;
        let mut resident_size_bytes = 0u64;

        for f in &files {
            total_size_bytes += f.encoded_size;
            if self.residency.is_resident(&f.ekey) {
                resident_files += 1;
                resident_size_bytes += f.encoded_size;
            }
        }

        Ok(StorageStats {
            total_files,
            resident_files,
            total_size_bytes,
            resident_size_bytes,
        })
    }

    /// Verify the integrity of resident loose files.
    ///
    /// Checks existence, size, and MD5 hash (encoding key = MD5 of
    /// BLTE-encoded data) for each file in the database.
    pub async fn verify(&self) -> ContainerlessResult<VerifyReport> {
        let files = self.db.all_files().await?;
        let total = files.len();
        let mut valid = 0usize;
        let mut invalid = 0usize;
        let mut missing = 0usize;
        let mut invalid_keys = Vec::new();

        for entry in &files {
            let ekey_hex = hex::encode(entry.ekey);

            if !self.loose.exists(&entry.ekey) {
                missing += 1;
                invalid_keys.push(ekey_hex.clone());
                debug!(ekey = %ekey_hex, "missing");
                continue;
            }

            match self.loose.read(&entry.ekey).await {
                Ok(data) => {
                    if data.len() as u64 != entry.encoded_size {
                        invalid += 1;
                        invalid_keys.push(ekey_hex.clone());
                        warn!(
                            ekey = %ekey_hex,
                            expected = entry.encoded_size,
                            actual = data.len(),
                            "size mismatch"
                        );
                        continue;
                    }

                    let hash = md5::compute(&data);
                    if hash.0 != entry.ekey {
                        invalid += 1;
                        invalid_keys.push(ekey_hex.clone());
                        warn!(
                            ekey = %ekey_hex,
                            actual_hash = %hex::encode(hash.0),
                            "hash mismatch"
                        );
                        continue;
                    }

                    valid += 1;
                }
                Err(e) => {
                    invalid += 1;
                    invalid_keys.push(ekey_hex.clone());
                    warn!(ekey = %ekey_hex, error = %e, "read error during verify");
                }
            }
        }

        info!(total, valid, invalid, missing, "verification complete");

        Ok(VerifyReport {
            total,
            valid,
            invalid,
            missing,
            invalid_keys,
        })
    }

    /// Flush the database to disk.
    ///
    /// For encrypted databases, this re-encrypts and writes to the
    /// configured path. For plaintext databases opened from a file,
    /// this is a no-op (WAL mode handles persistence).
    pub async fn flush(&self) -> ContainerlessResult<()> {
        if let Some(ref key) = self.config.db_key {
            let default_iv = iv_from_key(key);
            let iv = self.config.db_iv.as_deref().unwrap_or(&default_iv);
            let encrypted = self.db.export_encrypted(key, iv).await?;
            let db_path = self.config.resolved_db_path();
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&db_path, &encrypted).await?;
            debug!("flushed encrypted database to {}", db_path.display());
        }
        Ok(())
    }

    /// Access the e-header cache.
    #[must_use]
    pub fn eheader_cache(&self) -> &EHeaderCache {
        &self.eheader_cache
    }

    /// Access the loose file store.
    #[must_use]
    pub fn loose_store(&self) -> &LooseFileStore {
        &self.loose
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample_entry(index: u32, data: &[u8]) -> FileEntry {
        let hash = md5::compute(data);
        FileEntry {
            index,
            ekey: hash.0,
            ckey: [index as u8; 16],
            encoded_size: data.len() as u64,
            decoded_size: data.len() as u64,
            path: None,
            flags: 0,
        }
    }

    #[tokio::test]
    async fn test_open_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.resident_files, 0);
    }

    #[tokio::test]
    async fn test_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let data = b"hello containerless world";
        let entry = sample_entry(1, data);

        storage.write(&entry, data).await.unwrap();
        assert!(storage.is_resident(&entry.ekey));

        let loaded = storage.read_by_ekey(&entry.ekey).await.unwrap();
        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn test_read_by_ckey() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let data = b"ckey lookup test";
        let entry = sample_entry(2, data);
        let ckey = entry.ckey;

        storage.write(&entry, data).await.unwrap();
        let loaded = storage.read_by_ckey(&ckey).await.unwrap();
        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn test_remove() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let data = b"to be removed";
        let entry = sample_entry(3, data);
        let ekey = entry.ekey;

        storage.write(&entry, data).await.unwrap();
        storage.remove(&ekey).await.unwrap();

        assert!(!storage.is_resident(&ekey));
        assert!(storage.query_file(&ekey).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_verify() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let data = b"verify me";
        let entry = sample_entry(4, data);
        storage.write(&entry, data).await.unwrap();

        let report = storage.verify().await.unwrap();
        assert_eq!(report.total, 1);
        assert_eq!(report.valid, 1);
        assert_eq!(report.invalid, 0);
        assert_eq!(report.missing, 0);
    }

    #[tokio::test]
    async fn test_stats() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let data1 = b"file one content";
        let data2 = b"file two";
        let entry1 = sample_entry(1, data1);
        let entry2 = sample_entry(2, data2);

        storage.write(&entry1, data1).await.unwrap();
        storage.write(&entry2, data2).await.unwrap();

        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.resident_files, 2);
        assert_eq!(stats.total_size_bytes, (data1.len() + data2.len()) as u64);
    }

    #[tokio::test]
    async fn test_list_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = ContainerlessConfig::new(dir.path().to_path_buf());
        let storage = ContainerlessStorage::open(config).await.unwrap();

        let data = b"list test";
        let entry = sample_entry(10, data);
        storage.write(&entry, data).await.unwrap();

        let files = storage.list_files().await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].index, 10);
    }
}

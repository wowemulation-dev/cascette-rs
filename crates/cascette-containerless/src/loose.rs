//! Loose file storage.
//!
//! Files are stored at `{root}/{ekey_hex[0..2]}/{ekey_hex[2..4]}/{ekey_hex}`
//! matching the CDN URL path convention used by TACTSharp and agent.exe.

use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;

use crate::error::ContainerlessResult;

/// Manages loose files on disk keyed by encoding key hash.
pub struct LooseFileStore {
    root: PathBuf,
}

impl LooseFileStore {
    /// Create a store rooted at the given directory.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Read a loose file by encoding key.
    pub async fn read(&self, ekey: &[u8; 16]) -> ContainerlessResult<Vec<u8>> {
        let path = self.path_for_ekey(ekey);
        let data = fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::error::ContainerlessError::NotFound(format!(
                    "loose file not found: {}",
                    hex::encode(ekey)
                ))
            } else {
                crate::error::ContainerlessError::Io(e)
            }
        })?;
        Ok(data)
    }

    /// Write a loose file by encoding key.
    ///
    /// Writes to a `.tmp` sibling file then renames atomically, matching
    /// Agent.exe's temp-then-rename pattern for crash safety.
    pub async fn write(&self, ekey: &[u8; 16], data: &[u8]) -> ContainerlessResult<()> {
        let path = self.path_for_ekey(ekey);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, data).await?;
        fs::rename(&tmp_path, &path).await?;
        debug!(ekey = %hex::encode(ekey), size = data.len(), "wrote loose file");
        Ok(())
    }

    /// Remove a loose file by encoding key.
    pub async fn remove(&self, ekey: &[u8; 16]) -> ContainerlessResult<()> {
        let path = self.path_for_ekey(ekey);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(crate::error::ContainerlessError::Io(e)),
        }
    }

    /// Check whether a loose file exists on disk.
    #[must_use]
    pub fn exists(&self, ekey: &[u8; 16]) -> bool {
        self.path_for_ekey(ekey).exists()
    }

    /// Compute the on-disk path for a given encoding key.
    ///
    /// Path layout: `{root}/{hex[0..2]}/{hex[2..4]}/{hex}`
    #[must_use]
    pub fn path_for_ekey(&self, ekey: &[u8; 16]) -> PathBuf {
        let hex_str = hex::encode(ekey);
        self.root
            .join(&hex_str[..2])
            .join(&hex_str[2..4])
            .join(&hex_str)
    }

    /// Create a sparse file with the expected total size.
    ///
    /// The file is created at the path for the given encoding key,
    /// set to the specified size, and marked sparse. On platforms
    /// without sparse support, the file is still created at the
    /// expected size (filled with zeros).
    pub async fn write_sparse(&self, ekey: &[u8; 16], total_size: u64) -> ContainerlessResult<()> {
        let path = self.path_for_ekey(ekey);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Create the file at the expected size.
        let file = fs::File::create(&path).await?;
        file.set_len(total_size).await?;
        drop(file);

        // Mark as sparse. This is a no-op on Linux/macOS (files are
        // implicitly sparse on ext4/btrfs/APFS), but required on Windows.
        if let Err(e) = crate::sparse::set_sparse(&path) {
            debug!(
                ekey = %hex::encode(ekey),
                error = %e,
                "failed to set sparse attribute, performance may be impacted"
            );
        }

        debug!(ekey = %hex::encode(ekey), size = total_size, "created sparse loose file");
        Ok(())
    }

    /// Compute the total size of all files under the root directory.
    pub async fn total_size(&self) -> ContainerlessResult<u64> {
        total_dir_size(&self.root).await
    }

    /// Return the root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Recursively sum file sizes in a directory.
async fn total_dir_size(path: &Path) -> ContainerlessResult<u64> {
    let mut total = 0u64;
    if !path.exists() {
        return Ok(0);
    }
    let mut entries = fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ft = entry.file_type().await?;
        if ft.is_file() {
            total += entry.metadata().await?.len();
        } else if ft.is_dir() {
            total += Box::pin(total_dir_size(&entry.path())).await?;
        }
    }
    Ok(total)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_ekey() -> [u8; 16] {
        [
            0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45,
            0x67, 0x89,
        ]
    }

    #[test]
    fn test_path_for_ekey() {
        let store = LooseFileStore::new(PathBuf::from("/data"));
        let path = store.path_for_ekey(&test_ekey());
        assert_eq!(
            path,
            PathBuf::from("/data/ab/cd/abcdef0123456789abcdef0123456789")
        );
    }

    #[tokio::test]
    async fn test_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = LooseFileStore::new(dir.path().to_path_buf());
        let ekey = test_ekey();
        let data = b"test file content";

        store.write(&ekey, data).await.unwrap();
        assert!(store.exists(&ekey));

        let loaded = store.read(&ekey).await.unwrap();
        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn test_read_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = LooseFileStore::new(dir.path().to_path_buf());
        let ekey = test_ekey();

        let result = store.read(&ekey).await;
        assert!(matches!(
            result,
            Err(crate::error::ContainerlessError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = LooseFileStore::new(dir.path().to_path_buf());
        let ekey = test_ekey();

        store.write(&ekey, b"data").await.unwrap();
        assert!(store.exists(&ekey));

        store.remove(&ekey).await.unwrap();
        assert!(!store.exists(&ekey));
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = LooseFileStore::new(dir.path().to_path_buf());
        // Removing a non-existent file should succeed silently.
        store.remove(&test_ekey()).await.unwrap();
    }

    #[tokio::test]
    async fn test_total_size() {
        let dir = tempfile::tempdir().unwrap();
        let store = LooseFileStore::new(dir.path().to_path_buf());

        let ekey1 = [0x01; 16];
        let ekey2 = [0x02; 16];
        store.write(&ekey1, &[0u8; 100]).await.unwrap();
        store.write(&ekey2, &[0u8; 200]).await.unwrap();

        let size = store.total_size().await.unwrap();
        assert_eq!(size, 300);
    }
}

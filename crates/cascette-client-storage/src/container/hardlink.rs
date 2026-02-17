//! Hard link container for filesystem hard links.
//!
//! Manages hard links between installations to share content without
//! duplication. Uses a trie directory structure backed by a
//! `.trie_directory` token file.
//!
//! Hard link support is probed at initialization:
//! 1. Create test file `casc_hard_link_test_file` in source directory
//! 2. Attempt to create hard link in target directory
//! 3. Clean up both files
//!
//! If the filesystem doesn't support hard links, falls back to
//! `ResidencyContainer` behavior.
//!

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::container::{AccessMode, Container};
use crate::{Result, StorageError};

/// Hard link test file name used by `casc::HardLink::TestSupport`.
const HARDLINK_TEST_FILE: &str = "casc_hard_link_test_file";

/// Trie directory token file name.
///
/// CASC uses `.trie_directory` (not `.residency`) for the hard
/// link container.
const TRIE_DIRECTORY_TOKEN: &str = ".trie_directory";

/// Hard link container for filesystem hard links.
///
/// Configuration from `tact_HardLinkContainer` (0x30 = 48 bytes):
/// - Same layout as `ResidencyContainer`
/// - Backed by a CASC trie directory at offset 0x28
/// - Agent passes 0x200 (512) as max path components
///
/// Operations:
/// - `test_support()`: Probe filesystem for hard link support
/// - `create_link()`: Create a hard link (3-retry delete before create)
/// - `validate_links()`: Verify existing hard links
/// - `remove_file()`: Remove a hard-linked file
pub struct HardLinkContainer {
    /// Whether hard links are supported on this filesystem.
    supported: bool,
    /// Access mode.
    access_mode: AccessMode,
    /// Whether the container is read-only.
    read_only: bool,
    /// Storage directory path.
    storage_path: PathBuf,
}

impl HardLinkContainer {
    /// Create a new hard link container.
    ///
    /// Call `test_support()` after creation to check filesystem support.
    pub fn new(access_mode: AccessMode, storage_path: PathBuf) -> Self {
        let read_only = access_mode == AccessMode::ReadOnly;
        Self {
            supported: false,
            access_mode,
            read_only,
            storage_path,
        }
    }

    /// Test if the filesystem supports hard links.
    ///
    /// Creates and removes test files matching CASC's
    /// `casc::HardLink::TestSupport`:
    /// 1. Delete existing test files in both directories
    /// 2. Create test file in source directory
    /// 3. Attempt hard link to target directory
    /// 4. Clean up both files
    pub fn test_support(&mut self, source_dir: &Path, target_dir: &Path) -> Result<bool> {
        let source_test = source_dir.join(HARDLINK_TEST_FILE);
        let target_test = target_dir.join(HARDLINK_TEST_FILE);

        // Clean up any existing test files
        let _ = std::fs::remove_file(&target_test);
        let _ = std::fs::remove_file(&source_test);

        // Create test file in source directory
        if let Err(e) = std::fs::write(&source_test, b"hardlink_test") {
            warn!(
                "failed to create test file at {}: {e}",
                source_test.display()
            );
            self.supported = false;
            return Ok(false);
        }

        // Attempt to create hard link
        let link_result = std::fs::hard_link(&source_test, &target_test);

        // Clean up
        let _ = std::fs::remove_file(&target_test);
        let _ = std::fs::remove_file(&source_test);

        match link_result {
            Ok(()) => {
                debug!("hard link support verified");
                self.supported = true;
                Ok(true)
            }
            Err(e) => {
                info!("hard links not supported: {e}");
                self.supported = false;
                Ok(false)
            }
        }
    }

    /// Check if hard links are supported.
    pub const fn is_supported(&self) -> bool {
        self.supported
    }

    /// Check if the container is read-only.
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Create a hard link from `source` to `destination`.
    ///
    /// CASC `tact::HardLinkContainer::CreateLink`:
    /// - Rejects zero keys (returns error code 3)
    /// - Delegates to `casc::TrieDirectory::CreateLink`
    /// - `casc::HardLink::CreateLink` wraps `CreateHardLinkA`
    ///
    /// The 3-retry delete pattern is from
    /// `tact::VerifyHardLinkFileState::Execute` with 5-second delays.
    pub fn create_link(&self, key: &[u8; 16], source: &Path, destination: &Path) -> Result<()> {
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

        // Zero-key check
        if key == &[0u8; 16] {
            return Err(StorageError::InvalidFormat(
                "zero key rejected for hard link creation".to_string(),
            ));
        }

        // 3-retry delete before creating hard link
        for attempt in 0..3 {
            if destination.exists()
                && let Err(e) = std::fs::remove_file(destination)
            {
                if attempt < 2 {
                    warn!(
                        "retry {}: failed to remove existing file at {}: {e}",
                        attempt + 1,
                        destination.display()
                    );
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    continue;
                }
                return Err(StorageError::Archive(format!(
                    "failed to remove file {} after 3 retries: {e}",
                    destination.display()
                )));
            }
            break;
        }

        std::fs::hard_link(source, destination).map_err(|e| {
            StorageError::Archive(format!(
                "failed to create hard link {} -> {}: {e}",
                destination.display(),
                source.display()
            ))
        })?;

        debug!(
            "created hard link for key {}: {} -> {}",
            hex::encode(&key[..9]),
            destination.display(),
            source.display()
        );

        Ok(())
    }

    /// Remove a hard-linked file.
    ///
    /// Silently succeeds if the file doesn't exist (matching Agent
    /// behavior for FILE_NOT_FOUND / PATH_NOT_FOUND).
    pub fn remove_file(&self, key: &[u8; 16], path: &Path) -> Result<()> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "hard link container is read-only".to_string(),
            ));
        }

        match std::fs::remove_file(path) {
            Ok(()) => {
                debug!(
                    "removed hard link file for key {}: {}",
                    hex::encode(&key[..9]),
                    path.display()
                );
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Silently succeed on file-not-found
                Ok(())
            }
            Err(e) => Err(StorageError::Archive(format!(
                "failed to remove file {}: {e}",
                path.display()
            ))),
        }
    }

    /// Initialize the container directory and token file.
    pub async fn initialize(&mut self) -> Result<()> {
        if self.access_mode.can_write() {
            tokio::fs::create_dir_all(&self.storage_path)
                .await
                .map_err(|e| {
                    StorageError::Archive(format!(
                        "failed to create hard link directory {}: {e}",
                        self.storage_path.display()
                    ))
                })?;

            let token_path = self.storage_path.join(TRIE_DIRECTORY_TOKEN);
            if !token_path.exists() {
                tokio::fs::write(&token_path, b"").await.map_err(|e| {
                    StorageError::Archive(format!(
                        "failed to create .trie_directory token at {}: {e}",
                        token_path.display()
                    ))
                })?;
            }
        }

        Ok(())
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
        // Hard link container redirects reads to the linked file.
        // The actual file read is handled by the DynamicContainer
        // or StaticContainer that owns the data.
        Err(StorageError::InvalidFormat(
            "use the linked container for reads".to_string(),
        ))
    }

    async fn write(&self, _key: &[u8; 16], _data: &[u8]) -> Result<()> {
        // Hard link container creates links, not data writes
        Err(StorageError::InvalidFormat(
            "use create_link() for hard link operations".to_string(),
        ))
    }

    async fn remove(&self, key: &[u8; 16]) -> Result<()> {
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

        // CASC `tact::HardLinkContainer::RemoveFile`
        // removes from the trie directory. We don't track the file path
        // here, so this is a partial implementation.
        debug!("remove: key={} (trie entry only)", hex::encode(&key[..9]));
        Ok(())
    }

    async fn query(&self, _key: &[u8; 16]) -> Result<bool> {
        if !self.supported {
            return Ok(false);
        }
        // Would check trie directory for key existence
        Ok(false)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hardlink_creation() {
        let dir = tempdir().expect("tempdir");
        let container = HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());
        assert!(!container.is_supported());
        assert!(!container.is_read_only());
    }

    #[test]
    fn test_test_support() {
        let dir = tempdir().expect("tempdir");
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        std::fs::create_dir_all(&source).expect("mkdir source");
        std::fs::create_dir_all(&target).expect("mkdir target");

        let mut container = HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

        // On most Unix filesystems in tmpdir, hard links are supported
        let supported = container.test_support(&source, &target).expect("test");
        // We don't assert the result since it depends on the filesystem
        assert_eq!(container.is_supported(), supported);
    }

    #[test]
    fn test_create_link_requires_support() {
        let dir = tempdir().expect("tempdir");
        let container = HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

        let key = [0xAA; 16];
        let src = dir.path().join("source_file");
        let dst = dir.path().join("dest_file");
        assert!(container.create_link(&key, &src, &dst).is_err());
    }

    #[test]
    fn test_zero_key_rejected() {
        let dir = tempdir().expect("tempdir");
        let mut container = HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());
        container.supported = true; // Force support for test

        let src = dir.path().join("source_file");
        let dst = dir.path().join("dest_file");
        let result = container.create_link(&[0u8; 16], &src, &dst);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_remove_link() {
        let dir = tempdir().expect("tempdir");
        let mut container = HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

        // Test filesystem support first
        let source_dir = dir.path().join("src");
        let target_dir = dir.path().join("dst");
        std::fs::create_dir_all(&source_dir).expect("mkdir");
        std::fs::create_dir_all(&target_dir).expect("mkdir");

        if !container
            .test_support(&source_dir, &target_dir)
            .expect("test support")
        {
            // Skip test on filesystems without hard link support
            return;
        }

        // Create a source file
        let source_file = source_dir.join("data.bin");
        std::fs::write(&source_file, b"test content").expect("write source");

        // Create hard link
        let key = [0xCC; 16];
        let link_file = target_dir.join("link.bin");
        container
            .create_link(&key, &source_file, &link_file)
            .expect("create link");

        assert!(link_file.exists());

        // Remove the link
        container
            .remove_file(&key, &link_file)
            .expect("remove link");
        assert!(!link_file.exists());

        // Remove non-existent file should succeed silently
        container
            .remove_file(&key, &link_file)
            .expect("remove missing file should succeed");
    }

    #[test]
    fn test_read_only_rejects_mutations() {
        let dir = tempdir().expect("tempdir");
        let mut container = HardLinkContainer::new(AccessMode::ReadOnly, dir.path().to_path_buf());
        container.supported = true;

        let key = [0xDD; 16];
        let path = dir.path().join("test");
        assert!(container.create_link(&key, &path, &path).is_err());
        assert!(container.remove_file(&key, &path).is_err());
    }

    #[tokio::test]
    async fn test_initialize_creates_token() {
        let dir = tempdir().expect("tempdir");
        let mut container = HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

        container.initialize().await.expect("init");
        assert!(dir.path().join(TRIE_DIRECTORY_TOKEN).exists());
    }
}

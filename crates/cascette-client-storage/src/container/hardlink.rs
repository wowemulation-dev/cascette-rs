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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use parking_lot::RwLock;
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

/// Maximum FD cache capacity.
const FD_CACHE_CAPACITY: usize = 64;

/// Format an encoding key as a trie directory path: `XX/YY/remaining_hex`.
///
/// Agent uses a 780-byte buffer with 38-byte hex suffix. The first
/// byte becomes the first two hex chars (directory), the second byte
/// becomes the next two hex chars (subdirectory), and bytes 2-8
/// become the remaining 14 hex chars (filename).
pub fn format_content_key_path(base: &Path, ekey: &[u8; 9]) -> PathBuf {
    let hex = hex::encode(ekey);
    // XX/YY/ZZZZZZZZZZZZZZ (2 + 2 + 14 = 18 hex chars for 9 bytes)
    base.join(&hex[..2]).join(&hex[2..4]).join(&hex[4..])
}

/// FD cache entry with LRU tracking.
struct FdCacheEntry {
    /// Index in the LRU list (for O(1) removal).
    lru_index: usize,
    /// Whether this path exists on disk (cached stat result).
    exists: bool,
}

/// File descriptor cache with LRU eviction.
///
/// Caches path existence checks to avoid repeated filesystem stat
/// calls. Uses a flat array doubly-linked list for O(1) LRU
/// operations matching Agent.exe's FD cache pattern.
struct FdCache {
    /// Cached entries keyed by 9-byte encoding key.
    entries: HashMap<[u8; 9], FdCacheEntry>,
    /// LRU order (most recent at front). Stores ekeys.
    lru_order: Vec<[u8; 9]>,
    /// Maximum capacity.
    capacity: usize,
}

impl FdCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Look up a key, returning cached existence if present.
    /// Moves the key to the front of the LRU list on hit.
    fn get(&mut self, key: &[u8; 9]) -> Option<bool> {
        let entry = self.entries.get(key)?;
        let exists = entry.exists;
        let idx = entry.lru_index;

        // Move to front
        if idx != 0 {
            self.lru_order.remove(idx);
            self.lru_order.insert(0, *key);
            // Update indices for shifted entries
            for (i, k) in self.lru_order.iter().enumerate() {
                if let Some(e) = self.entries.get_mut(k) {
                    e.lru_index = i;
                }
            }
        }

        Some(exists)
    }

    /// Insert or update a cache entry. Evicts the LRU entry if full.
    fn insert(&mut self, key: [u8; 9], exists: bool) {
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.exists = exists;
            let idx = entry.lru_index;
            if idx != 0 {
                self.lru_order.remove(idx);
                self.lru_order.insert(0, key);
                for (i, k) in self.lru_order.iter().enumerate() {
                    if let Some(e) = self.entries.get_mut(k) {
                        e.lru_index = i;
                    }
                }
            }
            return;
        }

        // Evict if full
        if self.entries.len() >= self.capacity
            && let Some(evicted) = self.lru_order.pop()
        {
            self.entries.remove(&evicted);
        }

        // Insert at front
        self.lru_order.insert(0, key);
        for (i, k) in self.lru_order.iter().enumerate() {
            if let Some(e) = self.entries.get_mut(k) {
                e.lru_index = i;
            }
        }
        self.entries.insert(key, FdCacheEntry { lru_index: 0, exists });
    }

    /// Invalidate a cache entry.
    fn remove(&mut self, key: &[u8; 9]) {
        if let Some(entry) = self.entries.remove(key) {
            self.lru_order.remove(entry.lru_index);
            // Update indices for shifted entries
            for (i, k) in self.lru_order.iter().enumerate() {
                if let Some(e) = self.entries.get_mut(k) {
                    e.lru_index = i;
                }
            }
        }
    }

    /// Clear the cache.
    fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
    }
}

/// Trie directory storage for hard link path management.
///
/// Organizes files in a two-level trie: `XX/YY/remaining_hex` where
/// XX and YY are the first two bytes of the encoding key in hex.
/// Provides FD-cached lookups and directory maintenance operations.
struct TrieDirectoryStorage {
    /// Base path for the trie directory.
    base_path: PathBuf,
    /// FD cache for path existence checks.
    fd_cache: FdCache,
}

impl TrieDirectoryStorage {
    fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            fd_cache: FdCache::new(FD_CACHE_CAPACITY),
        }
    }

    /// Check if a key exists in the trie directory.
    fn exists(&mut self, ekey: &[u8; 9]) -> bool {
        // Check FD cache first
        if let Some(exists) = self.fd_cache.get(ekey) {
            return exists;
        }

        // Stat the file
        let path = format_content_key_path(&self.base_path, ekey);
        let exists = path.exists();
        self.fd_cache.insert(*ekey, exists);
        exists
    }

    /// Get the full path for a key.
    fn path_for_key(&self, ekey: &[u8; 9]) -> PathBuf {
        format_content_key_path(&self.base_path, ekey)
    }

    /// Invalidate the cache entry for a key.
    fn invalidate(&mut self, ekey: &[u8; 9]) {
        self.fd_cache.remove(ekey);
    }

    /// Clean the trie directory: remove all files except `.idx` and
    /// `shmem*` files. Matches Agent.exe's `CleanDirectory`.
    fn clean_directory(&mut self) -> Result<usize> {
        let mut removed = 0;
        self.fd_cache.clear();

        if !self.base_path.exists() {
            return Ok(0);
        }

        // Walk the two-level trie structure
        let entries = std::fs::read_dir(&self.base_path).map_err(|e| {
            StorageError::Archive(format!(
                "failed to read trie directory {}: {e}",
                self.base_path.display()
            ))
        })?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip .idx files, shmem files, and token files
            if name_str.ends_with(".idx")
                || name_str.starts_with("shmem")
                || name_str == TRIE_DIRECTORY_TOKEN
            {
                continue;
            }

            let path = entry.path();
            if path.is_dir() {
                // This is a trie level-1 directory (XX/)
                removed += Self::clean_trie_subdir(&path);

                // Remove the directory if empty
                let _ = std::fs::remove_dir(&path);
            } else {
                // Remove non-trie files at root level
                if std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }

    /// Clean a trie subdirectory (level 2: XX/YY/).
    fn clean_trie_subdir(dir: &Path) -> usize {
        let mut removed = 0;
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Level-2 directory (YY/), clean its contents
                let Ok(sub_entries) = std::fs::read_dir(&path) else {
                    continue;
                };
                for sub_entry in sub_entries.flatten() {
                    if std::fs::remove_file(sub_entry.path()).is_ok() {
                        removed += 1;
                    }
                }
                let _ = std::fs::remove_dir(&path);
            } else if std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }

        removed
    }

    /// Compact the trie directory: validate structure at each depth,
    /// remove orphaned files. Matches Agent.exe's `CompactDirectory`.
    fn compact_directory(&mut self) -> Result<usize> {
        let mut removed = 0;
        self.fd_cache.clear();

        if !self.base_path.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(&self.base_path).map_err(|e| {
            StorageError::Archive(format!(
                "failed to read trie directory {}: {e}",
                self.base_path.display()
            ))
        })?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip non-trie entries
            if name_str.len() != 2 || !name_str.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }

            let l1_path = entry.path();
            if !l1_path.is_dir() {
                // Orphan file at trie root level -- remove
                if std::fs::remove_file(&l1_path).is_ok() {
                    removed += 1;
                }
                continue;
            }

            let Ok(l2_entries) = std::fs::read_dir(&l1_path) else {
                continue;
            };

            let mut l2_empty = true;
            for l2_entry in l2_entries.flatten() {
                let l2_name = l2_entry.file_name();
                let l2_name_str = l2_name.to_string_lossy();

                if l2_name_str.len() != 2 || !l2_name_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    // Orphan at level 2
                    let p = l2_entry.path();
                    if p.is_file() && std::fs::remove_file(&p).is_ok() {
                        removed += 1;
                    }
                    continue;
                }

                let l2_path = l2_entry.path();
                if !l2_path.is_dir() {
                    if std::fs::remove_file(&l2_path).is_ok() {
                        removed += 1;
                    }
                    continue;
                }

                // Validate leaf entries at level 3
                let Ok(l3_entries) = std::fs::read_dir(&l2_path) else {
                    continue;
                };

                let mut l3_empty = true;
                for l3_entry in l3_entries.flatten() {
                    let l3_name = l3_entry.file_name();
                    let l3_name_str = l3_name.to_string_lossy();

                    // Leaf files should be 14-char hex strings (7 remaining bytes)
                    if l3_name_str.len() != 14
                        || !l3_name_str.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        let p = l3_entry.path();
                        if p.is_file() && std::fs::remove_file(&p).is_ok() {
                            removed += 1;
                        }
                    } else {
                        l3_empty = false;
                    }
                }

                if l3_empty {
                    let _ = std::fs::remove_dir(&l2_path);
                } else {
                    l2_empty = false;
                }
            }

            if l2_empty {
                let _ = std::fs::remove_dir(&l1_path);
            }
        }

        Ok(removed)
    }
}

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
    /// Trie directory storage with FD cache.
    trie: RwLock<TrieDirectoryStorage>,
}

impl HardLinkContainer {
    /// Create a new hard link container.
    ///
    /// Call `test_support()` after creation to check filesystem support.
    pub fn new(access_mode: AccessMode, storage_path: PathBuf) -> Self {
        let read_only = access_mode == AccessMode::ReadOnly;
        let trie = TrieDirectoryStorage::new(storage_path.clone());
        Self {
            supported: false,
            access_mode,
            read_only,
            storage_path,
            trie: RwLock::new(trie),
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

    /// Create a hard link from `source` to the trie path derived from `key`.
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

        // Ensure trie subdirectories exist
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Archive(format!(
                    "failed to create trie directory {}: {e}",
                    parent.display()
                ))
            })?;
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

        // Update FD cache
        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&key[..9]);
        self.trie.write().fd_cache.insert(ekey, true);

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

        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&key[..9]);

        match std::fs::remove_file(path) {
            Ok(()) => {
                self.trie.write().invalidate(&ekey);
                debug!(
                    "removed hard link file for key {}: {}",
                    hex::encode(ekey),
                    path.display()
                );
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Silently succeed on file-not-found
                self.trie.write().invalidate(&ekey);
                Ok(())
            }
            Err(e) => Err(StorageError::Archive(format!(
                "failed to remove file {}: {e}",
                path.display()
            ))),
        }
    }

    /// Delete keys using two-phase approach.
    ///
    /// Phase 1: Collect candidates where nlink <= 1 (not shared).
    /// Phase 2: Remove collected files.
    ///
    /// This prevents removing files that are still shared by other
    /// installations.
    pub fn delete_keys(&self, keys: &[[u8; 16]]) -> Result<usize> {
        if self.read_only {
            return Err(StorageError::AccessDenied(
                "hard link container is read-only".to_string(),
            ));
        }

        let trie = self.trie.read();

        // Phase 1: Collect unlinked candidates
        let mut candidates: Vec<(PathBuf, [u8; 9])> = Vec::new();
        for key in keys {
            let mut ekey = [0u8; 9];
            ekey.copy_from_slice(&key[..9]);
            let path = trie.path_for_key(&ekey);

            if let Ok(metadata) = std::fs::metadata(&path) {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if metadata.nlink() <= 1 {
                        candidates.push((path, ekey));
                    }
                }
                #[cfg(not(unix))]
                {
                    // On non-Unix, always remove (no nlink check available)
                    let _ = metadata;
                    candidates.push((path, ekey));
                }
            }
        }

        drop(trie);

        // Phase 2: Remove collected files
        let mut removed = 0;
        let mut removed_ekeys = Vec::new();
        for (path, ekey) in &candidates {
            if std::fs::remove_file(path).is_ok() {
                removed_ekeys.push(*ekey);
                removed += 1;
            }
        }

        // Invalidate cache entries
        for ekey in &removed_ekeys {
            self.trie.write().invalidate(ekey);
        }

        Ok(removed)
    }

    /// Clean the trie directory: remove all files except `.idx` and
    /// `shmem*` files.
    pub fn clean_directory(&self) -> Result<usize> {
        self.trie.write().clean_directory()
    }

    /// Compact the trie directory: validate structure at each depth,
    /// remove orphaned files.
    pub fn compact_directory(&self) -> Result<usize> {
        self.trie.write().compact_directory()
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

        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&key[..9]);
        let path = self.trie.read().path_for_key(&ekey);
        self.remove_file(key, &path)
    }

    async fn query(&self, key: &[u8; 16]) -> Result<bool> {
        if !self.supported {
            return Ok(false);
        }
        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&key[..9]);
        Ok(self.trie.write().exists(&ekey))
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

        let mut container =
            HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

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
        let mut container =
            HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());
        container.supported = true; // Force support for test

        let src = dir.path().join("source_file");
        let dst = dir.path().join("dest_file");
        let result = container.create_link(&[0u8; 16], &src, &dst);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_remove_link() {
        let dir = tempdir().expect("tempdir");
        let mut container =
            HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

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
        let mut container =
            HardLinkContainer::new(AccessMode::ReadOnly, dir.path().to_path_buf());
        container.supported = true;

        let key = [0xDD; 16];
        let path = dir.path().join("test");
        assert!(container.create_link(&key, &path, &path).is_err());
        assert!(container.remove_file(&key, &path).is_err());
    }

    #[tokio::test]
    async fn test_initialize_creates_token() {
        let dir = tempdir().expect("tempdir");
        let mut container =
            HardLinkContainer::new(AccessMode::ReadWrite, dir.path().to_path_buf());

        container.initialize().await.expect("init");
        assert!(dir.path().join(TRIE_DIRECTORY_TOKEN).exists());
    }

    #[test]
    fn test_format_content_key_path() {
        let base = Path::new("/data/trie");
        let ekey: [u8; 9] = [0xAB, 0xCD, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE];
        let path = format_content_key_path(base, &ekey);
        assert_eq!(
            path,
            PathBuf::from("/data/trie/ab/cd/12345678_9abcde")
                .parent()
                .unwrap()
                .join("123456789abcde")
        );
        // Verify trie structure: XX/YY/remaining
        let components: Vec<_> = path
            .strip_prefix(base)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        assert_eq!(components.len(), 3);
        assert_eq!(components[0], "ab");
        assert_eq!(components[1], "cd");
        assert_eq!(components[2], "123456789abcde");
    }

    #[test]
    fn test_fd_cache_lru_eviction() {
        let mut cache = FdCache::new(3);

        // Fill cache
        cache.insert([1; 9], true);
        cache.insert([2; 9], true);
        cache.insert([3; 9], false);

        // All entries present
        assert_eq!(cache.get(&[1; 9]), Some(true));
        assert_eq!(cache.get(&[2; 9]), Some(true));
        assert_eq!(cache.get(&[3; 9]), Some(false));

        // Insert a 4th entry -- should evict least recently used
        // After the gets above, LRU order is: [3], [2], [1]
        // So [1] was most recently accessed (reordered by get)
        // Actually: get([1]) moves 1 to front, get([2]) moves 2 to front, get([3]) moves 3 to front
        // Order: [3], [2], [1] -> inserting [4] should evict [1]
        cache.insert([4; 9], true);
        assert_eq!(cache.get(&[1; 9]), None); // evicted
        assert_eq!(cache.get(&[4; 9]), Some(true));
    }

    #[test]
    fn test_fd_cache_update() {
        let mut cache = FdCache::new(4);
        cache.insert([1; 9], false);
        assert_eq!(cache.get(&[1; 9]), Some(false));

        // Update existing entry
        cache.insert([1; 9], true);
        assert_eq!(cache.get(&[1; 9]), Some(true));
    }

    #[test]
    fn test_trie_directory_exists() {
        let dir = tempdir().expect("tempdir");
        let mut trie = TrieDirectoryStorage::new(dir.path().to_path_buf());

        let ekey: [u8; 9] = [0xAB, 0xCD, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];

        // File does not exist
        assert!(!trie.exists(&ekey));

        // Create the file
        let path = trie.path_for_key(&ekey);
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, b"data").expect("write");

        // Invalidate cache so next check goes to disk
        trie.invalidate(&ekey);
        assert!(trie.exists(&ekey));
    }

    #[test]
    fn test_clean_directory() {
        let dir = tempdir().expect("tempdir");
        let base = dir.path().to_path_buf();
        let mut trie = TrieDirectoryStorage::new(base.clone());

        // Create some trie files
        let ekey1: [u8; 9] = [0xAA, 0xBB, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let ekey2: [u8; 9] = [0xCC, 0xDD, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E];
        let path1 = format_content_key_path(&base, &ekey1);
        let path2 = format_content_key_path(&base, &ekey2);
        std::fs::create_dir_all(path1.parent().unwrap()).expect("mkdir");
        std::fs::create_dir_all(path2.parent().unwrap()).expect("mkdir");
        std::fs::write(&path1, b"data1").expect("write");
        std::fs::write(&path2, b"data2").expect("write");

        // Create files that should be preserved
        std::fs::write(base.join("00000000.idx"), b"idx").expect("write idx");
        std::fs::write(base.join("shmem_v4"), b"shmem").expect("write shmem");

        let removed = trie.clean_directory().expect("clean");
        assert_eq!(removed, 2);

        // Preserved files should still exist
        assert!(base.join("00000000.idx").exists());
        assert!(base.join("shmem_v4").exists());
    }

    #[test]
    fn test_compact_directory_removes_orphans() {
        let dir = tempdir().expect("tempdir");
        let base = dir.path().to_path_buf();
        let mut trie = TrieDirectoryStorage::new(base.clone());

        // Create valid trie structure
        let valid_path = base.join("ab").join("cd").join("123456789abcde");
        std::fs::create_dir_all(valid_path.parent().unwrap()).expect("mkdir");
        std::fs::write(&valid_path, b"valid").expect("write");

        // Create orphan file with wrong name length
        let orphan_dir = base.join("ab").join("cd");
        std::fs::write(orphan_dir.join("bad"), b"orphan").expect("write orphan");

        let removed = trie.compact_directory().expect("compact");
        assert_eq!(removed, 1); // orphan removed

        // Valid file should still exist
        assert!(valid_path.exists());
    }

    #[test]
    fn test_two_phase_delete() {
        let dir = tempdir().expect("tempdir");
        let base = dir.path().to_path_buf();
        let mut container = HardLinkContainer::new(AccessMode::ReadWrite, base.clone());
        container.supported = true;

        // Create trie files
        let keys: Vec<[u8; 16]> = (0..5u8)
            .map(|i| {
                let mut k = [0u8; 16];
                k[0] = i;
                k[1] = 0xAA;
                k
            })
            .collect();

        for key in &keys {
            let mut ekey = [0u8; 9];
            ekey.copy_from_slice(&key[..9]);
            let path = format_content_key_path(&base, &ekey);
            std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
            std::fs::write(&path, b"data").expect("write");
        }

        // Delete the keys (all have nlink=1, so all should be removed)
        let removed = container.delete_keys(&keys).expect("delete");
        assert_eq!(removed, 5);
    }

    #[tokio::test]
    async fn test_query_returns_actual_result() {
        let dir = tempdir().expect("tempdir");
        let base = dir.path().to_path_buf();
        let mut container = HardLinkContainer::new(AccessMode::ReadWrite, base.clone());
        container.supported = true;

        let key = [0xAB; 16];

        // Query should return false for non-existent key
        assert!(!container.query(&key).await.expect("query"));

        // Create the trie file
        let mut ekey = [0u8; 9];
        ekey.copy_from_slice(&key[..9]);
        let path = format_content_key_path(&base, &ekey);
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, b"data").expect("write");

        // Invalidate cache
        container.trie.write().invalidate(&ekey);

        // Query should now return true
        assert!(container.query(&key).await.expect("query"));
    }
}

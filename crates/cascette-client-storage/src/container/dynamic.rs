//! Dynamic container for read-write CASC archive storage.
//!
//! This is the primary container type. Most read and write operations
//! go through the dynamic container. It manages archive segments, the
//! Key Mapping Table (KMT), and coordinates with shared memory for
//! multi-process access.
//!
//! Configuration struct: `tact_DynamicContainerConfig` (40 bytes).
//!

use std::path::PathBuf;
use std::sync::Arc;

use cascette_crypto::EncodingKey;
use parking_lot::RwLock;
use tracing::{debug, warn};

use crate::container::residency::ResidencyContainer;
use crate::container::{AccessMode, Container};
use crate::index::{IndexManager, UpdateStatus};
use crate::lru::LruManager;
use crate::storage::archive_file::ArchiveManager;
use crate::storage::segment::SegmentAllocator;
use crate::{Result, StorageError};

/// Dynamic container for read-write CASC archive storage.
///
/// Configuration fields:
/// - `access_mode`: How the container is opened
/// - `shared_memory`: Enable shmem control block
/// - `storage_path`: Base directory for archive files
/// - `segment_limit`: Maximum segments, capped at 0x3FF (1023)
/// - `max_segment_size`: Maximum bytes per segment
/// - `free_space_reclaim`: Enable free space reclamation
pub struct DynamicContainer {
    /// Access mode for this container.
    access_mode: AccessMode,
    /// Base storage directory.
    storage_path: PathBuf,
    /// Enable shared memory coordination.
    shared_memory: bool,
    /// Container is opened read-only (access_mode == ReadOnly).
    read_only: bool,
    /// Maximum number of segments (capped at 0x3FF).
    segment_limit: u16,
    /// Maximum bytes per archive segment.
    max_segment_size: u64,
    /// Enable free space reclamation during writes.
    free_space_reclaim: bool,
    /// Path hash for segment header key generation.
    path_hash: [u8; 16],
    /// Index manager for key-to-location mapping (KMT).
    ///
    /// Wrapped in `RwLock` because the Container trait takes `&self`
    /// but mutations (add/remove entries) need `&mut`.
    index: RwLock<IndexManager>,
    /// Archive manager for data file I/O.
    ///
    /// Wrapped in `RwLock` for the same reason as `index`.
    archive: RwLock<ArchiveManager>,
    /// Segment allocator for write path.
    ///
    /// Manages frozen/thawed segments and allocates space for new
    /// data writes.
    segment_allocator: RwLock<SegmentAllocator>,
    /// Optional residency container for truncation tracking.
    ///
    /// When set, truncated reads mark the affected span as
    /// non-resident in the residency container.
    residency: Option<Arc<ResidencyContainer>>,
    /// Optional LRU cache manager.
    ///
    /// When set, read and write operations touch the key to keep
    /// recently-accessed data from being evicted.
    lru: Option<Arc<RwLock<LruManager>>>,
}

/// Maximum number of archive segments .
pub const MAX_SEGMENTS: u16 = 0x3FF;

/// Builder for `DynamicContainer`.
///
/// Absorbs new parameters (path_hash, residency, lru, shmem)
/// without breaking the existing API.
pub struct DynamicContainerBuilder {
    access_mode: AccessMode,
    storage_path: PathBuf,
    shared_memory: bool,
    segment_limit: u16,
    max_segment_size: u64,
    free_space_reclaim: bool,
    path_hash: [u8; 16],
    residency: Option<Arc<ResidencyContainer>>,
    lru: Option<Arc<RwLock<LruManager>>>,
}

impl DynamicContainerBuilder {
    /// Create a new builder with the storage path.
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            access_mode: AccessMode::ReadWrite,
            storage_path,
            shared_memory: false,
            segment_limit: MAX_SEGMENTS,
            max_segment_size: crate::storage::segment::SEGMENT_SIZE,
            free_space_reclaim: false,
            path_hash: [0u8; 16],
            residency: None,
            lru: None,
        }
    }

    /// Set the access mode.
    #[must_use]
    pub const fn access_mode(mut self, mode: AccessMode) -> Self {
        self.access_mode = mode;
        self
    }

    /// Enable or disable shared memory.
    #[must_use]
    pub const fn shared_memory(mut self, enabled: bool) -> Self {
        self.shared_memory = enabled;
        self
    }

    /// Set the maximum number of segments.
    #[must_use]
    pub const fn segment_limit(mut self, limit: u16) -> Self {
        self.segment_limit = limit;
        self
    }

    /// Set the maximum segment size.
    #[must_use]
    pub const fn max_segment_size(mut self, size: u64) -> Self {
        self.max_segment_size = size;
        self
    }

    /// Enable or disable free space reclamation.
    #[must_use]
    pub const fn free_space_reclaim(mut self, enabled: bool) -> Self {
        self.free_space_reclaim = enabled;
        self
    }

    /// Set the path hash for segment header key generation.
    #[must_use]
    pub const fn path_hash(mut self, hash: [u8; 16]) -> Self {
        self.path_hash = hash;
        self
    }

    /// Set the residency container for truncation tracking.
    #[must_use]
    pub fn residency(mut self, container: Arc<ResidencyContainer>) -> Self {
        self.residency = Some(container);
        self
    }

    /// Set the LRU cache manager.
    #[must_use]
    pub fn lru(mut self, manager: Arc<RwLock<LruManager>>) -> Self {
        self.lru = Some(manager);
        self
    }

    /// Build the `DynamicContainer`.
    ///
    /// Returns `StorageError::Config` if `storage_path` is empty.
    pub fn build(self) -> Result<DynamicContainer> {
        DynamicContainer::new_from_builder(self)
    }
}

impl DynamicContainer {
    /// Create a new dynamic container.
    ///
    /// This only sets up the configuration. Call [`open`](Self::open) to
    /// load index files and open archive data files.
    ///
    /// Returns `StorageError::Config` if `storage_path` is empty.
    pub fn new(
        access_mode: AccessMode,
        storage_path: PathBuf,
        shared_memory: bool,
        segment_limit: u16,
        max_segment_size: u64,
        free_space_reclaim: bool,
    ) -> Result<Self> {
        DynamicContainerBuilder::new(storage_path)
            .access_mode(access_mode)
            .shared_memory(shared_memory)
            .segment_limit(segment_limit)
            .max_segment_size(max_segment_size)
            .free_space_reclaim(free_space_reclaim)
            .build()
    }

    /// Create a new builder for configuring a dynamic container.
    pub fn builder(storage_path: PathBuf) -> DynamicContainerBuilder {
        DynamicContainerBuilder::new(storage_path)
    }

    fn new_from_builder(b: DynamicContainerBuilder) -> Result<Self> {
        if b.storage_path.as_os_str().is_empty() {
            return Err(StorageError::Config(
                "storage path is required for DynamicContainer".to_string(),
            ));
        }

        let segment_limit = b.segment_limit.min(MAX_SEGMENTS);
        let read_only = b.access_mode == AccessMode::ReadOnly;

        let index = IndexManager::new(&b.storage_path);
        let archive = ArchiveManager::new(&b.storage_path);
        let segment_allocator =
            SegmentAllocator::new(b.storage_path.clone(), b.path_hash, segment_limit);

        Ok(Self {
            access_mode: b.access_mode,
            storage_path: b.storage_path,
            shared_memory: b.shared_memory,
            read_only,
            segment_limit,
            max_segment_size: b.max_segment_size,
            free_space_reclaim: b.free_space_reclaim,
            path_hash: b.path_hash,
            index: RwLock::new(index),
            archive: RwLock::new(archive),
            segment_allocator: RwLock::new(segment_allocator),
            residency: b.residency,
            lru: b.lru,
        })
    }

    /// Open the container: load index files and open archive data files.
    ///
    /// Must be called before any read/write operations.
    pub async fn open(&self) -> Result<()> {
        debug!(
            "Opening DynamicContainer at {}",
            self.storage_path.display()
        );

        // Ensure the storage directory exists for writable containers
        if self.access_mode.can_write() {
            tokio::fs::create_dir_all(&self.storage_path)
                .await
                .map_err(|e| {
                    StorageError::Archive(format!(
                        "failed to create storage directory {}: {e}",
                        self.storage_path.display()
                    ))
                })?;
        }

        // Load index files (KMT).
        // Take ownership briefly to avoid holding the lock across await.
        let mut index = std::mem::replace(
            &mut *self.index.write(),
            IndexManager::new(&self.storage_path),
        );
        index.load_all().await?;
        *self.index.write() = index;

        // Open archive data files.
        let mut archive = std::mem::replace(
            &mut *self.archive.write(),
            ArchiveManager::new(&self.storage_path),
        );
        archive.open_all().await?;
        *self.archive.write() = archive;

        // Load existing segments.
        self.segment_allocator.write().load_existing()?;

        let entry_count = self.index.read().entry_count();
        let archive_count = self.archive.read().stats().archive_count;
        let segment_count = self.segment_allocator.read().segment_count();
        debug!(
            "DynamicContainer opened: {} index entries, {} archives, {} segments",
            entry_count, archive_count, segment_count,
        );

        Ok(())
    }

    /// Get the access mode.
    pub const fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    /// Get the storage path.
    pub fn storage_path(&self) -> &PathBuf {
        &self.storage_path
    }

    /// Check if the container is read-only.
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Get the segment limit.
    pub const fn segment_limit(&self) -> u16 {
        self.segment_limit
    }

    /// Get the maximum segment size.
    pub const fn max_segment_size(&self) -> u64 {
        self.max_segment_size
    }

    /// Check if shared memory is enabled.
    pub const fn shared_memory_enabled(&self) -> bool {
        self.shared_memory
    }

    /// Check if free space reclamation is enabled.
    pub const fn free_space_reclaim(&self) -> bool {
        self.free_space_reclaim
    }

    /// Get the path hash.
    pub const fn path_hash(&self) -> &[u8; 16] {
        &self.path_hash
    }

    /// Get the number of archive segments.
    pub fn segment_count(&self) -> usize {
        self.segment_allocator.read().segment_count()
    }

    /// Flush the KMT update section for a specific bucket.
    ///
    /// Merges update entries into the sorted section with atomic
    /// file replacement.
    #[allow(clippy::significant_drop_tightening)]
    pub fn flush_bucket(&self, bucket: u8) -> Result<()> {
        let alloc = self.segment_allocator.read();
        let _lock = alloc.bucket_write_lock(bucket);
        let mut index = self.index.write();
        index.flush_updates_for_bucket(bucket)?;
        Ok(())
    }

    /// Flush all KMT update sections.
    pub fn flush_all_updates(&self) -> Result<()> {
        let mut index = self.index.write();
        index.flush_all_updates()
    }

    /// Handle a truncated read by marking the affected span as
    /// non-resident and updating the KMT entry status.
    ///
    /// Called when `read_content` fails due to the archive being
    /// shorter than the entry's recorded size.
    fn handle_truncated_read(&self, key: &[u8; 16], archive_offset: u32, entry_size: u32) {
        // Mark span non-resident in residency container
        if let Some(ref residency) = self.residency {
            let offset = i32::try_from(archive_offset).unwrap_or(i32::MAX);
            let length = i32::try_from(entry_size).unwrap_or(i32::MAX);
            if let Err(e) = residency.mark_span_non_resident(key, offset, length) {
                warn!(
                    "failed to mark span non-resident for key {}: {e}",
                    hex::encode(&key[..9])
                );
            }
        }

        // Update KMT entry status to DATA_NON_RESIDENT (7)
        let ekey = EncodingKey::from_bytes(*key);
        let mut index = self.index.write();
        index.update_entry_status(&ekey, UpdateStatus::DataNonResident);
    }

    /// Get the number of indexed entries.
    pub fn entry_count(&self) -> usize {
        self.index.read().entry_count()
    }

    /// Remove a byte span from an archive entry.
    ///
    /// CASC's `casc::Dynamic::RemoveSpan` adjusts the
    /// offset by +0x1E (LOCAL_HEADER_SIZE) to account for the local header
    /// before the BLTE data. It silently succeeds on FILE_NOT_FOUND and
    /// PATH_NOT_FOUND errors.
    pub fn remove_span(&self, key: &[u8; 16], offset: u64, length: u64) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }

        // Agent adjusts offset by +0x1E before calling the span removal.
        // Span removal is part of maintenance operations (deferred).
        // This is a no-op matching Agent's behavior of silently
        // succeeding when the file is not found.
        debug!(
            "remove_span: key={}, offset={:#x}+0x1E, length={:#x} (no-op)",
            hex::encode(&key[..9]),
            offset,
            length
        );

        Ok(())
    }
}

impl Container for DynamicContainer {
    async fn reserve(&self, _key: &[u8; 16]) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }
        // Reservation is handled implicitly during write.
        // CASC's allocate path is part of ContainerIndex which
        // we handle inside write().
        Ok(())
    }

    async fn read(&self, key: &[u8; 16], _offset: u64, _len: u32, buf: &mut [u8]) -> Result<usize> {
        if !self.access_mode.can_read() {
            return Err(StorageError::AccessDenied(
                "container has no read access".to_string(),
            ));
        }

        // Look up key in index (KMT)
        let ekey = EncodingKey::from_bytes(*key);
        let entry = {
            let index = self.index.read();
            index.lookup(&ekey).ok_or_else(|| {
                StorageError::NotFound(format!("key {} not in index", hex::encode(&key[..9])))
            })?
        };

        let archive_id = entry.archive_id();
        let archive_offset = entry.archive_offset();
        let entry_size = entry.size;

        // Read from archive.
        // Truncation detection: CASC's `casc::Dynamic::Read`
        // checks if bytes_read < expected_size at the raw I/O level. If the
        // archive file on disk is shorter than entry_size, `read_raw` (called
        // by `read_content`) returns an Archive error which we convert to
        // TruncatedRead.
        let data = {
            let archive = self.archive.read();
            match archive.read_content(archive_id, archive_offset, entry_size) {
                Ok(data) => data,
                Err(e) => {
                    // Convert archive bounds errors to TruncatedRead to match
                    // CASC behavior (CASC error 3 -> TACT error 7).
                    if matches!(&e, StorageError::Archive(msg) if msg.contains("beyond archive bounds"))
                    {
                        warn!(
                            "truncated read for key {}: archive {} too short for entry at offset {:#x} size {}",
                            hex::encode(&key[..9]),
                            archive_id,
                            archive_offset,
                            entry_size,
                        );

                        // Truncation tracking: mark span non-resident and
                        // update KMT entry status to DATA_NON_RESIDENT (7).
                        self.handle_truncated_read(key, archive_offset, entry_size);

                        return Err(StorageError::TruncatedRead(format!(
                            "key {}: archive file truncated",
                            hex::encode(&key[..9]),
                        )));
                    }
                    return Err(e);
                }
            }
        };

        // Touch LRU cache to keep this key from eviction.
        if let Some(ref lru) = self.lru {
            let ekey_9: [u8; 9] = key[..9].try_into().unwrap_or([0; 9]);
            lru.write().touch(&ekey_9);
        }

        // Copy to output buffer
        let copy_len = data.len().min(buf.len());
        buf[..copy_len].copy_from_slice(&data[..copy_len]);

        Ok(copy_len)
    }

    async fn write(&self, key: &[u8; 16], data: &[u8]) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }

        // CASC `casc::Dynamic::Write`:
        // 1. Validates access mode == 2 (ReadWrite)
        // 2. Checks total_size = data.len() + 0x1E fits in a segment
        // 3. Allocates via ContainerIndex (selects archive + offset)
        // 4. Writes 30-byte header at storage offset, data at offset+0x1E
        // 5. Updates KMT with new entry

        // Write to archive (ArchiveManager handles BLTE encoding,
        // local header, and archive selection)
        let (archive_id, offset, total_size, encoding_key) = {
            let mut archive = self.archive.write();
            archive.write_content(data, false)?
        };

        debug!(
            "wrote key {} to archive {} at offset {:#x}, size {}",
            hex::encode(&key[..9]),
            archive_id,
            offset,
            total_size,
        );

        // Update index (KMT) with the new entry.
        // The key stored in the index is the first 9 bytes of the
        // encoding key (MD5 of BLTE data), not the content key passed in.
        {
            let mut index = self.index.write();
            index.add_entry(
                &EncodingKey::from_bytes(encoding_key),
                archive_id,
                offset,
                total_size,
            )?;
        }

        // Touch LRU cache to keep this key from eviction.
        if let Some(ref lru) = self.lru {
            let ekey_9: [u8; 9] = encoding_key[..9].try_into().unwrap_or([0; 9]);
            lru.write().touch(&ekey_9);
        }

        // Persist the updated index to disk
        {
            let index = self.index.read();
            index.save_all()?;
        }

        Ok(())
    }

    async fn remove(&self, key: &[u8; 16]) -> Result<()> {
        if !self.access_mode.can_write() {
            return Err(StorageError::AccessDenied(
                "container is read-only".to_string(),
            ));
        }

        // CASC `casc::Dynamic::Remove`
        // delegates to `DeleteKeys(arg1, arg2, 1)`.
        let ekey = EncodingKey::from_bytes(*key);
        let removed = {
            let mut index = self.index.write();
            index.remove_entry(&ekey)
        };

        if removed {
            debug!("removed key {} from index", hex::encode(&key[..9]));
            // Persist the updated index
            let index = self.index.read();
            index.save_all()?;
        }

        Ok(())
    }

    async fn query(&self, key: &[u8; 16]) -> Result<bool> {
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
    fn test_dynamic_container_creation() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        assert_eq!(container.access_mode(), AccessMode::ReadWrite);
        assert!(!container.is_read_only());
        assert_eq!(container.segment_limit(), 100);
    }

    #[test]
    fn test_segment_limit_capped() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            2000, // Exceeds MAX_SEGMENTS
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        assert_eq!(container.segment_limit(), MAX_SEGMENTS);
    }

    #[test]
    fn test_empty_path_rejected() {
        let result = DynamicContainer::new(
            AccessMode::ReadWrite,
            PathBuf::new(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_only_rejects_writes() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadOnly,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        let key = [0u8; 16];
        let result = container.write(&key, b"data").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_read_round_trip() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        // Open the container (creates storage directory)
        container.open().await.expect("open");

        let test_data = b"Hello from DynamicContainer write-read test!";
        let key = [0xABu8; 16];

        // Write data
        container
            .write(&key, test_data)
            .await
            .expect("write should succeed");

        // The write stores the data keyed by its encoding key (MD5 of BLTE).
        // To read it back, we need the encoding key that was generated.
        // Get it from the index by iterating.
        let entry = {
            let index = container.index.read();
            let mut entries: Vec<_> = index.iter_entries().collect();
            drop(index);
            assert_eq!(entries.len(), 1, "should have exactly one entry");
            entries.pop().expect("entry").1.clone()
        };

        // Reconstruct the full 16-byte key from the 9-byte truncated key
        let mut ekey = [0u8; 16];
        ekey[..9].copy_from_slice(&entry.key);

        // Read it back
        let mut buf = vec![0u8; test_data.len() + 64]; // extra space
        let bytes_read = container
            .read(&ekey, 0, 0, &mut buf)
            .await
            .expect("read should succeed");

        assert_eq!(&buf[..bytes_read], test_data);
    }

    #[tokio::test]
    async fn test_query_after_write() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        container.open().await.expect("open");

        let key = [0xCDu8; 16];
        let test_data = b"query test data";

        // Before write: key should not exist
        // (the encoding key won't match since we use the content key here)
        assert!(!container.query(&key).await.expect("query"));

        // Write
        container.write(&key, test_data).await.expect("write");

        // The encoding key is different from the content key,
        // so querying with the content key still returns false.
        // Query with the actual encoding key from the index.
        let ekey = {
            let index = container.index.read();
            let entry_key = index.iter_entries().next().expect("one entry").1.key;
            drop(index);
            let mut k = [0u8; 16];
            k[..9].copy_from_slice(&entry_key);
            k
        };

        assert!(container.query(&ekey).await.expect("query with ekey"));
    }

    #[tokio::test]
    async fn test_remove_entry() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        container.open().await.expect("open");

        let key = [0xEFu8; 16];
        container.write(&key, b"remove test").await.expect("write");

        // Get the encoding key
        let ekey = {
            let index = container.index.read();
            let entry_key = index.iter_entries().next().expect("entry").1.key;
            drop(index);
            let mut k = [0u8; 16];
            k[..9].copy_from_slice(&entry_key);
            k
        };

        assert!(container.query(&ekey).await.expect("query before remove"));

        // Remove
        container.remove(&ekey).await.expect("remove");

        assert!(
            !container.query(&ekey).await.expect("query after remove"),
            "key should be gone after remove"
        );
    }

    #[tokio::test]
    async fn test_open_creates_directory() {
        let dir = tempdir().expect("tempdir");
        let sub = dir.path().join("nested").join("storage");

        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            sub.clone(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        assert!(!sub.exists());
        container.open().await.expect("open");
        assert!(sub.exists());
    }

    #[test]
    fn test_remove_span_read_only_rejected() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadOnly,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        let key = [0u8; 16];
        assert!(container.remove_span(&key, 0, 100).is_err());
    }

    #[tokio::test]
    async fn test_not_found_returns_error() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        container.open().await.expect("open");

        let key = [0x42u8; 16];
        let mut buf = [0u8; 64];
        let result = container.read(&key, 0, 0, &mut buf).await;
        assert!(
            matches!(result, Err(StorageError::NotFound(_))),
            "reading a missing key should return NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_entry_count() {
        let dir = tempdir().expect("tempdir");
        let container = DynamicContainer::new(
            AccessMode::ReadWrite,
            dir.path().to_path_buf(),
            false,
            100,
            1024 * 1024 * 1024,
            false,
        )
        .expect("create");

        container.open().await.expect("open");
        assert_eq!(container.entry_count(), 0);

        container
            .write(&[0xAAu8; 16], b"data1")
            .await
            .expect("write1");
        assert_eq!(container.entry_count(), 1);

        container
            .write(&[0xBBu8; 16], b"data2")
            .await
            .expect("write2");
        assert_eq!(container.entry_count(), 2);
    }
}

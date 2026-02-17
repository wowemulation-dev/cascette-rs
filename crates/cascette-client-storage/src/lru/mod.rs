//! LRU cache with generation-based checkpoints.
//!
//! CASC maintains an LRU cache as a flat-file doubly-linked list
//! with a hash map for O(1) key lookups. The list is stored in shared
//! memory and checkpointed to disk as `.lru` files with generation-
//! numbered filenames.
//!
//! The LRU manager tracks 9-byte truncated encoding keys and supports:
//! - O(1) touch (move to MRU head)
//! - O(1) evict (remove from LRU tail)
//! - Generation-based checkpointing with old file cleanup
//! - Persistence via `.lru` file format
//!

pub mod lru_file;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

use lru_file::{
    LRU_SENTINEL, LruFileEntry, LruFileHeader, deserialize, generation_to_filename, lru_file_path,
    serialize,
};

/// LRU cache manager using a flat-array doubly-linked list.
///
/// The entry array forms a doubly-linked list via `prev`/`next` index
/// fields. A hash map provides O(1) lookup from 9-byte encoding key
/// to array index. The header tracks the MRU head and LRU tail.
///
/// - +0x40: generation_lo (u32)
/// - +0x44: generation_hi (u32)
/// - +0x48: prev_generation_lo (u32)
/// - +0x4C: prev_generation_hi (u32)
pub struct LruManager {
    /// File header with MRU head and LRU tail indices.
    header: LruFileHeader,
    /// Flat array of entries forming a doubly-linked list.
    entries: Vec<LruFileEntry>,
    /// Hash map from 9-byte key to entry index for O(1) lookup.
    key_map: HashMap<[u8; 9], u32>,
    /// Free list: indices of unused entries.
    free_list: Vec<u32>,
    /// Current generation counter (never 0 when active).
    generation: u64,
    /// Previous generation (for old file cleanup).
    prev_generation: u64,
    /// Maximum number of entries.
    capacity: u32,
    /// Directory where `.lru` files are stored.
    data_dir: PathBuf,
}

impl LruManager {
    /// Create a new LRU manager with the given capacity and data directory.
    pub fn new(capacity: u32, data_dir: PathBuf) -> Self {
        let entries = vec![LruFileEntry::empty(); capacity as usize];
        let free_list: Vec<u32> = (0..capacity).collect();

        Self {
            header: LruFileHeader::default(),
            entries,
            key_map: HashMap::with_capacity(capacity as usize),
            free_list,
            generation: 1, // Never 0
            prev_generation: 0,
            capacity,
            data_dir,
        }
    }

    /// Touch a key, moving it to the MRU head.
    ///
    /// If the key already exists, it is unlinked from its current position
    /// and moved to the head. If it doesn't exist and there is capacity,
    /// a new entry is allocated. If at capacity, the LRU tail is evicted
    /// first.
    ///
    /// Returns `true` if the key was successfully touched.
    pub fn touch(&mut self, ekey: &[u8; 9]) -> bool {
        // If already present, unlink and move to head
        if let Some(&idx) = self.key_map.get(ekey) {
            self.unlink(idx);
            self.link_at_head(idx);
            return true;
        }

        // Allocate a new slot
        let idx = if let Some(free_idx) = self.free_list.pop() {
            free_idx
        } else {
            // Evict LRU tail to make room
            let Some(evicted) = self.evict_tail() else {
                return false;
            };
            evicted
        };

        // Initialize the entry
        self.entries[idx as usize] = LruFileEntry {
            prev: LRU_SENTINEL,
            next: LRU_SENTINEL,
            ekey: *ekey,
            flags: 0,
        };
        self.key_map.insert(*ekey, idx);
        self.link_at_head(idx);

        true
    }

    /// Evict the least recently used entry (LRU tail).
    ///
    /// Returns the freed slot index, or `None` if the list is empty.
    ///
    pub fn evict_tail(&mut self) -> Option<u32> {
        let tail = self.header.lru_tail;
        if tail == LRU_SENTINEL {
            return None;
        }

        let entry = self.entries[tail as usize];
        self.key_map.remove(&entry.ekey);
        self.unlink(tail);
        self.entries[tail as usize] = LruFileEntry::empty();

        Some(tail)
    }

    /// Remove a specific key from the cache.
    ///
    /// Returns `true` if the key was found and removed.
    pub fn remove(&mut self, ekey: &[u8; 9]) -> bool {
        let Some(idx) = self.key_map.remove(ekey) else {
            return false;
        };

        self.unlink(idx);
        self.entries[idx as usize] = LruFileEntry::empty();
        self.free_list.push(idx);

        true
    }

    /// Check if a key is in the cache.
    pub fn contains(&self, ekey: &[u8; 9]) -> bool {
        self.key_map.contains_key(ekey)
    }

    /// Bump the generation counter.
    ///
    /// Generation 0 is reserved; wraps from u64::MAX to 1.
    pub fn bump_generation(&mut self) {
        self.prev_generation = self.generation;
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.generation = 1;
        }
    }

    /// Get the current generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Get the previous generation.
    pub const fn prev_generation(&self) -> u64 {
        self.prev_generation
    }

    /// Get the number of active entries.
    pub fn len(&self) -> usize {
        self.key_map.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.key_map.is_empty()
    }

    /// Get the capacity.
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Checkpoint the current state to disk.
    ///
    /// 1. Serialize the table with MD5 hash
    /// 2. Write to new generation file
    /// 3. Delete previous generation file
    pub async fn checkpoint_to_disk(&mut self) -> crate::Result<()> {
        let data = serialize(&self.header, &self.entries);
        let path = lru_file_path(&self.data_dir, self.generation);

        tokio::fs::write(&path, &data).await.map_err(|e| {
            crate::StorageError::Cache(format!(
                "failed to write LRU checkpoint to {}: {e}",
                path.display()
            ))
        })?;

        debug!(
            "LRU checkpoint: generation {} -> {}",
            generation_to_filename(self.generation),
            self.key_map.len()
        );

        // Delete previous generation file
        if self.prev_generation != 0 {
            let prev_path = lru_file_path(&self.data_dir, self.prev_generation);
            if let Err(e) = tokio::fs::remove_file(&prev_path).await
                && e.kind() != std::io::ErrorKind::NotFound
            {
                warn!("failed to delete old LRU file {}: {e}", prev_path.display());
            }
        }

        Ok(())
    }

    /// Load state from a `.lru` file on disk.
    ///
    pub async fn load_from_disk(&mut self, generation: u64) -> crate::Result<()> {
        let path = lru_file_path(&self.data_dir, generation);
        let data = tokio::fs::read(&path).await.map_err(|e| {
            crate::StorageError::Cache(format!("failed to read LRU file {}: {e}", path.display()))
        })?;

        let (header, entries) = deserialize(&data).ok_or_else(|| {
            crate::StorageError::Cache(format!("invalid LRU file: {}", path.display()))
        })?;

        // Rebuild the key map and free list
        self.header = header;
        self.key_map.clear();
        self.free_list.clear();

        // Resize entries to match loaded data (may differ from capacity)
        self.entries = entries;

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.is_active() {
                self.key_map.insert(entry.ekey, i as u32);
            } else {
                self.free_list.push(i as u32);
            }
        }

        self.generation = generation;
        debug!(
            "LRU loaded: {} entries from {}",
            self.key_map.len(),
            path.display()
        );

        Ok(())
    }

    /// Find the latest `.lru` file in the data directory.
    ///
    pub fn find_latest_lru_file(dir: &Path) -> Option<(u64, PathBuf)> {
        let read_dir = std::fs::read_dir(dir).ok()?;
        let mut best: Option<(u64, PathBuf)> = None;

        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_str()?;
            if let Some(file_gen) = lru_file::filename_to_generation(name_str)
                && best
                    .as_ref()
                    .is_none_or(|(best_gen, _)| file_gen > *best_gen)
            {
                best = Some((file_gen, entry.path()));
            }
        }

        best
    }

    /// Reset the table to initial state.
    ///
    pub fn reset(&mut self) {
        self.header = LruFileHeader::default();
        self.entries.clear();
        self.entries
            .resize(self.capacity as usize, LruFileEntry::empty());
        self.key_map.clear();
        self.free_list.clear();
        self.free_list.extend(0..self.capacity);
    }

    // === Internal linked-list operations ===

    /// Unlink an entry from its current position in the doubly-linked list.
    fn unlink(&mut self, idx: u32) {
        let entry = self.entries[idx as usize];
        let prev = entry.prev;
        let next = entry.next;

        if prev == LRU_SENTINEL {
            // Was the LRU tail
            self.header.lru_tail = next;
        } else {
            self.entries[prev as usize].next = next;
        }

        if next == LRU_SENTINEL {
            // Was the MRU head
            self.header.mru_head = prev;
        } else {
            self.entries[next as usize].prev = prev;
        }

        self.entries[idx as usize].prev = LRU_SENTINEL;
        self.entries[idx as usize].next = LRU_SENTINEL;
    }

    /// Link an entry at the MRU head (most recently used).
    fn link_at_head(&mut self, idx: u32) {
        let old_head = self.header.mru_head;

        self.entries[idx as usize].next = LRU_SENTINEL;
        self.entries[idx as usize].prev = old_head;

        if old_head == LRU_SENTINEL {
            // List was empty, this is also the tail
            self.header.lru_tail = idx;
        } else {
            self.entries[old_head as usize].next = idx;
        }

        self.header.mru_head = idx;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_manager() {
        let dir = tempdir().expect("tempdir");
        let lru = LruManager::new(100, dir.path().to_path_buf());
        assert!(lru.is_empty());
        assert_eq!(lru.capacity(), 100);
        assert_eq!(lru.generation(), 1);
    }

    #[test]
    fn test_touch_and_contains() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        let key = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        assert!(!lru.contains(&key));

        lru.touch(&key);
        assert!(lru.contains(&key));
        assert_eq!(lru.len(), 1);
    }

    #[test]
    fn test_touch_existing_moves_to_head() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        let k1 = [1; 9];
        let k2 = [2; 9];
        let k3 = [3; 9];

        lru.touch(&k1);
        lru.touch(&k2);
        lru.touch(&k3);
        assert_eq!(lru.len(), 3);

        // Touch k1 again, should move to head
        lru.touch(&k1);
        assert_eq!(lru.len(), 3); // No duplicate

        // k2 should now be the LRU tail
        let evicted = lru.evict_tail();
        assert!(evicted.is_some());
        assert!(!lru.contains(&k2)); // k2 was evicted
        assert!(lru.contains(&k1)); // k1 still present
        assert!(lru.contains(&k3)); // k3 still present
    }

    #[test]
    fn test_evict_at_capacity() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(3, dir.path().to_path_buf());

        let k1 = [1; 9];
        let k2 = [2; 9];
        let k3 = [3; 9];
        let k4 = [4; 9];

        lru.touch(&k1);
        lru.touch(&k2);
        lru.touch(&k3);
        assert_eq!(lru.len(), 3);

        // Adding k4 should evict k1 (LRU tail)
        lru.touch(&k4);
        assert_eq!(lru.len(), 3);
        assert!(!lru.contains(&k1));
        assert!(lru.contains(&k4));
    }

    #[test]
    fn test_remove() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        let key = [0xAA; 9];
        lru.touch(&key);
        assert!(lru.contains(&key));

        assert!(lru.remove(&key));
        assert!(!lru.contains(&key));
        assert_eq!(lru.len(), 0);

        // Remove non-existent
        assert!(!lru.remove(&[0xBB; 9]));
    }

    #[test]
    fn test_bump_generation() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        assert_eq!(lru.generation(), 1);
        assert_eq!(lru.prev_generation(), 0);

        lru.bump_generation();
        assert_eq!(lru.generation(), 2);
        assert_eq!(lru.prev_generation(), 1);
    }

    #[test]
    fn test_generation_never_zero() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        lru.generation = u64::MAX;
        lru.bump_generation();
        assert_eq!(lru.generation(), 1); // Wraps past 0 to 1
    }

    #[test]
    fn test_reset() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        lru.touch(&[1; 9]);
        lru.touch(&[2; 9]);
        assert_eq!(lru.len(), 2);

        lru.reset();
        assert!(lru.is_empty());
        assert_eq!(lru.header.mru_head, LRU_SENTINEL);
        assert_eq!(lru.header.lru_tail, LRU_SENTINEL);
    }

    #[test]
    fn test_evict_empty() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());
        assert!(lru.evict_tail().is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_and_load() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(100, dir.path().to_path_buf());

        let k1 = [0x11; 9];
        let k2 = [0x22; 9];
        let k3 = [0x33; 9];

        lru.touch(&k1);
        lru.touch(&k2);
        lru.touch(&k3);

        // Checkpoint
        lru.checkpoint_to_disk().await.expect("checkpoint");

        // Verify file exists
        let lru_path = lru_file::lru_file_path(dir.path(), 1);
        assert!(lru_path.exists());

        // Load into a new manager
        let mut lru2 = LruManager::new(100, dir.path().to_path_buf());
        lru2.load_from_disk(1).await.expect("load");

        assert_eq!(lru2.len(), 3);
        assert!(lru2.contains(&k1));
        assert!(lru2.contains(&k2));
        assert!(lru2.contains(&k3));
    }

    #[tokio::test]
    async fn test_checkpoint_deletes_previous() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(100, dir.path().to_path_buf());

        lru.touch(&[0xAA; 9]);
        lru.checkpoint_to_disk().await.expect("first checkpoint");

        let first_path = lru_file::lru_file_path(dir.path(), 1);
        assert!(first_path.exists());

        lru.bump_generation();
        lru.checkpoint_to_disk().await.expect("second checkpoint");

        let second_path = lru_file::lru_file_path(dir.path(), 2);
        assert!(second_path.exists());
        assert!(!first_path.exists()); // Deleted by checkpoint
    }

    #[test]
    fn test_find_latest_lru_file() {
        let dir = tempdir().expect("tempdir");

        // No files
        assert!(LruManager::find_latest_lru_file(dir.path()).is_none());

        // Create some files
        std::fs::write(dir.path().join("0000000000000003.lru"), vec![0; 28]).expect("write");
        std::fs::write(dir.path().join("0000000000000007.lru"), vec![0; 28]).expect("write");
        std::fs::write(dir.path().join("0000000000000001.lru"), vec![0; 28]).expect("write");

        let (latest_gen, _path) = LruManager::find_latest_lru_file(dir.path()).expect("find");
        assert_eq!(latest_gen, 7);
    }

    #[test]
    fn test_linked_list_ordering() {
        let dir = tempdir().expect("tempdir");
        let mut lru = LruManager::new(10, dir.path().to_path_buf());

        let k1 = [1; 9]; // Oldest (LRU tail after all three added)
        let k2 = [2; 9];
        let k3 = [3; 9]; // Newest (MRU head)

        lru.touch(&k1);
        lru.touch(&k2);
        lru.touch(&k3);

        // Evict should remove k1 first (LRU tail)
        lru.evict_tail();
        assert!(!lru.contains(&k1));

        // Then k2
        lru.evict_tail();
        assert!(!lru.contains(&k2));

        // Then k3
        lru.evict_tail();
        assert!(!lru.contains(&k3));

        assert!(lru.is_empty());
    }
}

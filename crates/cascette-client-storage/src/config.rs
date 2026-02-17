//! Configuration for the storage system

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration for the storage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base path for storage
    pub base_path: PathBuf,

    /// Enable shared memory IPC
    pub enable_shared_memory: bool,

    /// Maximum memory for index cache (in bytes)
    pub max_index_cache_size: usize,

    /// Enable memory-mapped I/O for archives
    pub enable_mmap: bool,

    /// Number of concurrent read threads
    pub read_threads: usize,

    /// Enable content verification
    pub verify_content: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("./data"),
            enable_shared_memory: false,
            max_index_cache_size: 100 * 1024 * 1024, // 100 MB
            enable_mmap: true,
            read_threads: 4,
            verify_content: true,
        }
    }
}

impl StorageConfig {
    /// Create a new configuration with the specified base path
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Set the base path for storage
    #[must_use]
    pub fn with_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.base_path = path.as_ref().to_path_buf();
        self
    }

    /// Enable or disable shared memory IPC
    #[must_use]
    pub const fn with_shared_memory(mut self, enable: bool) -> Self {
        self.enable_shared_memory = enable;
        self
    }

    /// Set the maximum index cache size
    #[must_use]
    pub const fn with_index_cache_size(mut self, size: usize) -> Self {
        self.max_index_cache_size = size;
        self
    }
}

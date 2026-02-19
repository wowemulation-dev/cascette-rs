//! Local CASC storage implementation for World of Warcraft game installations.
//!
//! This crate provides the local file storage layer that game clients expect,
//! managing .idx index files and .data archive files on disk. The architecture
//! mirrors the CASC four-container model:
//!
//! - **Dynamic Container**: Read-write CASC archives (primary storage)
//! - **Static Container**: Read-only finalized archives
//! - **Residency Container**: Download tracking with `.residency` token files
//! - **Hard Link Container**: Filesystem hard links between installations
//!
//! # Storage Layout
//!
//! Both `.idx` and `.data` files in `Data/data/`.
//! The shared memory protocol (v4/v5) coordinates access between processes.
//!
//! # Example
//!
//! ```rust,ignore
//! use cascette_client_storage::Installation;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let data_root = PathBuf::from("/path/to/wow/Data");
//! let install = Installation::open(data_root)?;
//! install.initialize().await?;
//!
//! let stats = install.stats().await;
//! println!("Index entries: {}", stats.index_entries);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![allow(clippy::must_use_candidate)]

use thiserror::Error;

// Container architecture (CASC four-container model)
pub mod container;

// Storage internals
pub mod storage;

// Index management
pub mod index;

// Key Mapping Table
pub mod kmt;

// Shared memory protocol
pub mod shmem;

// LRU cache
pub mod lru;

// Content resolution pipeline
pub mod resolver;

// Installation management
pub mod installation;

// Configuration
pub mod config;

// Binary format validation
pub mod validation;

// Build info parser (.build.info BPSV file)
pub mod build_info;

// Top-level storage manager (manages installations)
mod storage_manager;

pub use build_info::BuildInfoFile;
pub use config::StorageConfig;
pub use container::AccessMode;
pub use index::IndexEntry;
pub use installation::Installation;
pub use storage_manager::Storage;

/// Result type for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Errors that can occur during storage operations.
///
/// Error codes in parentheses reference the TACT error code mapping
/// from `agent-container-storage.md`.
#[derive(Debug, Error)]
pub enum StorageError {
    /// I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Index file error.
    #[error("Index error: {0}")]
    Index(String),

    /// Archive file error.
    #[error("Archive error: {0}")]
    Archive(String),

    /// Content not found.
    #[error("Content not found: {0}")]
    NotFound(String),

    /// Invalid data format.
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Shared memory error.
    #[error("Shared memory error: {0}")]
    SharedMemory(String),

    /// Installation error.
    #[error("Installation error: {0}")]
    Installation(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Verification failed.
    #[error("Verification failed: {0}")]
    Verification(String),

    /// Resolver error.
    #[error("Resolver error: {0}")]
    Resolver(String),

    /// Cache error.
    #[error("Cache error: {0}")]
    Cache(String),

    /// Corruption detected.
    #[error("Data corruption detected: {0}")]
    Corruption(String),

    /// Concurrent operation failed.
    #[error("Concurrent operation failed: {0}")]
    ConcurrencyError(String),

    /// Resource exhausted (TACT error code 7).
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Operation timeout.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Access denied.
    #[error("Access denied: {0}")]
    AccessDenied(String),

    /// Incompatible version.
    #[error("Incompatible version: {0}")]
    IncompatibleVersion(String),

    /// Container is locked for exclusive access (TACT error code 11/0xb).
    #[error("Container locked: {0}")]
    ContainerLocked(String),

    /// Truncated read detected (TACT error code 7).
    /// Data is partially available; the key should be marked non-resident.
    #[error("Truncated read: {0}")]
    TruncatedRead(String),
}

/// Version information for the storage system.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default data directory name (installation root).
pub const DEFAULT_DATA_DIR: &str = "Data";

/// Data subdirectory where both `.idx` and `.data` files live.
///
/// CASC stores index and archive files in the same directory:
/// `<install>/Data/data/`.
pub const DATA_DIR: &str = "data";

/// `.build.info` filename at the installation root.
///
/// CASC reads this BPSV file to determine product, region,
/// build config hash, and CDN config hash for the installation.
pub const BUILD_INFO_FILE: &str = ".build.info";

/// CDN index cache subdirectory.
///
/// CASC stores downloaded CDN archive indices (`.index` files)
/// in `Data/indices/`.
pub const INDICES_DIR: &str = "indices";

/// Residency container subdirectory.
///
/// CASC stores the residency tracking database in `Data/residency/`.
/// Created by `tact::ResidencyContainerInit`.
pub const RESIDENCY_DIR: &str = "residency";

/// E-header cache subdirectory (preservation set).
///
/// CASC stores the e-header cache in `Data/ecache/`.
/// Created by `casc::PreservationSet::CreateEHeaderCache`.
pub const ECACHE_DIR: &str = "ecache";

/// Hard link container subdirectory.
///
/// CASC stores the hard link trie directory in `Data/hardlink/`.
/// Created by `tact::HardLinkContainerInit`.
pub const HARDLINK_DIR: &str = "hardlink";

// =============================================================================
// TACT Error Code Translation
// =============================================================================

/// CASC error code as used by the CASC storage layer.
pub type CascErrorCode = u32;

/// TACT error code as returned to callers.
pub type TactErrorCode = u32;

/// Translate a CASC error code to a TACT error code.
///
/// Mapping from `agent-container-storage.md`:
///
/// | CASC | TACT | Notes |
/// |------|------|-------|
/// | 0    | 0    | Success |
/// | 2    | 6    | |
/// | 3    | 7    | Truncated read / needs update |
/// | 4    | 3    | Invalid parameter |
/// | 5    | 5    | |
/// | 7    | 10   | |
/// | 9    | 11   | Container locked / exclusive access |
/// | 10   | 3    | Invalid parameter |
/// | 11   | 16   | |
/// | 12   | 9    | |
/// | 13   | 15   | |
/// | 15   | 2    | |
/// | 21   | 12   | |
/// | 23   | 19   | |
/// | 26   | 24   | |
/// | default | 1 | Unknown error |
pub const fn translate_error_code(casc_code: CascErrorCode) -> TactErrorCode {
    match casc_code {
        0 => 0,
        2 => 6,
        3 => 7,
        4 | 10 => 3,
        5 => 5,
        7 => 10,
        9 => 11,
        11 => 16,
        12 => 9,
        13 => 15,
        15 => 2,
        21 => 12,
        23 => 19,
        26 => 24,
        _ => 1,
    }
}

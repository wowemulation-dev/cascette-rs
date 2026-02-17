//! Local CASC storage implementation for World of Warcraft game installations.
//!
//! This crate provides the local file storage layer that game clients expect,
//! managing .idx index files and .data archive files on disk.
//!
//! # Overview
//!
//! The client storage system handles:
//! - Index files (.idx) that map content keys to archive locations
//! - Data files (.data) containing BLTE-encoded game content
//! - Shared memory for inter-process communication with game clients
//! - Content resolution from file paths to actual data
//! - Storage optimization including deduplication and space management
//!
//! # Example
//!
//! ```rust,ignore
//! use cascette_client_storage::{Storage, StorageConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize storage system
//! let config = StorageConfig::default()
//!     .with_path("/path/to/wow/data");
//!
//! let storage = Storage::new(config)?;
//! let installation = storage.open_installation("wow_retail").await?;
//!
//! // Read a file by content key
//! let content_key = [0xDE, 0xAD, 0xBE, 0xEF, /* ... */];
//! let data = installation.read_file(&content_key).await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![allow(clippy::must_use_candidate)]

use thiserror::Error;

pub mod archive;
pub mod config;
pub mod index;
pub mod installation;
pub mod resolver;
pub mod shmem;
pub mod storage;
pub mod validation;

pub use config::StorageConfig;
pub use index::IndexEntry;
pub use installation::Installation;
pub use storage::Storage;

/// Result type for storage operations
pub type Result<T> = std::result::Result<T, StorageError>;

/// Errors that can occur during storage operations
#[derive(Debug, Error)]
pub enum StorageError {
    /// I/O error occurred
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Index file error
    #[error("Index error: {0}")]
    Index(String),

    /// Archive file error
    #[error("Archive error: {0}")]
    Archive(String),

    /// Content not found
    #[error("Content not found: {0}")]
    NotFound(String),

    /// Invalid data format
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Shared memory error
    #[error("Shared memory error: {0}")]
    SharedMemory(String),

    /// Installation error
    #[error("Installation error: {0}")]
    Installation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Verification failed
    #[error("Verification failed: {0}")]
    Verification(String),

    /// Resolver error
    #[error("Resolver error: {0}")]
    Resolver(String),

    /// Cache error
    #[error("Cache error: {0}")]
    Cache(String),

    /// Corruption detected
    #[error("Data corruption detected: {0}")]
    Corruption(String),

    /// Concurrent operation failed
    #[error("Concurrent operation failed: {0}")]
    ConcurrencyError(String),

    /// Resource exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Operation timeout
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Access denied
    #[error("Access denied: {0}")]
    AccessDenied(String),

    /// Incompatible version
    #[error("Incompatible version: {0}")]
    IncompatibleVersion(String),
}

/// Version information for the storage system
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default data directory name
pub const DEFAULT_DATA_DIR: &str = "Data";

/// Default indices subdirectory
pub const INDICES_DIR: &str = "indices";

/// Default data archives subdirectory
pub const DATA_DIR: &str = "data";

/// Default configuration subdirectory
pub const CONFIG_DIR: &str = "config";

/// Default shared memory subdirectory
pub const SHMEM_DIR: &str = "shmem";

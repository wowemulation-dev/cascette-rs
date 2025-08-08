//! CASC (Content Addressable Storage Container) implementation for local game file storage
//!
//! This crate provides a complete implementation of Blizzard's CASC storage system,
//! supporting efficient content-addressed storage with deduplication, compression,
//! and fast access to game files.

pub mod archive;
pub mod cache;
pub mod config;
pub mod error;
pub mod index;
pub mod manifest;
pub mod progressive;
pub mod storage;
pub mod types;
pub mod utils;

pub use error::{CascError, Result};
pub use storage::CascStorage;
pub use types::{ArchiveLocation, EKey, IndexEntry};

// Re-export commonly used types
pub use archive::{Archive, ArchiveReader};
pub use config::{ConfigDiscovery, WowConfigSet};
pub use index::{GroupIndex, IndexFile, IndexVersion};
pub use manifest::{FileMapping, ManifestConfig, TactManifests};
pub use progressive::{ProgressiveConfig, ProgressiveFile, ProgressiveFileManager, SizeHint};

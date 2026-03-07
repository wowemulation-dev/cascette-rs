//! Containerless storage backend for modern Blizzard titles.
//!
//! Instead of CASC `.data` archives with `.idx` indices, files are stored
//! as loose files on disk with an encrypted SQLite database for metadata.
//! This storage model is used by the Blizzard Agent for Overwatch 2,
//! Diablo IV, and Call of Duty.
//!
//! # Storage Model
//!
//! - **Loose files** are stored at `{root}/{ekey[0..2]}/{ekey[2..4]}/{ekey}`
//!   matching the CDN URL path convention.
//! - **SQLite database** tracks file entries (ekey, ckey, sizes, path, flags),
//!   build metadata, and tags. The database can be encrypted with Salsa20.
//! - **Residency tracking** maintains an in-memory set of which files are
//!   locally available, avoiding filesystem stat calls.
//!
//! # Usage
//!
//! ```rust,no_run
//! use cascette_containerless::{ContainerlessStorage, ContainerlessConfig};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), cascette_containerless::ContainerlessError> {
//! let config = ContainerlessConfig::new(PathBuf::from("/opt/game/Data/data"));
//! let storage = ContainerlessStorage::open(config).await?;
//! let files = storage.list_files().await?;
//! # Ok(())
//! # }
//! ```

pub mod bgdl;
pub mod block_mover;
pub mod config;
pub mod db;
pub mod eheader;
pub mod error;
pub mod loose;
pub mod product_db;
pub mod residency;
pub mod sparse;
pub mod storage;

// Re-exports for convenience.
pub use bgdl::{BgdlConfig, BgdlManager, BgdlProgress, BgdlState, FailedFile};
pub use block_mover::{BlockDescriptor, BlockMoveInstruction, BlockMover, BlockMoverState};
pub use config::ContainerlessConfig;
pub use db::FileDatabase;
pub use db::{BuildMeta, FileEntry};
pub use eheader::{EHeader, EHeaderCache};
pub use error::{ContainerlessError, ContainerlessResult};
pub use loose::LooseFileStore;
pub use product_db::Database as ProductDatabase;
pub use residency::ResidencyTracker;
pub use sparse::SparseCapability;
pub use storage::{ContainerlessStorage, StorageStats, VerifyReport};

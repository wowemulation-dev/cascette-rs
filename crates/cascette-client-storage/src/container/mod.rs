//! Container types matching the CASC four-container architecture.
//!
//! CASC organizes CASC storage into four container types:
//! - `DynamicContainer`: Read-write CASC archives (primary storage)
//! - `StaticContainer`: Read-only finalized archives
//! - `ResidencyContainer`: Download tracking with `.residency` token files
//! - `HardLinkContainer`: Filesystem hard links between installations
//!
//! Each container type implements the [`Container`] trait for a unified
//! interface. The [`AccessMode`] enum controls how containers are opened.

use crate::{Result, StorageError};

pub mod dynamic;
pub mod hardlink;
pub mod residency;
pub mod static_container;

pub use dynamic::DynamicContainer;
pub use hardlink::HardLinkContainer;
pub use residency::ResidencyContainer;
pub use static_container::{KeyState, StaticContainer};

/// Access mode for container operations.
///
/// Maps to CASC internal flags:
/// - `None` (0) -> flags 0
/// - `ReadOnly` (1) -> flags 2
/// - `ReadWrite` (2) -> flags 5
/// - `Exclusive` (3) -> flags 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessMode {
    /// No access.
    None = 0,
    /// Read-only access. Container data can be read but not modified.
    ReadOnly = 1,
    /// Read-write access. Container data can be read and modified.
    ReadWrite = 2,
    /// Exclusive access. Only one process can hold the container.
    Exclusive = 3,
}

impl AccessMode {
    /// Convert to CASC internal flags.
    pub const fn to_internal_flags(self) -> u8 {
        match self {
            Self::None => 0,
            Self::ReadOnly => 2,
            Self::ReadWrite => 5,
            Self::Exclusive => 1,
        }
    }

    /// Check if this mode allows reads.
    pub const fn can_read(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite | Self::Exclusive)
    }

    /// Check if this mode allows writes.
    pub const fn can_write(self) -> bool {
        matches!(self, Self::ReadWrite | Self::Exclusive)
    }
}

/// Common container interface matching CASC's four container types.
///
/// All container types implement this trait. Operations that don't apply
/// to a particular container type (e.g., `write` on a `StaticContainer`)
/// return `StorageError::AccessDenied`.
pub trait Container: Send + Sync {
    /// Reserve space for a key.
    ///
    /// Prepares the container to accept data for the given key.
    /// For `DynamicContainer`, this allocates space in the current segment.
    fn reserve(&self, key: &[u8; 16]) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Read file data by key.
    ///
    /// Returns the number of bytes read into `buf`. If the read is shorter
    /// than expected, the key is marked non-resident and
    /// `StorageError::TruncatedRead` is returned (matching Agent behavior).
    fn read(
        &self,
        key: &[u8; 16],
        offset: u64,
        len: u32,
        buf: &mut [u8],
    ) -> impl std::future::Future<Output = Result<usize>> + Send;

    /// Write file data by key.
    ///
    /// Writes data to the container. For `DynamicContainer`, this prepends
    /// the 30-byte local header and updates the KMT.
    fn write(
        &self,
        key: &[u8; 16],
        data: &[u8],
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Remove a key from the container.
    fn remove(&self, key: &[u8; 16]) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Check if a key exists in the container.
    fn query(&self, key: &[u8; 16]) -> impl std::future::Future<Output = Result<bool>> + Send;
}

/// Translate a CASC error code to a `StorageError`.
///
/// This maps the numeric error codes from CASC's container operations
/// to typed errors.
pub fn storage_error_from_casc_code(casc_code: u32, context: &str) -> StorageError {
    match casc_code {
        0 => unreachable!("CASC code 0 is success"),
        3 => StorageError::TruncatedRead(context.to_string()),
        4 | 10 => StorageError::InvalidFormat(context.to_string()),
        9 => StorageError::ContainerLocked(context.to_string()),
        _ => StorageError::Archive(format!("CASC error {casc_code}: {context}")),
    }
}

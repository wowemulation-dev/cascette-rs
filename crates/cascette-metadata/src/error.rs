//! Error types for metadata operations.

use thiserror::Error;

/// Result type alias for metadata operations.
pub type MetadataResult<T> = Result<T, MetadataError>;

/// Errors that can occur during metadata operations.
#[derive(Debug, Error)]
pub enum MetadataError {
    /// FileDataID not found in the mapping.
    #[error("FileDataID {0} not found")]
    FileDataIdNotFound(u32),

    /// File path not found in the reverse mapping.
    #[error("path not found: {0}")]
    PathNotFound(String),

    /// TACT encryption key not found.
    #[error("TACT key not found: {0:016X}")]
    TactKeyNotFound(u64),

    /// Import provider error.
    #[cfg(feature = "import")]
    #[error("import error: {0}")]
    Import(#[from] cascette_import::ImportError),

    /// Service not initialized.
    #[error("service not initialized")]
    NotInitialized,

    /// Key store error.
    #[error("key store error: {0}")]
    KeyStore(#[from] cascette_crypto::CryptoError),
}

//! Error types for the ngdp-cache crate

use thiserror::Error;

/// Result type for ngdp-cache operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for cache operations
#[derive(Debug, Error)]
pub enum Error {
    /// Cache directory could not be determined
    #[error("Could not determine cache directory for the current platform")]
    CacheDirectoryNotFound,

    /// IO error occurred
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The cache entry was not found
    #[error("Cache entry not found: {0}")]
    CacheEntryNotFound(String),

    /// Invalid cache key provided
    #[error("Invalid cache key: {0}")]
    InvalidCacheKey(String),

    /// Cache corruption detected
    #[error("Cache corruption detected: {0}")]
    CacheCorruption(String),

    /// Ribbit client error
    #[error("Ribbit client error: {0}")]
    RibbitClient(#[from] ribbit_client::Error),
}

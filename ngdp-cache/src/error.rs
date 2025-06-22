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

    /// TACT client error
    #[error("TACT client error: {0}")]
    TactClient(#[from] tact_client::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request error
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    /// CDN client error
    #[error("CDN client error: {0}")]
    CdnClient(#[from] ngdp_cdn::Error),

    /// UTF-8 conversion error
    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

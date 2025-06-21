//! Error types for NGDP CDN operations

use thiserror::Error;

/// Error types for CDN operations
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Network timeout
    #[error("Request timeout after {duration_ms}ms")]
    Timeout {
        /// Timeout duration in milliseconds
        duration_ms: u64,
    },

    /// All CDN hosts exhausted
    #[error("All CDN hosts exhausted for {resource}")]
    CdnExhausted {
        /// Resource being requested
        resource: String,
    },

    /// Invalid CDN host format
    #[error("Invalid CDN host: {host}")]
    InvalidHost {
        /// The invalid host string
        host: String,
    },

    /// Invalid content hash format
    #[error("Invalid content hash: {hash}")]
    InvalidHash {
        /// The invalid hash string
        hash: String,
    },

    /// Content not found on CDN
    #[error("Content not found: {hash}")]
    ContentNotFound {
        /// The content hash that was not found
        hash: String,
    },

    /// Content verification failed
    #[error("Content verification failed for {hash}: expected {expected}, got {actual}")]
    VerificationFailed {
        /// The content hash being verified
        hash: String,
        /// Expected checksum/hash value
        expected: String,
        /// Actual checksum/hash value
        actual: String,
    },

    /// Invalid response from CDN
    #[error("Invalid response from CDN: {reason}")]
    InvalidResponse {
        /// Reason for the invalid response
        reason: String,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: retry after {retry_after_secs} seconds")]
    RateLimited {
        /// Seconds to wait before retrying
        retry_after_secs: u64,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid URL
    #[error("Invalid URL: {url}")]
    InvalidUrl {
        /// The invalid URL
        url: String,
    },

    /// Content size mismatch
    #[error("Content size mismatch: expected {expected} bytes, got {actual} bytes")]
    SizeMismatch {
        /// Expected size in bytes
        expected: u64,
        /// Actual size in bytes
        actual: u64,
    },

    /// Partial content not supported
    #[error("CDN server does not support partial content (range requests)")]
    PartialContentNotSupported,
}

/// Result type for CDN operations
pub type Result<T> = std::result::Result<T, Error>;

// Helper methods for common error construction
impl Error {
    /// Create a CDN exhausted error
    pub fn cdn_exhausted(resource: impl Into<String>) -> Self {
        Self::CdnExhausted {
            resource: resource.into(),
        }
    }

    /// Create an invalid host error
    pub fn invalid_host(host: impl Into<String>) -> Self {
        Self::InvalidHost { host: host.into() }
    }

    /// Create an invalid hash error
    pub fn invalid_hash(hash: impl Into<String>) -> Self {
        Self::InvalidHash { hash: hash.into() }
    }

    /// Create a content not found error
    pub fn content_not_found(hash: impl Into<String>) -> Self {
        Self::ContentNotFound { hash: hash.into() }
    }

    /// Create a verification failed error
    pub fn verification_failed(
        hash: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::VerificationFailed {
            hash: hash.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create an invalid response error
    pub fn invalid_response(reason: impl Into<String>) -> Self {
        Self::InvalidResponse {
            reason: reason.into(),
        }
    }

    /// Create a rate limited error
    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self::RateLimited { retry_after_secs }
    }

    /// Create an invalid URL error
    pub fn invalid_url(url: impl Into<String>) -> Self {
        Self::InvalidUrl { url: url.into() }
    }

    /// Create a size mismatch error
    pub fn size_mismatch(expected: u64, actual: u64) -> Self {
        Self::SizeMismatch { expected, actual }
    }
}

//! Error types for TACT client

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    // Network errors
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("All CDN hosts exhausted for {resource}")]
    CdnExhausted { resource: String },

    #[error("Connection timeout to {host}")]
    ConnectionTimeout { host: String },

    // Data format errors
    #[error("BPSV parse error: {0}")]
    Bpsv(#[from] ngdp_bpsv::Error),

    #[error("Invalid manifest format at line {line}: {reason}")]
    InvalidManifest { line: usize, reason: String },

    #[error("Missing required field: {field}")]
    MissingField { field: &'static str },

    #[error("Invalid hash format: {hash}")]
    InvalidHash { hash: String },

    #[error("Checksum verification failed: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Invalid response format")]
    InvalidResponse,

    // Configuration errors
    #[error("Invalid region: {0}")]
    InvalidRegion(String),

    #[error("Product not supported: {0}")]
    UnsupportedProduct(String),

    #[error("Invalid protocol version")]
    InvalidProtocolVersion,

    // File errors
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Helper methods for common error construction
impl Error {
    /// Create an invalid manifest error with line number and reason
    pub fn invalid_manifest(line: usize, reason: impl Into<String>) -> Self {
        Self::InvalidManifest {
            line,
            reason: reason.into(),
        }
    }

    /// Create a missing field error
    pub fn missing_field(field: &'static str) -> Self {
        Self::MissingField { field }
    }

    /// Create a CDN exhausted error
    pub fn cdn_exhausted(resource: impl Into<String>) -> Self {
        Self::CdnExhausted {
            resource: resource.into(),
        }
    }

    /// Create a file not found error
    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    /// Create an invalid hash error
    pub fn invalid_hash(hash: impl Into<String>) -> Self {
        Self::InvalidHash { hash: hash.into() }
    }

    /// Create a checksum mismatch error
    pub fn checksum_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::ChecksumMismatch {
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

//! Error types for patch operations

use thiserror::Error;

/// Result type for patch operations
pub type Result<T> = std::result::Result<T, PatchError>;

/// Errors that can occur during patch operations
#[derive(Error, Debug)]
pub enum PatchError {
    /// I/O error occurred
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid patch format
    #[error("Invalid patch format: {0}")]
    InvalidFormat(String),

    /// Corrupt patch data
    #[error("Corrupt patch data: {0}")]
    CorruptPatch(String),

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// Patch not found
    #[error("Patch not found for content key: {0}")]
    PatchNotFound(String),

    /// Invalid signature
    #[error("Invalid signature: expected {expected:016X}, got {actual:016X}")]
    InvalidSignature { expected: u64, actual: u64 },

    /// Size mismatch
    #[error("Size mismatch: expected {expected} bytes, got {actual} bytes")]
    SizeMismatch { expected: usize, actual: usize },

    /// Decompression error
    #[error("Decompression error: {0}")]
    DecompressionError(String),

    /// Missing required data
    #[error("Missing required data: {0}")]
    MissingData(String),

    /// Unsupported patch version
    #[error("Unsupported patch version: {0}")]
    UnsupportedVersion(u8),
}

//! Error types for ngdp-crypto operations.

use thiserror::Error;

/// Errors that can occur during crypto operations.
#[derive(Error, Debug)]
pub enum CryptoError {
    /// Key not found in key service.
    #[error("encryption key not found: {0:016x}")]
    KeyNotFound(u64),

    /// Invalid key format.
    #[error("invalid key format: {0}")]
    InvalidKeyFormat(String),

    /// Invalid key size.
    #[error("invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    /// Invalid IV size.
    #[error("invalid IV size: expected {expected}, got {actual}")]
    InvalidIvSize { expected: usize, actual: usize },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid key file format.
    #[error("invalid key file format: {0}")]
    InvalidKeyFile(String),

    /// Decryption failed.
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    /// Invalid block index.
    #[error("invalid block index: {0}")]
    InvalidBlockIndex(usize),
}
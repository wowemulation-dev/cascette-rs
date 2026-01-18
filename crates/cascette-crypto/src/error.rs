//! Error types for cryptographic operations

use thiserror::Error;

/// Errors that can occur during cryptographic operations
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Invalid key size
    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize {
        /// Expected key size in bytes
        expected: usize,
        /// Actual key size in bytes
        actual: usize,
    },

    /// Invalid IV size
    #[error("Invalid IV size: expected {expected}, got {actual}")]
    InvalidIvSize {
        /// Expected IV size in bytes
        expected: usize,
        /// Actual IV size in bytes
        actual: usize,
    },

    /// Key not found
    #[error("Encryption key not found: {0:016x}")]
    KeyNotFound(u64),

    /// Invalid key format
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
}

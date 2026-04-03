//! Error types for containerless storage operations.

use std::io;

/// Result type alias for containerless operations.
pub type ContainerlessResult<T> = Result<T, ContainerlessError>;

/// Errors that can occur during containerless storage operations.
#[derive(Debug, thiserror::Error)]
pub enum ContainerlessError {
    /// SQLite database error.
    #[error("database error: {0}")]
    Database(#[from] turso::Error),

    /// Cryptographic operation error.
    #[error("crypto error: {0}")]
    Crypto(#[from] cascette_crypto::CryptoError),

    /// File I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Requested file not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Hex decoding error.
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),

    /// Schema migration error.
    #[error("schema error: {0}")]
    Schema(String),

    /// Data integrity error.
    #[error("integrity error: {0}")]
    Integrity(String),

    /// Product database error.
    #[error("product db error: {0}")]
    ProductDb(String),
}

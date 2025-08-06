//! Error types for BLTE parsing and decompression

use thiserror::Error;

/// Result type for BLTE operations
pub type Result<T> = std::result::Result<T, Error>;

/// BLTE error types
#[derive(Error, Debug)]
pub enum Error {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid BLTE magic bytes
    #[error("Invalid BLTE magic: expected [66, 76, 84, 69], got {0:?}")]
    InvalidMagic([u8; 4]),

    /// Invalid header size
    #[error("Invalid header size: {0}")]
    InvalidHeaderSize(u32),

    /// Invalid chunk count
    #[error("Invalid chunk count: {0}")]
    InvalidChunkCount(u32),

    /// Unknown compression mode
    #[error("Unknown compression mode: {0:#04x}")]
    UnknownCompressionMode(u8),

    /// Decompression failed
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    /// Encryption error from ngdp-crypto
    #[error("Encryption error: {0}")]
    Encryption(#[from] ngdp_crypto::CryptoError),

    /// Key not found for decryption
    #[error("Key not found: {0:#018x}")]
    KeyNotFound(u64),

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// Truncated data
    #[error("Truncated data: expected {expected} bytes, got {actual}")]
    TruncatedData { expected: usize, actual: usize },

    /// Invalid encrypted block structure
    #[error("Invalid encrypted block: {0}")]
    InvalidEncryptedBlock(String),

    /// Unsupported encryption type
    #[error("Unsupported encryption type: {0:#04x}")]
    UnsupportedEncryptionType(u8),
}

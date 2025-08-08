//! Error types for BLTE parsing and decompression

use thiserror::Error;

use crate::Md5;

/// Result type for BLTE operations
pub type Result<T> = std::result::Result<T, Error>;

/// BLTE error types
#[derive(Error, Debug)]
pub enum Error {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid BLTE magic bytes
    #[error("Invalid BLTE magic: expected 'BLTE', got {0:?}")]
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
    #[error(
        "Checksum mismatch: expected {}, got {}",
        hex::encode(expected),
        hex::encode(actual)
    )]
    ChecksumMismatch { expected: Vec<u8>, actual: Md5 },

    /// Truncated data
    #[error("Truncated data: expected {expected} bytes, got {actual}")]
    TruncatedData { expected: u64, actual: u64 },

    /// Invalid encrypted block structure
    #[error("Invalid encrypted block: {0}")]
    InvalidEncryptedBlock(String),

    /// Unsupported encryption type
    #[error("Unsupported encryption type: {0:#04x}")]
    UnsupportedEncryptionType(u8),

    #[error("Unsupported table format: {0:#x}")]
    UnsupportedTableFormat(u8),

    #[error("Chunk index {0} is out of range, must be less than {1}")]
    ChunkIndexOutOfRange(usize, usize),

    #[error("Unsupported LZ4HC header version: {0:#x}")]
    UnsupportedLz4hcVersion(u8),
}

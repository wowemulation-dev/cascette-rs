//! BLTE error types

use thiserror::Error;

/// BLTE-specific error type
#[derive(Debug, Error)]
pub enum BlteError {
    /// Invalid BLTE magic bytes
    #[error("invalid BLTE magic: expected [42 4C 54 45], got {0:02X?}")]
    InvalidMagic([u8; 4]),

    /// Invalid header size
    #[error("invalid header size: {0}")]
    InvalidHeaderSize(u32),

    /// Invalid header format
    #[error("invalid header: {0}")]
    InvalidHeader(String),

    /// Invalid chunk format
    #[error("invalid chunk: {0}")]
    InvalidChunk(String),

    /// Invalid chunk count
    #[error("invalid chunk count: {0}")]
    InvalidChunkCount(u32),

    /// Empty chunk encountered
    #[error("empty chunk (zero size)")]
    EmptyChunk,

    /// Invalid chunk size for builder
    #[error("invalid chunk size: {size} bytes (must be between {min} and {max} bytes)")]
    InvalidChunkSize {
        /// The invalid size that was provided
        size: usize,
        /// Minimum allowed size
        min: usize,
        /// Maximum allowed size
        max: usize,
    },

    /// Unknown compression mode
    #[error("unknown compression mode: 0x{0:02X}")]
    UnknownCompressionMode(u8),

    /// Unsupported compression mode
    #[error("unsupported compression mode: 0x{0:02X}")]
    UnsupportedCompressionMode(u8),

    /// Checksum mismatch
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum
        expected: String,
        /// Actual checksum
        actual: String,
    },

    /// Compression/decompression error
    #[error("compression error: {0}")]
    CompressionError(String),

    /// Decompression failed
    #[error("decompression failed: {0}")]
    DecompressionFailed(String),

    /// Encryption key not found
    #[error("encryption key not found: {0:016X}")]
    KeyNotFound(u64),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Binary parsing error
    #[error("binary parsing error: {0}")]
    BinRw(#[from] binrw::Error),
}

/// Result type for BLTE operations
pub type BlteResult<T> = Result<T, BlteError>;

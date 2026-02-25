//! Patch Archive error types

use thiserror::Error;

/// Patch Archive-specific error type
#[derive(Debug, Error)]
pub enum PatchArchiveError {
    /// Invalid PA magic bytes
    #[error("invalid PA magic: expected [50 41], got {0:02X?}")]
    InvalidMagic([u8; 2]),

    /// Unsupported PA version
    #[error("unsupported PA version: {0}")]
    UnsupportedVersion(u8),

    /// Invalid key sizes
    #[error("invalid key sizes: file={file}, old={old}, patch={patch}")]
    InvalidKeySize {
        /// File key size
        file: u8,
        /// Old key size
        old: u8,
        /// Patch key size
        patch: u8,
    },

    /// Invalid block size bits
    #[error("invalid block size bits: {0}")]
    InvalidBlockSize(u8),

    /// Unsupported flags
    #[error("unsupported PA flags: 0x{0:02X}")]
    UnsupportedFlags(u8),

    /// Blocks are not sorted by CKey
    #[error("blocks not sorted: block {index} CKey is not >= previous block CKey")]
    BlocksNotSorted {
        /// Index of the out-of-order block
        index: usize,
    },

    /// String too long in patch entry
    #[error("string too long in patch entry")]
    StringTooLong,

    /// Invalid string encoding
    #[error("invalid string encoding: {0}")]
    InvalidString(#[from] std::string::FromUtf8Error),

    /// Invalid compression specification
    #[error("invalid compression specification: {0}")]
    InvalidCompressionSpec(String),

    /// Unsupported compression method
    #[error("unsupported compression method: {0}")]
    UnsupportedCompression(char),

    /// Decompression error
    #[error("decompression error: {0}")]
    DecompressionError(#[from] std::io::Error),

    /// Patch verification failed
    #[error("patch verification failed: expected {expected:02X?}, got {actual:02X?}")]
    PatchVerificationFailed {
        /// Expected hash
        expected: [u8; 16],
        /// Actual hash
        actual: [u8; 16],
    },

    /// ZBSDIFF error
    #[error("ZBSDIFF error: {0}")]
    ZbsdiffError(String),

    /// Binary parsing error
    #[error("binary parsing error: {0}")]
    BinRw(#[from] binrw::Error),

    /// Invalid header format
    #[error("invalid header: {0}")]
    InvalidHeader(String),

    /// Invalid entry format
    #[error("invalid entry: {0}")]
    InvalidEntry(String),

    /// Unexpected end of block data
    #[error("unexpected end of block data at offset {0}")]
    UnexpectedEndOfBlock(u64),
}

/// Result type for Patch Archive operations
pub type PatchArchiveResult<T> = Result<T, PatchArchiveError>;

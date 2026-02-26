//! Patch Index error types

use thiserror::Error;

/// Patch Index-specific error type
#[derive(Debug, Error)]
pub enum PatchIndexError {
    /// Data too short for minimum header
    #[error("data too short: got {actual} bytes, minimum is {minimum}")]
    DataTooShort {
        /// Actual data length
        actual: usize,
        /// Minimum required length
        minimum: usize,
    },

    /// Declared header size exceeds available data
    #[error("truncated: declared header size {header_size}, but only {actual} bytes available")]
    TruncatedHeader {
        /// Declared header size
        header_size: u32,
        /// Actual data length
        actual: usize,
    },

    /// Total data size is less than header + block data
    #[error("truncated: expected {expected} bytes (header + blocks), got {actual}")]
    TruncatedData {
        /// Expected total size
        expected: usize,
        /// Actual data length
        actual: usize,
    },

    /// Unsupported version
    #[error("unsupported patch index version: {0}")]
    UnsupportedVersion(u32),

    /// Declared data_size field does not match actual file size
    #[error("data size mismatch: header says {declared}, actual is {actual}")]
    DataSizeMismatch {
        /// Size declared in header
        declared: u32,
        /// Actual data length
        actual: usize,
    },

    /// Invalid block version byte
    #[error("block type {block_type}: expected version {expected}, got {actual}")]
    InvalidBlockVersion {
        /// Block type ID
        block_type: u32,
        /// Expected version byte
        expected: u8,
        /// Actual version byte
        actual: u8,
    },

    /// I/O error during parsing
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Entry data does not fit block size
    #[error(
        "block type {block_type}: entry data overflows block (entries need {needed}, block has {available})"
    )]
    EntryOverflow {
        /// Block type ID
        block_type: u32,
        /// Bytes needed for entries
        needed: usize,
        /// Bytes available in block
        available: usize,
    },
}

/// Result type for Patch Index operations
pub type PatchIndexResult<T> = Result<T, PatchIndexError>;

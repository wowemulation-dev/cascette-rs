//! Error types for encoding file operations

use thiserror::Error;

/// Errors that can occur when working with encoding files
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum EncodingError {
    #[error("Invalid magic: expected 'EN', got {0:?}")]
    InvalidMagic([u8; 2]),

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),

    #[error("Page checksum mismatch")]
    ChecksumMismatch,

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid ESpec: {0}")]
    InvalidESpec(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Binary parsing error: {0}")]
    BinRw(#[from] binrw::Error),

    #[error("BLTE decompression error: {0}")]
    Blte(#[from] crate::blte::BlteError),

    #[error("Invalid page size: {0}")]
    InvalidPageSize(usize),

    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    #[error("ESpec table size doesn't match header")]
    InvalidESpecSize,

    #[error("Invalid flags: unk_11 must be 0, got {0}")]
    InvalidFlags(u8),

    #[error("Invalid {field} hash size: expected 1..=16, got {value}")]
    InvalidHashSize {
        /// Which hash size field is invalid
        field: &'static str,
        /// The invalid value
        value: u8,
    },

    #[error("Invalid {field} page count: must be > 0, got {value}")]
    InvalidPageCount {
        /// Which page count field is invalid
        field: &'static str,
        /// The invalid value
        value: u32,
    },

    #[error("Invalid ESpec block size: must be > 0, got {0}")]
    InvalidESpecBlockSize(u32),

    #[error("Empty ESpec string found")]
    EmptyESpec,

    #[error("ESpec block not null-terminated")]
    UnterminatedESpec,
}

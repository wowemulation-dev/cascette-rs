//! TVFS error types

use thiserror::Error;

/// TVFS-specific error type
#[derive(Debug, Error)]
pub enum TvfsError {
    /// Invalid TVFS magic bytes
    #[error("invalid TVFS magic: expected 'TVFS', got {0:?}")]
    InvalidMagic([u8; 4]),

    /// Unsupported TVFS version
    #[error("unsupported TVFS version: {0}")]
    UnsupportedVersion(u8),

    /// Invalid header size
    #[error("invalid header size: {0}, expected 46")]
    InvalidHeaderSize(u8),

    /// Invalid key sizes
    #[error("invalid key sizes: EKey={ekey}, PKey={pkey}, expected 9 each")]
    InvalidKeySize {
        /// EKey size found
        ekey: u8,
        /// PKey size found
        pkey: u8,
    },

    /// Unsupported flags
    #[error("unsupported flags: 0x{0:08X}")]
    UnsupportedFlags(u32),

    /// Path not found
    #[error("path not found: {0}")]
    PathNotFound(String),

    /// Empty path table
    #[error("empty path table")]
    EmptyPathTable,

    /// Invalid VFS entry index
    #[error("invalid VFS entry index: {0}")]
    InvalidVfsEntry(u32),

    /// Invalid container entry index
    #[error("invalid container entry index: {0}")]
    InvalidContainerEntry(u32),

    /// Path table truncated
    #[error("path table truncated at offset {0}")]
    PathTableTruncated(usize),

    /// Invalid path node format
    #[error("invalid path node format at offset {0}: {1}")]
    InvalidPathNode(usize, String),

    /// Variable integer parsing error
    #[error("variable integer parsing error at offset {0}")]
    VarIntError(usize),

    /// Table size mismatch
    #[error("table size mismatch: expected {expected}, got {actual}")]
    TableSizeMismatch {
        /// Expected table size
        expected: u32,
        /// Actual table size
        actual: u32,
    },

    /// Invalid table offset
    #[error("invalid table offset: {0} is beyond data bounds")]
    InvalidTableOffset(u32),

    /// BLTE error
    #[error("BLTE error: {0}")]
    BlteError(#[from] crate::blte::BlteError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Binary parsing error
    #[error("binary parsing error: {0}")]
    BinRw(#[from] binrw::Error),

    /// UTF-8 conversion error
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

/// Result type for TVFS operations
pub type TvfsResult<T> = Result<T, TvfsError>;

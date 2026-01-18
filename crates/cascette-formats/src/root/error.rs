//! Error types for root file parsing and building

use thiserror::Error;

/// Errors that can occur when parsing or building root files
#[derive(Error, Debug)]
pub enum RootError {
    /// Invalid magic signature detected
    #[error("Invalid root file magic: {0:?}")]
    InvalidMagic([u8; 4]),

    /// Unsupported root file version
    #[error("Unsupported root version: {0}")]
    UnsupportedVersion(u32),

    /// Truncated root block at specified offset
    #[error("Truncated root block at offset {0}")]
    TruncatedBlock(u64),

    /// Invalid `FileDataID` delta sequence
    #[error("Invalid FileDataID delta sequence")]
    InvalidDelta,

    /// Corrupted block header
    #[error("Corrupted block header: {0}")]
    CorruptedBlockHeader(String),

    /// Invalid content or locale flags
    #[error("Invalid flags: content={content:08x}, locale={locale:08x}")]
    InvalidFlags {
        /// Invalid content flags value
        content: u64,
        /// Invalid locale flags value
        locale: u32,
    },

    /// Name hash calculation error
    #[error("Name hash calculation failed for path: {path}")]
    NameHashError {
        /// File path that failed name hash calculation
        path: String,
    },

    /// Lookup failure for specified file
    #[error("Failed to resolve file: {description}")]
    LookupError {
        /// Description of lookup failure
        description: String,
    },

    /// I/O operation failed
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// `BinRW` parsing/writing error
    #[error("Binary format error: {0}")]
    BinRw(#[from] binrw::Error),
}

/// Type alias for root file operation results
pub type Result<T> = std::result::Result<T, RootError>;

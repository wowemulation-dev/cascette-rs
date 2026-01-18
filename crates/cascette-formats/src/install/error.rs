//! Error types for install manifest parsing and building

use thiserror::Error;

/// Errors that can occur when parsing or building install manifests
#[derive(Error, Debug)]
pub enum InstallError {
    /// Invalid magic signature detected
    #[error("Invalid install magic: expected 'IN', got {0:?}")]
    InvalidMagic([u8; 2]),

    /// Unsupported install manifest version
    #[error("Unsupported install version: {0}")]
    UnsupportedVersion(u8),

    /// Invalid content key length
    #[error("Invalid content key length: {0}")]
    InvalidCKeyLength(u8),

    /// Tag not found when associating with file
    #[error("Tag not found: {0}")]
    TagNotFound(String),

    /// File index out of bounds when accessing bit mask
    #[error("File index out of bounds: {0}")]
    FileIndexOutOfBounds(usize),

    /// Bit mask size mismatch
    #[error("Bit mask size mismatch: expected {expected}, got {actual}")]
    BitMaskSizeMismatch {
        /// Expected bit mask size in bytes
        expected: usize,
        /// Actual bit mask size in bytes
        actual: usize,
    },

    /// Invalid tag type value
    #[error("Invalid tag type: {0:04x}")]
    InvalidTagType(u16),

    /// Invalid UTF-8 string in file path or tag name
    #[error("Invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    /// Hex decoding error for content keys
    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    /// I/O operation failed
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// `BinRW` parsing/writing error
    #[error("Binary format error: {0}")]
    BinRw(#[from] binrw::Error),
}

/// Type alias for install manifest operation results
pub type Result<T> = std::result::Result<T, InstallError>;

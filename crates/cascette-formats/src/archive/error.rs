//! Error types for archive operations

use thiserror::Error;

/// Archive operation result type
pub type ArchiveResult<T> = Result<T, ArchiveError>;

/// Comprehensive error types for archive operations
#[derive(Debug, Error)]
pub enum ArchiveError {
    /// Archive file not found
    #[error("Archive not found: {0}")]
    ArchiveNotFound(String),

    /// Invalid archive index format
    #[error("Invalid archive index: {reason}")]
    InvalidIndex {
        /// Detailed description of the invalid format
        reason: String,
    },

    /// Content not found in any archive
    #[error("Content not found: encoding key {0}")]
    ContentNotFound(String),

    /// HTTP error during CDN operations
    #[error("HTTP error: status {0}")]
    HttpError(u16),

    /// Range request didn't return expected size
    #[error("Range request failed: requested {requested} bytes, received {received} bytes")]
    IncompleteRangeResponse {
        /// Number of bytes requested
        requested: u64,
        /// Number of bytes actually received
        received: u64,
    },

    /// Invalid hash length (should be 32 hex characters)
    #[error("Invalid hash length: expected 32, got {0}")]
    InvalidHashLength(usize),

    /// Archive index checksum mismatch
    #[error("Archive index checksum mismatch: expected {expected:02x?}, got {actual:02x?}")]
    ChecksumMismatch {
        /// Expected checksum value
        expected: [u8; 8],
        /// Actual checksum value found
        actual: [u8; 8],
    },

    /// Footer validation failed
    #[error("Footer checksum mismatch")]
    FooterChecksum,

    /// Table of contents checksum mismatch
    #[error("TOC checksum mismatch")]
    TocChecksum,

    /// Unsupported index version
    #[error("Unsupported index version: {0}")]
    UnsupportedVersion(u8),

    /// Invalid key size in footer
    #[error("Invalid key size: expected 9, got {0}")]
    InvalidKeySize(u8),

    /// Invalid segment size
    #[error("Invalid segment size: {0}")]
    InvalidSegmentSize(u8),

    /// Entries not properly sorted
    #[error("Entries not sorted by encoding key")]
    UnsortedEntries,

    /// Table of contents inconsistent with entries
    #[error("TOC inconsistent with actual entries")]
    TocInconsistent,

    /// Chunk boundary violation
    #[error("Chunk boundary violation")]
    ChunkBoundary,

    /// BLTE decompression failed
    #[error("BLTE decompression failed: {0}")]
    BlteError(#[from] crate::blte::BlteError),

    /// Binary read/write error
    #[error("Binary format error: {0}")]
    BinRead(#[from] binrw::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Network error during CDN operations
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Invalid format parameters
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

impl ArchiveError {
    /// Check if this error is retryable for network operations
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::HttpError(500..=599) | // Server errors
            Self::NetworkError(_) |      // Network issues
            Self::IncompleteRangeResponse { .. }
        )
    }

    /// Check if this error is permanent (no point in retrying)
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::ArchiveNotFound(_)
                | Self::ContentNotFound(_)
                | Self::InvalidHashLength(_)
                | Self::InvalidIndex { .. }
                | Self::UnsupportedVersion(_)
                | Self::InvalidKeySize(_)
                | Self::InvalidSegmentSize(_)
        )
    }

    /// Check if this is a validation error
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            Self::ChecksumMismatch { .. }
                | Self::FooterChecksum
                | Self::TocChecksum
                | Self::UnsortedEntries
                | Self::TocInconsistent
                | Self::ChunkBoundary
        )
    }
}

//! Error types for the Size manifest format

use thiserror::Error;

/// Errors that can occur when parsing or building size manifests
#[derive(Debug, Error)]
pub enum SizeError {
    /// Invalid magic bytes (expected "DS")
    #[error("Invalid magic: expected 'DS', got {0:?}")]
    InvalidMagic([u8; 2]),

    /// Unsupported format version
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),

    /// Data is too short for the expected format
    #[error("Truncated data: expected {expected} bytes, got {actual} bytes")]
    TruncatedData {
        /// Expected minimum size
        expected: usize,
        /// Actual data size
        actual: usize,
    },

    /// Invalid ekey_size value (must be 1–16)
    #[error("Invalid ekey_size {0}: must be 1–16")]
    InvalidEKeySize(u8),

    /// Entry count mismatch between header and parsed entries
    #[error("Entry count mismatch: header says {expected}, found {actual}")]
    EntryCountMismatch {
        /// Count from the header
        expected: u32,
        /// Actual parsed count
        actual: usize,
    },

    /// Tag count mismatch between header and parsed tags
    #[error("Tag count mismatch: header says {expected}, found {actual}")]
    TagCountMismatch {
        /// Count from the header
        expected: u16,
        /// Actual parsed count
        actual: usize,
    },

    /// Total size mismatch between header and sum of entry esizes
    #[error("Total size mismatch: header says {expected}, sum of esizes is {actual}")]
    TotalSizeMismatch {
        /// Total size from the header
        expected: u64,
        /// Computed sum of entry esizes
        actual: u64,
    },

    /// IO error during parsing or building
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Binary read/write error (from binrw shim)
    #[error("Binary parsing error: {0}")]
    BinRead(String),
}

impl From<binrw::Error> for SizeError {
    fn from(e: binrw::Error) -> Self {
        Self::BinRead(e.to_string())
    }
}

/// Result type alias for Size manifest operations
pub type Result<T> = std::result::Result<T, SizeError>;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SizeError::InvalidMagic([0x41, 0x42]);
        assert!(err.to_string().contains("'DS'"));

        let err = SizeError::UnsupportedVersion(3);
        assert!(err.to_string().contains('3'));

        let err = SizeError::TruncatedData {
            expected: 15,
            actual: 5,
        };
        assert!(err.to_string().contains("15"));

        let err = SizeError::InvalidEKeySize(0);
        assert!(err.to_string().contains('0'));

        let err = SizeError::TagCountMismatch {
            expected: 3,
            actual: 1,
        };
        assert!(err.to_string().contains('3'));
        assert!(err.to_string().contains('1'));
    }
}

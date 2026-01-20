//! Error types for ZBSDIFF1 format operations
//!
//! This module provides comprehensive error handling for all ZBSDIFF1 operations
//! including parsing, validation, compression, and patch application.

use thiserror::Error;

/// ZBSDIFF1-specific error types
#[derive(Error, Debug)]
pub enum ZbsdiffError {
    /// Invalid ZBSDIFF1 signature in header
    #[error("Invalid ZBSDIFF1 signature: expected {expected:#x}, got {actual:#x}")]
    InvalidSignature {
        /// Expected signature value
        expected: u64,
        /// Actual signature value found
        actual: u64,
    },

    /// Invalid size value in header field
    #[error("Invalid size in {field}: {value}")]
    InvalidSize {
        /// Header field name
        field: &'static str,
        /// Invalid size value
        value: i64,
    },

    /// Size value exceeds reasonable limits
    #[error("Size too large: {0} bytes (maximum 1GB)")]
    SizeTooLarge(i64),

    /// Size mismatch between expected and actual values
    #[error("Size mismatch: expected {expected} bytes, got {actual} bytes")]
    SizeMismatch {
        /// Expected size
        expected: usize,
        /// Actual size
        actual: usize,
    },

    /// Error during zlib compression
    #[error("Compression error: {0}")]
    CompressionError(std::io::Error),

    /// Error during zlib decompression
    #[error("Decompression error: {0}")]
    DecompressionError(std::io::Error),

    /// Binary format parsing error from binrw
    #[error("Binary format error: {0}")]
    BinaryFormatError(binrw::Error),

    /// Patch data is corrupted or malformed
    #[error("Corrupt patch data: {reason}")]
    CorruptPatch {
        /// Description of the corruption
        reason: String,
    },

    /// Data is not in ZBSDIFF1 format
    #[error("Not a ZBSDIFF1 format")]
    NotZbsdiffFormat,

    /// Seek operation failed or invalid
    #[error("Seek error: {0}")]
    SeekError(std::io::Error),

    /// Control block contains invalid entry
    #[error("Invalid control entry at index {index}: {reason}")]
    InvalidControlEntry {
        /// Index of the invalid entry
        index: usize,
        /// Description of why the entry is invalid
        reason: String,
    },

    /// Patch application failed
    #[error("Patch application failed: {reason}")]
    ApplicationFailed {
        /// Description of the failure
        reason: String,
    },

    /// Insufficient data for operation
    #[error("Insufficient data: need {needed} bytes, got {available} bytes")]
    InsufficientData {
        /// Bytes needed
        needed: usize,
        /// Bytes available
        available: usize,
    },

    /// Control block is empty when it shouldn't be
    #[error("Empty control block")]
    EmptyControlBlock,

    /// Error reading from old file during patch application
    #[error("Error reading old file: {0}")]
    OldFileReadError(std::io::Error),

    /// Async join error (for batch processing)
    #[error("Join error: {0}")]
    JoinError(String),

    /// BLTE decompression error (for integration)
    #[error("BLTE error: {0}")]
    BlteError(String),
}

/// Result type for ZBSDIFF1 operations
pub type ZbsdiffResult<T> = Result<T, ZbsdiffError>;

impl ZbsdiffError {
    /// Create a corrupt patch error with a reason
    pub fn corrupt_patch<S: Into<String>>(reason: S) -> Self {
        Self::CorruptPatch {
            reason: reason.into(),
        }
    }

    /// Create an invalid control entry error
    pub fn invalid_control_entry<S: Into<String>>(index: usize, reason: S) -> Self {
        Self::InvalidControlEntry {
            index,
            reason: reason.into(),
        }
    }

    /// Create an application failed error
    pub fn application_failed<S: Into<String>>(reason: S) -> Self {
        Self::ApplicationFailed {
            reason: reason.into(),
        }
    }

    /// Create an insufficient data error
    pub fn insufficient_data(needed: usize, available: usize) -> Self {
        Self::InsufficientData { needed, available }
    }

    /// Create a decompression error from an IO error
    pub fn decompression_error(error: std::io::Error) -> Self {
        Self::DecompressionError(error)
    }

    /// Create an old file read error
    pub fn old_file_read_error(error: std::io::Error) -> Self {
        Self::OldFileReadError(error)
    }

    /// Check if this error indicates corrupt or invalid data
    pub fn is_corruption_error(&self) -> bool {
        matches!(
            self,
            ZbsdiffError::CorruptPatch { .. }
                | ZbsdiffError::InvalidSignature { .. }
                | ZbsdiffError::InvalidSize { .. }
                | ZbsdiffError::InvalidControlEntry { .. }
                | ZbsdiffError::NotZbsdiffFormat
        )
    }

    /// Check if this error is likely recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ZbsdiffError::CompressionError(_)
                | ZbsdiffError::DecompressionError(_)
                | ZbsdiffError::SeekError(_)
                | ZbsdiffError::OldFileReadError(_)
        )
    }
}

// Conversion from binrw errors with more context
impl From<binrw::Error> for ZbsdiffError {
    fn from(error: binrw::Error) -> Self {
        // Check if it's a parsing assertion failure (invalid signature)
        if let binrw::Error::AssertFail { .. } = error {
            // This is likely an invalid signature, but we can't extract the values
            // from the error, so we'll use a generic corruption error
            return ZbsdiffError::corrupt_patch("Invalid header signature or assertion failed");
        }

        ZbsdiffError::BinaryFormatError(error)
    }
}

// Conversion from IO errors to specific error types
impl From<std::io::Error> for ZbsdiffError {
    fn from(error: std::io::Error) -> Self {
        // Default to compression error, can be overridden with specific methods
        ZbsdiffError::CompressionError(error)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation_helpers() {
        let corrupt = ZbsdiffError::corrupt_patch("test corruption");
        assert!(matches!(corrupt, ZbsdiffError::CorruptPatch { .. }));

        let invalid_entry = ZbsdiffError::invalid_control_entry(5, "negative size");
        assert!(matches!(
            invalid_entry,
            ZbsdiffError::InvalidControlEntry { index: 5, .. }
        ));

        let app_failed = ZbsdiffError::application_failed("seek beyond EOF");
        assert!(matches!(app_failed, ZbsdiffError::ApplicationFailed { .. }));

        let insufficient = ZbsdiffError::insufficient_data(100, 50);
        assert!(matches!(
            insufficient,
            ZbsdiffError::InsufficientData {
                needed: 100,
                available: 50
            }
        ));
    }

    #[test]
    fn test_error_categorization() {
        let corruption_errors = vec![
            ZbsdiffError::CorruptPatch {
                reason: "test".to_string(),
            },
            ZbsdiffError::InvalidSignature {
                expected: 1,
                actual: 2,
            },
            ZbsdiffError::InvalidSize {
                field: "test",
                value: -1,
            },
            ZbsdiffError::NotZbsdiffFormat,
        ];

        for error in &corruption_errors {
            assert!(
                error.is_corruption_error(),
                "Error should be corruption: {:?}",
                error
            );
            assert!(
                !error.is_recoverable(),
                "Corruption error should not be recoverable: {:?}",
                error
            );
        }

        let recoverable_errors = vec![
            ZbsdiffError::CompressionError(std::io::Error::other("test")),
            ZbsdiffError::DecompressionError(std::io::Error::other("test")),
        ];

        for error in &recoverable_errors {
            assert!(
                !error.is_corruption_error(),
                "Error should not be corruption: {:?}",
                error
            );
            assert!(
                error.is_recoverable(),
                "Error should be recoverable: {:?}",
                error
            );
        }
    }

    #[test]
    fn test_error_display() {
        let error = ZbsdiffError::SizeMismatch {
            expected: 100,
            actual: 50,
        };
        let message = error.to_string();
        assert!(message.contains("100"));
        assert!(message.contains("50"));

        let invalid_sig = ZbsdiffError::InvalidSignature {
            expected: 0x1234,
            actual: 0x5678,
        };
        let sig_message = invalid_sig.to_string();
        assert!(sig_message.contains("0x1234"));
        assert!(sig_message.contains("0x5678"));
    }
}

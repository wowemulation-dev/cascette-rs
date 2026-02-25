//! Error types for download manifest parsing and building

use thiserror::Error;

/// Result type alias for download operations
pub type Result<T> = std::result::Result<T, DownloadError>;

/// Errors that can occur when parsing or building download manifests
#[derive(Debug, Error)]
pub enum DownloadError {
    /// Invalid magic bytes (expected "DL")
    #[error("Invalid magic: expected 'DL', got {0:?}")]
    InvalidMagic([u8; 2]),

    /// Unsupported format version
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),

    /// File size too large for 40-bit field
    #[error("File size too large for 40-bit field: {0} (max: 1,099,511,627,775)")]
    FileSizeTooLarge(u64),

    /// Flags not supported in specified version
    #[error("Flags not supported in version {0} (requires version 2+)")]
    FlagsNotSupportedInVersion(u8),

    /// Base priority not supported in specified version
    #[error("Base priority not supported in version {0} (requires version 3+)")]
    BasePriorityNotSupportedInVersion(u8),

    /// Invalid flag size for entry
    #[error("Invalid flag size: got {0} bytes, expected {1} bytes")]
    InvalidFlagSize(usize, u8),

    /// Unsupported flag size in header (Agent.exe rejects > 4)
    #[error("Unsupported number of flag bytes in download manifest: {0}")]
    UnsupportedFlagSize(u8),

    /// Missing required checksum
    #[error("Missing checksum when required by header")]
    MissingChecksum,

    /// Missing required flags
    #[error("Missing flags when required by header")]
    MissingFlags,

    /// Checksums not enabled in manifest
    #[error("Checksums not enabled in this manifest")]
    ChecksumsNotEnabled,

    /// Flags not enabled in manifest
    #[error("Flags not enabled in this manifest")]
    FlagsNotEnabled,

    /// File index out of bounds
    #[error("File index out of bounds: {0}")]
    FileIndexOutOfBounds(usize),

    /// Tag not found by name
    #[error("Tag not found: {0}")]
    TagNotFound(String),

    /// Bit mask size mismatch
    #[error("Bit mask size mismatch between header and data")]
    BitMaskSizeMismatch,

    /// Entry count mismatch
    #[error("Entry count mismatch: header says {0}, found {1}")]
    EntryCountMismatch(u32, usize),

    /// Tag count mismatch
    #[error("Tag count mismatch: header says {0}, found {1}")]
    TagCountMismatch(u16, usize),

    /// Invalid encoding key length
    #[error("Invalid encoding key length: got {0} bytes, expected 16 bytes")]
    InvalidEncodingKeyLength(u8),

    /// Version validation failed
    #[error("Version {0} validation failed: {1}")]
    VersionValidationFailed(u8, String),

    /// Reserved field validation failed
    #[error("Reserved field must be zero, got: {0:?}")]
    ReservedFieldNotZero([u8; 3]),

    /// Priority calculation overflow
    #[error("Priority calculation overflow: priority={0}, base_priority={1}")]
    PriorityCalculationOverflow(i8, i8),

    /// IO error during parsing or building
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Binary parsing/writing error
    #[error("Binary parsing error: {0}")]
    BinRead(#[from] binrw::Error),

    /// UTF-8 conversion error for tag names
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    /// Hex decoding error for keys
    #[error("Hex decoding error: {0}")]
    HexError(#[from] hex::FromHexError),

    /// Install format error (for tag operations)
    #[error("Install format error: {0}")]
    InstallError(#[from] crate::install::InstallError),
}

impl DownloadError {
    /// Create a version validation error with context
    pub fn version_validation(version: u8, context: impl Into<String>) -> Self {
        Self::VersionValidationFailed(version, context.into())
    }

    /// Check if this error indicates a format compatibility issue
    pub fn is_format_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidMagic(_)
                | Self::UnsupportedVersion(_)
                | Self::InvalidEncodingKeyLength(_)
                | Self::BitMaskSizeMismatch
                | Self::EntryCountMismatch(_, _)
                | Self::TagCountMismatch(_, _)
        )
    }

    /// Check if this error indicates a version-specific feature issue
    pub fn is_version_error(&self) -> bool {
        matches!(
            self,
            Self::FlagsNotSupportedInVersion(_)
                | Self::BasePriorityNotSupportedInVersion(_)
                | Self::VersionValidationFailed(_, _)
        )
    }

    /// Check if this error indicates a builder configuration issue
    pub fn is_builder_error(&self) -> bool {
        matches!(
            self,
            Self::FileIndexOutOfBounds(_)
                | Self::TagNotFound(_)
                | Self::ChecksumsNotEnabled
                | Self::FlagsNotEnabled
                | Self::MissingChecksum
                | Self::MissingFlags
                | Self::InvalidFlagSize(_, _)
        )
    }

    /// Check if this error indicates a data validation issue
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            Self::FileSizeTooLarge(_)
                | Self::UnsupportedFlagSize(_)
                | Self::ReservedFieldNotZero(_)
                | Self::PriorityCalculationOverflow(_, _)
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        let format_error = DownloadError::InvalidMagic(*b"XX");
        assert!(format_error.is_format_error());
        assert!(!format_error.is_version_error());
        assert!(!format_error.is_builder_error());
        assert!(!format_error.is_validation_error());

        let version_error = DownloadError::UnsupportedVersion(99);
        assert!(version_error.is_format_error());
        assert!(!version_error.is_version_error());

        let flags_error = DownloadError::FlagsNotSupportedInVersion(1);
        assert!(!flags_error.is_format_error());
        assert!(flags_error.is_version_error());

        let builder_error = DownloadError::FileIndexOutOfBounds(100);
        assert!(!builder_error.is_format_error());
        assert!(builder_error.is_builder_error());

        let validation_error = DownloadError::FileSizeTooLarge(u64::MAX);
        assert!(!validation_error.is_format_error());
        assert!(validation_error.is_validation_error());
    }

    #[test]
    fn test_error_messages() {
        let error = DownloadError::InvalidMagic(*b"XX");
        assert_eq!(
            error.to_string(),
            "Invalid magic: expected 'DL', got [88, 88]"
        );

        let error = DownloadError::FileSizeTooLarge(0x1_0000_0000_0000);
        assert!(
            error
                .to_string()
                .contains("File size too large for 40-bit field")
        );

        let error = DownloadError::FlagsNotSupportedInVersion(1);
        assert!(error.to_string().contains("requires version 2+"));

        let error = DownloadError::BasePriorityNotSupportedInVersion(2);
        assert!(error.to_string().contains("requires version 3+"));
    }

    #[test]
    fn test_version_validation_helper() {
        let error = DownloadError::version_validation(2, "flags are required but not provided");
        match error {
            DownloadError::VersionValidationFailed(version, context) => {
                assert_eq!(version, 2);
                assert_eq!(context, "flags are required but not provided");
            }
            _ => unreachable!("Expected VersionValidationFailed"),
        }
    }

    #[test]
    fn test_error_display() {
        let errors = vec![
            DownloadError::InvalidMagic(*b"XX"),
            DownloadError::UnsupportedVersion(99),
            DownloadError::FileSizeTooLarge(u64::MAX),
            DownloadError::InvalidFlagSize(3, 2),
            DownloadError::MissingChecksum,
            DownloadError::FileIndexOutOfBounds(100),
            DownloadError::TagNotFound("NonExistent".to_string()),
            DownloadError::EntryCountMismatch(10, 5),
            DownloadError::ReservedFieldNotZero([1, 2, 3]),
        ];

        for error in errors {
            let message = error.to_string();
            assert!(!message.is_empty());
            assert!(!message.starts_with("Error"));
        }
    }
}

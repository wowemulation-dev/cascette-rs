//! Patch application error types

/// Errors that can occur during patch application
#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    /// BLTE decode failed
    #[error("BLTE decode error: {0}")]
    BlteDecode(#[from] cascette_formats::blte::BlteError),

    /// ZBSDIFF1 application failed
    #[error("ZBSDIFF1 error: {0}")]
    Zbsdiff(#[from] cascette_formats::zbsdiff::ZbsdiffError),

    /// ESpec parsing failed
    #[error("ESpec error: {0}")]
    ESpec(#[from] cascette_formats::espec::ESpecError),

    /// Patch chain construction failed
    #[error("patch chain error: {0}")]
    Chain(#[from] cascette_formats::patch_chain::PatchChainError),

    /// Output size does not match expected
    #[error("size mismatch: expected {expected}, got {actual}")]
    SizeMismatch {
        /// Expected size
        expected: usize,
        /// Actual size
        actual: usize,
    },

    /// Output hash does not match expected
    #[error(
        "hash mismatch: expected {}, got {}",
        hex::encode(expected),
        hex::encode(actual)
    )]
    HashMismatch {
        /// Expected MD5
        expected: [u8; 16],
        /// Actual MD5
        actual: [u8; 16],
    },

    /// Base file required but not available
    #[error("missing base file for patch")]
    MissingBaseFile,

    /// Patch data required but not available
    #[error("missing patch data")]
    MissingPatchData,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Format-level error from CascFormat trait operations
    #[error("format error: {0}")]
    Format(String),
}

impl From<Box<dyn std::error::Error>> for PatchError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self::Format(err.to_string())
    }
}

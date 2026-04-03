//! Error types for installation operations.

use std::io;

use cascette_client_storage::StorageError;
use cascette_formats::bpsv::BpsvError;
use cascette_protocol::ProtocolError;

/// Result type alias for installation operations.
pub type InstallationResult<T> = Result<T, InstallationError>;

/// Errors that can occur during installation operations.
#[derive(Debug, thiserror::Error)]
pub enum InstallationError {
    /// Protocol-level error (CDN, Ribbit, TACT).
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Binary format parsing error.
    #[error("format error: {0}")]
    Format(String),

    /// BPSV format error.
    #[error("BPSV error: {0}")]
    Bpsv(#[from] BpsvError),

    /// Local CASC storage error.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    /// File I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Checkpoint read/write error.
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// CDN download error.
    #[error("CDN error: {0}")]
    Cdn(String),

    /// Requested content not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Path traversal attempt detected.
    #[error("path traversal: {0}")]
    PathTraversal(String),

    /// Hex decoding error.
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Patch application error.
    #[error("patch error: {0}")]
    Patch(#[from] crate::patch::error::PatchError),

    /// Update prerequisite not met.
    #[error("update prerequisite: {0}")]
    UpdatePrerequisite(String),

    /// Alternate source error.
    #[error("alternate source: {0}")]
    AlternateSource(String),

    /// All CDN endpoints dropped below the minimum health threshold.
    #[error("all CDN endpoints exhausted (scores below minimum threshold)")]
    AllEndpointsExhausted,
}

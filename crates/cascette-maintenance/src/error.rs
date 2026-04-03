//! Error types for maintenance operations.

use cascette_client_storage::StorageError;

/// Result type alias for maintenance operations.
pub type MaintenanceResult<T> = Result<T, MaintenanceError>;

/// Errors that can occur during maintenance operations.
#[derive(Debug, thiserror::Error)]
pub enum MaintenanceError {
    /// Error from the underlying storage layer.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Error building the preservation set.
    #[error("preservation error: {0}")]
    Preservation(String),

    /// Error during garbage collection.
    #[error("garbage collection error: {0}")]
    GarbageCollection(String),

    /// Error during compaction.
    #[error("compaction error: {0}")]
    Compaction(String),

    /// Error during repair.
    #[error("repair error: {0}")]
    Repair(String),

    /// Error from the installation pipeline.
    #[error("installation error: {0}")]
    Installation(#[from] cascette_installation::InstallationError),
}

//! Error types for the agent service.

use std::io;

/// Result type alias for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

/// Errors that can occur in the agent service.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// HTTP server failed to bind to the requested port.
    #[error("failed to bind HTTP server to port {port}: {source}")]
    BindFailed {
        /// Port that failed to bind.
        port: u16,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// SQLite database error.
    #[error("database error: {0}")]
    Database(#[from] turso::Error),

    /// Installation pipeline error.
    #[error("installation error: {0}")]
    Installation(#[from] cascette_installation::InstallationError),

    /// Protocol (Ribbit/CDN) error.
    #[error("protocol error: {0}")]
    Protocol(#[from] cascette_protocol::ProtocolError),

    /// Client storage error.
    #[error("storage error: {0}")]
    Storage(#[from] cascette_client_storage::StorageError),

    /// File I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid state transition.
    #[error("invalid state transition: {from} -> {to}")]
    InvalidTransition {
        /// Current state.
        from: String,
        /// Attempted target state.
        to: String,
    },

    /// Product not found.
    #[error("product not found: {0}")]
    ProductNotFound(String),

    /// Operation not found.
    #[error("operation not found: {0}")]
    OperationNotFound(String),

    /// Product already has an active operation.
    #[error("product {product} already has an active {operation_type} operation")]
    ActiveOperationExists {
        /// Product code.
        product: String,
        /// Type of active operation.
        operation_type: String,
    },

    /// Product is in wrong state for the requested operation.
    #[error("product {product} is {status}, cannot {operation}")]
    InvalidProductState {
        /// Product code.
        product: String,
        /// Current product status.
        status: String,
        /// Attempted operation.
        operation: String,
    },

    /// Game process is running, preventing the operation.
    #[error("game process running for {product}, cannot {operation}")]
    GameProcessRunning {
        /// Product code.
        product: String,
        /// Attempted operation.
        operation: String,
    },

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Schema migration error.
    #[error("schema error: {0}")]
    Schema(String),

    /// Operation was cancelled.
    #[error("operation cancelled: {0}")]
    Cancelled(String),

    /// Network operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),
}

//! Error types for the Ribbit server.
//!
//! All errors use thiserror for consistent error handling across the codebase.

use std::path::PathBuf;
use thiserror::Error;

/// Database-related errors.
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// Failed to load builds from JSON file
    #[error("Failed to load builds from {path}: {source}")]
    LoadFailed {
        /// Path to the builds.json file
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// Invalid JSON format in builds file
    #[error("Invalid JSON in builds file: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// No builds found for product
    #[error("No builds found for product: {0}")]
    ProductNotFound(String),

    /// Empty database (no builds loaded)
    #[error("Database is empty: no builds loaded from file")]
    EmptyDatabase,

    /// Invalid field value in build record
    #[error("Invalid {field} in build {build_id}: {reason}")]
    InvalidField {
        /// Field name that failed validation
        field: String,
        /// Build ID where validation failed
        build_id: u64,
        /// Reason for validation failure
        reason: String,
    },
}

/// Configuration-related errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Invalid bind address format
    #[error("Invalid bind address '{address}': {reason}")]
    InvalidBindAddress {
        /// The invalid address string
        address: String,
        /// Reason for invalidity
        reason: String,
    },

    /// TLS configuration error
    #[error("TLS configuration error: {0}")]
    TlsConfig(String),

    /// Missing required configuration value
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),
}

/// Server runtime errors.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Failed to bind HTTP server
    #[error("Failed to bind HTTP server to {addr}: {source}")]
    HttpBindFailed {
        /// Address that failed to bind
        addr: std::net::SocketAddr,
        /// Underlying error
        #[source]
        source: std::io::Error,
    },

    /// Failed to bind TCP server
    #[error("Failed to bind TCP server to {addr}: {source}")]
    TcpBindFailed {
        /// Address that failed to bind
        addr: std::net::SocketAddr,
        /// Underlying error
        #[source]
        source: std::io::Error,
    },

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Server shutdown error
    #[error("Server shutdown error: {0}")]
    Shutdown(String),
}

/// Protocol-level errors for TCP/HTTP handlers.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Invalid command syntax
    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    /// Unsupported protocol version
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(String),

    /// I/O error during protocol handling
    #[error("Protocol I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Connection timeout
    #[error("Connection timeout after {seconds} seconds")]
    Timeout {
        /// Number of seconds before timeout
        seconds: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_error_messages() {
        let err = DatabaseError::ProductNotFound("test_product".to_string());
        assert_eq!(err.to_string(), "No builds found for product: test_product");

        let err = DatabaseError::EmptyDatabase;
        assert_eq!(
            err.to_string(),
            "Database is empty: no builds loaded from file"
        );
    }

    #[test]
    fn test_server_error_conversion() {
        let db_err = DatabaseError::ProductNotFound("wow".to_string());
        let server_err: ServerError = db_err.into();
        assert!(server_err.to_string().contains("No builds found"));
    }
}

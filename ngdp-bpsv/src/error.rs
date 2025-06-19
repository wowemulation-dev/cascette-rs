//! Error types for BPSV parsing and building

use thiserror::Error;

/// Result type for BPSV operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during BPSV operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum Error {
    /// Error parsing field type specification
    #[error("Invalid field type: {field_type}")]
    InvalidFieldType { field_type: String },

    /// Error parsing header line
    #[error("Invalid header format: {reason}")]
    InvalidHeader { reason: String },

    /// Error parsing sequence number
    #[error("Invalid sequence number: {line}")]
    InvalidSequenceNumber { line: String },

    /// Mismatch between schema and data
    #[error("Schema mismatch: expected {expected} fields, got {actual}")]
    SchemaMismatch { expected: usize, actual: usize },

    /// Invalid value for field type
    #[error("Invalid value for field '{field}' of type {field_type}: {value}")]
    InvalidValue {
        field: String,
        field_type: String,
        value: String,
    },

    /// Missing required header
    #[error("Missing header line")]
    MissingHeader,

    /// Empty document
    #[error("Document is empty")]
    EmptyDocument,

    /// Field not found in schema
    #[error("Field '{field}' not found in schema")]
    FieldNotFound { field: String },

    /// Duplicate field name
    #[error("Duplicate field name: {field}")]
    DuplicateField { field: String },

    /// Row validation error
    #[error("Row {row_index} validation failed: {reason}")]
    RowValidation { row_index: usize, reason: String },

    /// Hex decoding error
    #[error("Invalid hex value: {value}")]
    InvalidHex { value: String },

    /// Number parsing error
    #[error("Invalid number: {value}")]
    InvalidNumber { value: String },

    /// IO error during parsing or writing
    #[error("IO error: {0}")]
    Io(String),
}

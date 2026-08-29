//! Error types for import operations.

use thiserror::Error;

/// Result type alias for import operations.
pub type ImportResult<T> = Result<T, ImportError>;

/// Errors that can occur during import operations.
#[derive(Debug, Error)]
pub enum ImportError {
    /// Network communication error.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON parsing error.
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O operation error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Cache operation error.
    #[error("cache error: {0}")]
    Cache(String),

    /// Provider configuration error.
    #[error("provider configuration error: {0}")]
    Config(String),

    /// API rate limit exceeded.
    #[error("rate limit exceeded for provider {provider}: {message}")]
    RateLimit {
        /// Provider that hit the rate limit.
        provider: String,
        /// Rate limit message.
        message: String,
    },

    /// Authentication failure.
    #[error("authentication failed for provider {0}")]
    Authentication(String),

    /// Data validation error.
    #[error("data validation error: {0}")]
    Validation(String),

    /// Provider not available.
    #[error("provider {0} is not available: {1}")]
    ProviderUnavailable(String, String),

    /// Data not found.
    #[error("data not found: {0}")]
    NotFound(String),

    /// Provider operation timeout.
    #[error("provider {0} operation timed out")]
    Timeout(String),

    /// Invalid data format.
    #[error("invalid data format from {provider}: {message}")]
    InvalidFormat {
        /// Provider that returned invalid data.
        provider: String,
        /// Description of the format issue.
        message: String,
    },

    /// HTTP status error.
    #[error("HTTP {status} error from {provider}: {message}")]
    HttpStatus {
        /// Provider that returned the error.
        provider: String,
        /// HTTP status code.
        status: u16,
        /// Error message from the response.
        message: String,
    },

    /// Provider error.
    #[error("provider {provider} error: {message}")]
    Provider {
        /// Provider that encountered the error.
        provider: String,
        /// Error message.
        message: String,
    },
}

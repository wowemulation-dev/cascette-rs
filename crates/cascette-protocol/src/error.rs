//! Error types for protocol operations

use reqwest::StatusCode;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Cache error: {0}")]
    Cache(#[from] crate::cache::CacheError),

    #[error("All hosts failed")]
    AllHostsFailed,

    #[error("Rate limited")]
    RateLimited {
        /// Duration from the HTTP Retry-After header, if present
        retry_after: Option<Duration>,
    },

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("HTTP status: {0}")]
    HttpStatus(StatusCode),

    #[error("Server error: {0}")]
    ServerError(StatusCode),

    #[error("Invalid key")]
    InvalidKey,

    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),

    #[error("Range not supported")]
    RangeNotSupported,

    #[error("Timeout")]
    Timeout,

    #[error("Other error: {0}")]
    Other(String),

    #[error("UTF-8 error")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Unsupported on WASM: {0}")]
    UnsupportedOnWasm(String),
}

impl ProtocolError {
    /// Check if error is retryable
    #[cfg(not(target_arch = "wasm32"))]
    pub fn should_retry(&self) -> bool {
        match self {
            // Transient errors that should be retried
            Self::Network(_)
            | Self::ServerError(_)
            | Self::RateLimited { .. }
            | Self::ServiceUnavailable
            | Self::Timeout => true,
            Self::Http(e) => e.is_timeout() || e.is_connect(),
            Self::HttpStatus(status) => {
                matches!(
                    status,
                    &StatusCode::TOO_MANY_REQUESTS
                        | &StatusCode::INTERNAL_SERVER_ERROR
                        | &StatusCode::BAD_GATEWAY
                        | &StatusCode::SERVICE_UNAVAILABLE
                        | &StatusCode::GATEWAY_TIMEOUT
                )
            }
            _ => false,
        }
    }

    /// Check if error is retryable (WASM version)
    ///
    /// On WASM, reqwest doesn't expose `is_connect()` method since the browser
    /// handles connection management. We check only for timeout errors.
    #[cfg(target_arch = "wasm32")]
    pub fn should_retry(&self) -> bool {
        match self {
            // Transient errors that should be retried
            Self::Network(_)
            | Self::ServerError(_)
            | Self::RateLimited { .. }
            | Self::ServiceUnavailable
            | Self::Timeout => true,
            // On WASM, is_connect() is not available, only check timeout
            Self::Http(e) => e.is_timeout(),
            Self::HttpStatus(status) => {
                matches!(
                    status,
                    &StatusCode::TOO_MANY_REQUESTS
                        | &StatusCode::INTERNAL_SERVER_ERROR
                        | &StatusCode::BAD_GATEWAY
                        | &StatusCode::SERVICE_UNAVAILABLE
                        | &StatusCode::GATEWAY_TIMEOUT
                )
            }
            _ => false,
        }
    }

    /// Get the Retry-After hint duration, if this is a rate-limited error with one.
    pub fn retry_after_hint(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, ProtocolError>;

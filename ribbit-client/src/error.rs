//! Error types for the Ribbit client
//!
//! This module defines the error types that can occur when using the Ribbit client.
//!
//! # Example
//!
//! ```
//! use ribbit_client::{Error, Result, Region};
//! use std::str::FromStr;
//!
//! fn example() -> Result<()> {
//!     // This would return an InvalidRegion error
//!     match Region::from_str("invalid") {
//!         Ok(region) => Ok(()),
//!         Err(e) => Err(e),
//!     }
//! }
//!
//! // Example of error handling
//! let result = example();
//! assert!(result.is_err());
//! ```

use thiserror::Error;

/// Error types that can occur when using the Ribbit client
#[derive(Debug, Error)]
pub enum Error {
    /// IO error occurred during network operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid region string provided
    #[error("Invalid region: {0}")]
    InvalidRegion(String),

    /// Failed to connect to the Ribbit server
    #[error("Connection failed to {host}:{port}")]
    ConnectionFailed {
        /// The hostname that failed to connect
        host: String,
        /// The port number that failed to connect
        port: u16,
    },

    /// Connection timed out
    #[error("Connection timed out after {timeout_secs}s to {host}:{port}")]
    ConnectionTimeout {
        /// The hostname that timed out
        host: String,
        /// The port number that timed out
        port: u16,
        /// The timeout duration in seconds
        timeout_secs: u64,
    },

    /// Failed to send request to the server
    #[error("Failed to send request")]
    SendFailed,

    /// Failed to receive response from the server
    #[error("Failed to receive response")]
    ReceiveFailed,

    /// Response format is invalid or unexpected
    #[error("Invalid response format")]
    InvalidResponse,

    /// MIME parsing failed for V1 protocol responses
    #[error("MIME parsing error: {0}")]
    MimeParseError(String),

    /// Checksum validation failed for V1 protocol responses
    #[error("Checksum validation failed")]
    ChecksumMismatch,

    /// ASN.1 parsing failed for signature data
    #[error("ASN.1 parsing error: {0}")]
    Asn1Error(String),

    /// General parsing error for response data
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result type alias using the Ribbit Error type
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::InvalidRegion("xyz".to_string());
        assert_eq!(err.to_string(), "Invalid region: xyz");

        let err = Error::ConnectionFailed {
            host: "test.battle.net".to_string(),
            port: 1119,
        };
        assert_eq!(err.to_string(), "Connection failed to test.battle.net:1119");

        let err = Error::ConnectionTimeout {
            host: "test.battle.net".to_string(),
            port: 1119,
            timeout_secs: 10,
        };
        assert_eq!(
            err.to_string(),
            "Connection timed out after 10s to test.battle.net:1119"
        );

        let err = Error::SendFailed;
        assert_eq!(err.to_string(), "Failed to send request");

        let err = Error::ReceiveFailed;
        assert_eq!(err.to_string(), "Failed to receive response");

        let err = Error::InvalidResponse;
        assert_eq!(err.to_string(), "Invalid response format");

        let err = Error::MimeParseError("bad format".to_string());
        assert_eq!(err.to_string(), "MIME parsing error: bad format");

        let err = Error::ChecksumMismatch;
        assert_eq!(err.to_string(), "Checksum validation failed");

        let err = Error::Asn1Error("invalid ASN.1".to_string());
        assert_eq!(err.to_string(), "ASN.1 parsing error: invalid ASN.1");

        let err = Error::ParseError("invalid BPSV".to_string());
        assert_eq!(err.to_string(), "Parse error: invalid BPSV");
    }

    #[test]
    fn test_error_from_io() {
        use std::io::{Error as IoError, ErrorKind};

        let io_err = IoError::new(ErrorKind::ConnectionRefused, "refused");
        let err: Error = io_err.into();

        match err {
            Error::Io(_) => {}
            _ => panic!("Expected Error::Io variant"),
        }
    }
}

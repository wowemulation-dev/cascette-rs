//! Error types for cryptographic operations

use thiserror::Error;

/// Errors that can occur during cryptographic operations
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Invalid key size
    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize {
        /// Expected key size in bytes
        expected: usize,
        /// Actual key size in bytes
        actual: usize,
    },

    /// Invalid IV size
    #[error("Invalid IV size: expected {expected}, got {actual}")]
    InvalidIvSize {
        /// Expected IV size in bytes
        expected: usize,
        /// Actual IV size in bytes
        actual: usize,
    },

    /// Key not found
    #[error("Encryption key not found: {0:016x}")]
    KeyNotFound(u64),

    /// Invalid key format
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),

    /// Keyring service unavailable (when keyring feature is enabled)
    #[cfg(feature = "keyring")]
    #[error("Keyring service unavailable: {reason}")]
    KeyringServiceUnavailable {
        /// Reason for unavailability
        reason: String,
    },

    /// Keyring access denied (when keyring feature is enabled)
    #[cfg(feature = "keyring")]
    #[error("Key access denied by OS: {key_id}")]
    KeyringAccessDenied {
        /// Key ID that was denied
        key_id: u64,
    },

    /// Keyring I/O error (when keyring feature is enabled)
    #[cfg(feature = "keyring")]
    #[error("Keyring I/O error: {0}")]
    KeyringIoError(String),

    /// Key serialization error (when keyring feature is enabled)
    #[cfg(feature = "keyring")]
    #[error("Key serialization error: {0}")]
    KeySerialization(String),
}

#[cfg(feature = "keyring")]
impl From<keyring::Error> for CryptoError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoEntry => Self::KeyNotFound(0), // Key ID unknown at this level
            keyring::Error::Invalid(_, _) => {
                Self::KeySerialization("Invalid keyring entry format".to_string())
            }
            keyring::Error::PlatformFailure(msg) => Self::KeyringServiceUnavailable {
                reason: format!("Platform keyring failure: {msg}"),
            },
            _ => Self::KeyringIoError(format!("Keyring operation failed: {err}")),
        }
    }
}

#[cfg(feature = "keyring")]
impl From<base64::DecodeError> for CryptoError {
    fn from(err: base64::DecodeError) -> Self {
        Self::KeySerialization(format!("Base64 decode failed: {err}"))
    }
}

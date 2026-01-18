//! Cryptographic operations for NGDP/CASC system
//!
//! This crate provides the cryptographic primitives used throughout the CASC
//! system for content hashing, integrity verification, and encryption.
//!
//! # Components
//!
//! - **Hashing**: MD5 for content keys, Jenkins96 for archive indices
//! - **Encryption**: Salsa20 stream cipher for content protection, ARC4 for legacy blocks
//! - **Key Management**: TACT encryption key storage and lookup
//!
//! # Examples
//!
//! ## Content Key Generation
//!
//! ```
//! use cascette_crypto::md5::ContentKey;
//!
//! let data = b"Hello, World!";
//! let content_key = ContentKey::from_data(data);
//! println!("Content key: {}", content_key);
//! ```
//!
//! ## Jenkins96 Hashing
//!
//! ```
//! use cascette_crypto::jenkins::Jenkins96;
//!
//! let hash = Jenkins96::hash(b"test data");
//! println!("Jenkins96: {}", hash);
//! ```

#![warn(missing_docs)]

pub mod arc4;
pub mod error;
#[cfg(feature = "file-store")]
pub mod file_store;
pub mod jenkins;
#[cfg(feature = "keyring")]
pub mod keyring;
pub mod keys;
pub mod md5;
pub mod salsa20;
pub mod store_trait;

// Test modules
#[cfg(test)]
#[cfg(feature = "keyring")]
mod keyring_error_tests;
#[cfg(test)]
#[cfg(feature = "keyring")]
mod keyring_iterator_tests;
#[cfg(test)]
#[cfg(feature = "keyring")]
mod keyring_metrics_tests;

pub use error::CryptoError;

// Re-export commonly used types
pub use arc4::Arc4Cipher;
#[cfg(feature = "file-store")]
pub use file_store::{FileBasedTactKeyStore, FileStoreConfig, FileStoreError, KeyMetadata};
pub use jenkins::{Jenkins96, hashlittle, hashlittle2};
pub use keys::TactKey;
// Legacy TactKeyStore re-exported for backward compatibility
#[cfg(feature = "keyring")]
pub use keyring::{
    KeyringConfig, KeyringError, KeyringMetricsSnapshot, KeyringTactKeyStore as TactKeyStore,
};
pub use keys::TactKeyStore as LegacyTactKeyStore;
// Direct alias for the new implementation
#[cfg(feature = "keyring")]
pub use keyring::KeyringTactKeyStore;
pub use md5::{ContentKey, EncodingKey, FileDataId};
pub use salsa20::Salsa20Cipher;
pub use store_trait::{TactKeyIterator, TactKeyProvider, TactKeyStoreConfig, UnifiedKeyStore};

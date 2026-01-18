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
//! # Key Storage
//!
//! This crate provides an in-memory key store and a trait for custom backends:
//!
//! - [`TactKeyStore`] - In-memory storage with hardcoded WoW keys
//! - [`TactKeyProvider`] - Trait for implementing custom storage backends
//!
//! Applications can implement `TactKeyProvider` for persistent storage (keyring,
//! database, encrypted files, etc.).
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
//!
//! ## Custom Key Storage Backend
//!
//! ```
//! use cascette_crypto::{TactKeyProvider, TactKey, CryptoError};
//! use std::collections::HashMap;
//!
//! struct MyKeyStore {
//!     keys: HashMap<u64, [u8; 16]>,
//! }
//!
//! impl TactKeyProvider for MyKeyStore {
//!     fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
//!         Ok(self.keys.get(&id).copied())
//!     }
//!
//!     fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
//!         self.keys.insert(key.id, key.key);
//!         Ok(())
//!     }
//!
//!     fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
//!         Ok(self.keys.remove(&id))
//!     }
//!
//!     fn key_count(&self) -> Result<usize, CryptoError> {
//!         Ok(self.keys.len())
//!     }
//!
//!     fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError> {
//!         Ok(self.keys.keys().copied().collect())
//!     }
//! }
//! ```

#![warn(missing_docs)]

pub mod arc4;
pub mod error;
pub mod jenkins;
pub mod keys;
pub mod md5;
pub mod salsa20;
pub mod store_trait;

pub use error::CryptoError;

// Re-export commonly used types
pub use arc4::Arc4Cipher;
pub use jenkins::{hashlittle, hashlittle2, Jenkins96};
pub use keys::{TactKey, TactKeyStore};
pub use md5::{ContentKey, EncodingKey, FileDataId};
pub use salsa20::Salsa20Cipher;
pub use store_trait::{TactKeyIterator, TactKeyProvider, TactKeyStoreConfig, UnifiedKeyStore};

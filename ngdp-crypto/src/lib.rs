//! Encryption and decryption support for NGDP/TACT files.
//!
//! This crate provides:
//! - Key management for TACT encryption keys
//! - Salsa20 stream cipher implementation for BLTE mode 'E'
//! - ARC4 cipher implementation (for legacy support - **DEPRECATED**)
//! - Hardcoded WoW encryption keys
//! - Key file loading from multiple formats

#![cfg_attr(test, allow(deprecated))]

#[deprecated(
    since = "0.4.0",
    note = "ARC4 module is deprecated and will be removed in v0.5.0. Use salsa20 module instead."
)]
pub mod arc4;
pub mod error;
pub mod key_service;
pub mod keys;
pub mod salsa20;

pub use error::CryptoError;
pub use key_service::KeyService;

// Re-export encryption/decryption functions
#[allow(deprecated)]
#[deprecated(
    since = "0.4.0",
    note = "ARC4 decryption is deprecated and will be removed in v0.5.0. Use decrypt_salsa20 instead."
)]
pub use arc4::decrypt_arc4;
#[allow(deprecated)]
#[deprecated(
    since = "0.4.0",
    note = "ARC4 encryption is deprecated and will be removed in v0.5.0. Use encrypt_salsa20 instead."
)]
pub use arc4::encrypt_arc4;
pub use salsa20::{decrypt_salsa20, encrypt_salsa20};

/// Result type for crypto operations.
pub type Result<T> = std::result::Result<T, CryptoError>;

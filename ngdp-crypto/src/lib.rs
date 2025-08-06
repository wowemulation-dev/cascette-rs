//! Encryption and decryption support for NGDP/TACT files.
//!
//! This crate provides:
//! - Key management for TACT encryption keys
//! - Salsa20 stream cipher implementation for BLTE mode 'E'
//! - ARC4 cipher implementation (for legacy support)
//! - Hardcoded WoW encryption keys
//! - Key file loading from multiple formats

pub mod arc4;
pub mod error;
pub mod key_service;
pub mod keys;
pub mod salsa20;

pub use error::CryptoError;
pub use key_service::KeyService;

/// Result type for crypto operations.
pub type Result<T> = std::result::Result<T, CryptoError>;

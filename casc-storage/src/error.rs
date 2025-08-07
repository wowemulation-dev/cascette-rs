//! Error types for CASC storage operations

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CascError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Index not found for bucket {0:02x}")]
    IndexNotFound(u8),

    #[error("Entry not found for EKey {0}")]
    EntryNotFound(String),

    #[error("Archive {0} not found")]
    ArchiveNotFound(u16),

    #[error("Invalid index format: {0}")]
    InvalidIndexFormat(String),

    #[error("Invalid archive format: {0}")]
    InvalidArchiveFormat(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Invalid bucket index: {0}")]
    InvalidBucketIndex(u8),

    #[error("Archive size exceeded: {size} > {max}")]
    ArchiveSizeExceeded { size: u64, max: u64 },

    #[error("Decompression error: {0}")]
    DecompressionError(String),

    #[error("Encryption key not found: {0}")]
    KeyNotFound(String),

    #[error("Storage is read-only")]
    ReadOnly,

    #[error("Storage verification failed: {0}")]
    VerificationFailed(String),

    #[error("BLTE error: {0}")]
    Blte(#[from] blte::error::Error),
}

pub type Result<T> = std::result::Result<T, CascError>;
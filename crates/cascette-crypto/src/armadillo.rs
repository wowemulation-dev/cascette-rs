//! Armadillo DRM key file parsing
//!
//! Agent.exe loads Armadillo encryption keys from two sources:
//!
//! 1. **Hex string**: 32-character hex string parsed into a 16-byte key
//! 2. **`.ak` file**: 20-byte file at `%APPDATA%\Battle.net\Armadillo\<keyname>.ak`
//!    containing a 16-byte key followed by a 4-byte truncated MD5 checksum
//!
//! This module provides parsing and serialization functions for both formats,
//! plus platform-specific path resolution for the Armadillo key directory.

use std::path::PathBuf;

use md5::{Digest, Md5};
use thiserror::Error;

/// Errors from Armadillo key operations
#[derive(Debug, Error)]
pub enum ArmadilloError {
    /// Hex string is not exactly 32 characters
    #[error("invalid hex key length: expected 32 characters, got {0}")]
    InvalidHexLength(usize),

    /// Hex string contains non-hex characters
    #[error("invalid hex character in key: {0}")]
    InvalidHex(#[from] hex::FromHexError),

    /// .ak file is not exactly 20 bytes
    #[error("invalid .ak file size: expected 20 bytes, got {0}")]
    InvalidFileSize(usize),

    /// MD5 checksum in .ak file does not match key data
    #[error("checksum mismatch in .ak file")]
    ChecksumMismatch,

    /// I/O error reading .ak file
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Could not determine the Armadillo key directory
    #[error("failed to resolve Armadillo key directory")]
    PathResolutionFailed,
}

/// Size of a TACT encryption key in bytes
const KEY_SIZE: usize = 16;

/// Size of the truncated MD5 checksum in .ak files
const CHECKSUM_SIZE: usize = 4;

/// Total size of an .ak file (key + checksum)
const AK_FILE_SIZE: usize = KEY_SIZE + CHECKSUM_SIZE;

/// Parse a 32-character hex string into a 16-byte encryption key.
///
/// # Errors
///
/// Returns [`ArmadilloError::InvalidHexLength`] if the string is not 32 characters,
/// or [`ArmadilloError::InvalidHex`] if it contains non-hex characters.
///
/// # Examples
///
/// ```
/// use cascette_crypto::armadillo::parse_hex_key;
///
/// let key = parse_hex_key("0123456789abcdef0123456789abcdef").unwrap();
/// assert_eq!(key[0], 0x01);
/// assert_eq!(key[15], 0xef);
/// ```
pub fn parse_hex_key(hex_str: &str) -> Result<[u8; KEY_SIZE], ArmadilloError> {
    if hex_str.len() != 32 {
        return Err(ArmadilloError::InvalidHexLength(hex_str.len()));
    }
    let bytes = hex::decode(hex_str)?;
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Parse a 20-byte `.ak` file into a 16-byte encryption key.
///
/// The file format is: 16 bytes of key data followed by 4 bytes of truncated
/// MD5 checksum. The checksum is the first 4 bytes of `MD5(key_data)`.
///
/// # Errors
///
/// Returns [`ArmadilloError::InvalidFileSize`] if the data is not 20 bytes,
/// or [`ArmadilloError::ChecksumMismatch`] if the checksum does not match.
pub fn parse_ak_file(data: &[u8]) -> Result<[u8; KEY_SIZE], ArmadilloError> {
    if data.len() != AK_FILE_SIZE {
        return Err(ArmadilloError::InvalidFileSize(data.len()));
    }

    let key_data = &data[..KEY_SIZE];
    let stored_checksum = &data[KEY_SIZE..AK_FILE_SIZE];

    let computed = Md5::digest(key_data);
    if stored_checksum != &computed[..CHECKSUM_SIZE] {
        return Err(ArmadilloError::ChecksumMismatch);
    }

    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(key_data);
    Ok(key)
}

/// Serialize a 16-byte key into a 20-byte `.ak` file format.
///
/// Appends the first 4 bytes of `MD5(key)` as a checksum.
pub fn write_ak_file(key: &[u8; KEY_SIZE]) -> [u8; AK_FILE_SIZE] {
    let mut out = [0u8; AK_FILE_SIZE];
    out[..KEY_SIZE].copy_from_slice(key);
    let digest = Md5::digest(key);
    out[KEY_SIZE..AK_FILE_SIZE].copy_from_slice(&digest[..CHECKSUM_SIZE]);
    out
}

/// Returns the platform-specific Armadillo key directory.
///
/// - **Windows**: `%APPDATA%\Battle.net\Armadillo`
/// - **Linux/macOS**: `$XDG_DATA_HOME/Battle.net/Armadillo`
///   (defaults to `~/.local/share/Battle.net/Armadillo`)
///
/// # Errors
///
/// Returns [`ArmadilloError::PathResolutionFailed`] if the base directory
/// cannot be determined.
pub fn armadillo_key_dir() -> Result<PathBuf, ArmadilloError> {
    #[cfg(target_os = "windows")]
    let base = dirs::config_dir();

    #[cfg(not(target_os = "windows"))]
    let base = dirs::data_dir();

    let base = base.ok_or(ArmadilloError::PathResolutionFailed)?;
    Ok(base.join("Battle.net").join("Armadillo"))
}

/// Load an Armadillo key from a `.ak` file by name.
///
/// Resolves the file path as `<armadillo_key_dir>/<keyname>.ak`, reads it,
/// and parses the key.
///
/// # Errors
///
/// Returns errors from path resolution, file I/O, or file parsing.
pub fn load_ak_key(keyname: &str) -> Result<[u8; KEY_SIZE], ArmadilloError> {
    let dir = armadillo_key_dir()?;
    let path = dir.join(format!("{keyname}.ak"));
    let data = std::fs::read(path)?;
    parse_ak_file(&data)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_key_valid() {
        let key = parse_hex_key("0123456789abcdef0123456789ABCDEF").unwrap();
        assert_eq!(
            key,
            [
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
                0xcd, 0xef
            ]
        );
    }

    #[test]
    fn parse_hex_key_wrong_length() {
        let err = parse_hex_key("0123456789abcdef").unwrap_err();
        assert!(matches!(err, ArmadilloError::InvalidHexLength(16)));
    }

    #[test]
    fn parse_hex_key_invalid_chars() {
        let err = parse_hex_key("0123456789abcdefGGGGGGGGGGGGGGGG").unwrap_err();
        assert!(matches!(err, ArmadilloError::InvalidHex(_)));
    }

    #[test]
    fn ak_file_round_trip() {
        let key = [0x42u8; 16];
        let file_data = write_ak_file(&key);
        assert_eq!(file_data.len(), 20);

        let parsed = parse_ak_file(&file_data).unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn ak_file_wrong_size() {
        let err = parse_ak_file(&[0u8; 10]).unwrap_err();
        assert!(matches!(err, ArmadilloError::InvalidFileSize(10)));
    }

    #[test]
    fn ak_file_bad_checksum() {
        let key = [0x42u8; 16];
        let mut file_data = write_ak_file(&key);
        // Corrupt the checksum
        file_data[16] ^= 0xff;

        let err = parse_ak_file(&file_data).unwrap_err();
        assert!(matches!(err, ArmadilloError::ChecksumMismatch));
    }

    #[test]
    fn armadillo_key_dir_does_not_panic() {
        // The actual path is platform-dependent, but the function should not panic.
        // It may return an error if no home directory is set (e.g., in CI).
        let _result = armadillo_key_dir();
    }
}

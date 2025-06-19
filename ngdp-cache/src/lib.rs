//! Generic caching functionality for NGDP components
//!
//! This crate provides a flexible caching system for NGDP-related data including:
//! - Generic cache for arbitrary data
//! - TACT protocol cache
//! - CDN content cache
//! - Ribbit response cache

use std::path::{Path, PathBuf};

pub mod cached_ribbit_client;
pub mod cdn;
pub mod error;
pub mod generic;
pub mod ribbit;
pub mod tact;

pub use error::{Error, Result};

/// Get the base NGDP cache directory
///
/// Returns a path like:
/// - Linux: `~/.cache/ngdp`
/// - macOS: `~/Library/Caches/ngdp`
/// - Windows: `C:\Users\{user}\AppData\Local\ngdp\cache`
pub fn get_cache_dir() -> Result<PathBuf> {
    dirs::cache_dir()
        .ok_or(Error::CacheDirectoryNotFound)
        .map(|dir| dir.join("ngdp"))
}

/// Ensure a directory exists, creating it if necessary
pub(crate) async fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        tokio::fs::create_dir_all(path).await?;
    }
    Ok(())
}

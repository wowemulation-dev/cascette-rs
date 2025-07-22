//! Generic caching functionality for NGDP components
//!
//! This crate provides a flexible caching system for NGDP-related data including:
//! - Generic cache for arbitrary data
//! - CDN content cache (config, data, patch, indices)
//! - Ribbit response cache
//! - Cached clients for Ribbit, TACT, and CDN protocols

use std::path::{Path, PathBuf};

pub mod cached_cdn_client;
pub mod cached_ribbit_client;
pub mod cached_tact_client;
mod cache;
pub mod cdn;
pub mod error;
pub mod generic;
pub mod ribbit;

pub use cache::Cache;
pub use cdn::CdnCache;
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
pub(crate) async fn ensure_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if tokio::fs::metadata(path).await.is_err() {
        tokio::fs::create_dir_all(path).await?;
    }
    Ok(())
}

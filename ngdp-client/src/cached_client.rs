//! Cached Ribbit client support for the CLI
//!
//! This module provides a cached Ribbit client that can be used throughout
//! the CLI to reduce redundant API calls.

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::Region;
use std::sync::OnceLock;

/// Global flag to control whether caching is enabled
static CACHING_ENABLED: OnceLock<bool> = OnceLock::new();

/// Set whether caching is enabled globally
pub fn set_caching_enabled(enabled: bool) {
    let _ = CACHING_ENABLED.set(enabled);
}

/// Check if caching is enabled (defaults to true)
pub fn is_caching_enabled() -> bool {
    *CACHING_ENABLED.get_or_init(|| true)
}

/// Create a Ribbit client with optional caching
///
/// If caching is enabled (default), returns a CachedRibbitClient that
/// transparently caches all requests. Otherwise returns a regular RibbitClient.
pub async fn create_client(
    region: Region,
) -> Result<CachedRibbitClient, Box<dyn std::error::Error>> {
    let mut client = CachedRibbitClient::new(region).await?;

    // If caching is disabled globally, disable it on the client
    if !is_caching_enabled() {
        client.set_caching_enabled(false);
    }

    Ok(client)
}

/// Create a client with a specific cache directory (for testing)
#[cfg(test)]
pub async fn create_client_with_cache_dir(
    region: Region,
    cache_dir: std::path::PathBuf,
) -> Result<CachedRibbitClient, Box<dyn std::error::Error>> {
    let mut client = CachedRibbitClient::with_cache_dir(region, cache_dir).await?;

    if !is_caching_enabled() {
        client.set_caching_enabled(false);
    }

    Ok(client)
}

//! Cached client support with automatic Ribbit to TACT fallback
//!
//! This module provides a client that uses Ribbit as the primary protocol
//! and automatically falls back to TACT if Ribbit fails. Both protocols
//! return identical BPSV data, ensuring seamless operation.

use crate::fallback_client::FallbackClient;
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

/// Create a client with automatic Ribbit->TACT fallback and optional caching
///
/// If caching is enabled (default), both the Ribbit and TACT clients will
/// transparently cache all requests. The client will try Ribbit first (as
/// it's the primary protocol) and fall back to TACT on failure.
pub async fn create_client(region: Region) -> Result<FallbackClient, Box<dyn std::error::Error>> {
    let mut client = FallbackClient::new(region).await?;

    // If caching is disabled globally, disable it on the client
    if !is_caching_enabled() {
        client.set_caching_enabled(false);
    }

    Ok(client)
}

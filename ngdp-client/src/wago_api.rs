//! Wago Tools API client for retrieving build history

use chrono::{DateTime, Utc};
use ngdp_cache::generic::GenericCache;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Base URL for Wago Tools API
const WAGO_API_BASE: &str = "https://wago.tools/api";

/// Cache TTL for Wago builds API (30 minutes)
const WAGO_CACHE_TTL_SECS: u64 = 30 * 60;

/// Build information from Wago Tools API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WagoBuild {
    /// Product identifier (e.g., "wow", "wowt", "wowxptr")
    pub product: String,

    /// Build version string (e.g., "11.0.5.57212")
    pub version: String,

    /// Timestamp when the build was created
    pub created_at: String,

    /// Build configuration hash
    pub build_config: String,

    /// Product configuration (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_config: Option<String>,

    /// CDN configuration hash (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdn_config: Option<String>,

    /// Whether this is a background download build
    pub is_bgdl: bool,
}

/// Response from the builds API endpoint
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WagoBuildsResponse {
    /// Response is a map of product names to build arrays
    Map(std::collections::HashMap<String, Vec<WagoBuild>>),
    /// Response is a flat array of builds
    Array(Vec<WagoBuild>),
}

/// Fetch build history from Wago Tools API (uncached)
async fn fetch_builds_uncached() -> Result<WagoBuildsResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!("{WAGO_API_BASE}/builds");

    let response = client
        .get(&url)
        .header("User-Agent", "ngdp-client")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Wago API returned status: {}", response.status()).into());
    }

    let builds = response.json::<WagoBuildsResponse>().await?;
    Ok(builds)
}

/// Fetch build history from Wago Tools API with caching
pub async fn fetch_builds() -> Result<WagoBuildsResponse, Box<dyn std::error::Error>> {
    // Check if caching is disabled globally
    let cache_enabled = crate::cached_client::is_caching_enabled();

    if !cache_enabled {
        return fetch_builds_uncached().await;
    }

    // Initialize cache
    let cache = match GenericCache::with_subdirectory("wago").await {
        Ok(cache) => cache,
        Err(_) => {
            // If cache initialization fails, fall back to uncached
            return fetch_builds_uncached().await;
        }
    };

    let cache_key = "builds.json";
    let meta_key = "builds.meta";

    // Check if cached data exists and is valid
    let cache_path = cache.get_path(cache_key);
    let meta_path = cache.get_path(meta_key);

    // Check cache validity
    if let Ok(metadata_content) = tokio::fs::read_to_string(&meta_path).await {
        if let Ok(timestamp) = metadata_content.trim().parse::<u64>() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now < timestamp + WAGO_CACHE_TTL_SECS {
                // Cache is still valid, try to read it
                if let Ok(cached_data) = tokio::fs::read(&cache_path).await {
                    if let Ok(builds) = serde_json::from_slice(&cached_data) {
                        tracing::debug!("Using cached Wago builds data");
                        return Ok(builds);
                    }
                }
            }
        }
    }

    // Cache miss or invalid, fetch fresh data
    tracing::debug!("Fetching fresh Wago builds data");
    let builds = fetch_builds_uncached().await?;

    // Cache the response
    if let Ok(json_data) = serde_json::to_vec(&builds) {
        // Write cache data
        let _ = cache.write(cache_key, &json_data).await;

        // Write metadata (timestamp)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = cache
            .write(meta_key, timestamp.to_string().as_bytes())
            .await;
    }

    Ok(builds)
}

/// Filter builds by product name
pub fn filter_builds_by_product(builds: WagoBuildsResponse, product: &str) -> Vec<WagoBuild> {
    match builds {
        WagoBuildsResponse::Map(map) => map.get(product).cloned().unwrap_or_default(),
        WagoBuildsResponse::Array(builds) => builds
            .into_iter()
            .filter(|b| b.product == product)
            .collect(),
    }
}

/// Parse a date string from Wago API format to DateTime
pub fn parse_wago_date(date_str: &str) -> Option<DateTime<Utc>> {
    // Wago uses format: "2025-07-14 22:25:16"
    DateTime::parse_from_str(&format!("{date_str} +00:00"), "%Y-%m-%d %H:%M:%S %z")
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

//! CDN configuration support
//!
//! This module provides functionality for configuring CDN clients based on
//! application settings, including custom CDN fallback support.

use ngdp_cdn::CdnClientWithFallback;
use std::error::Error;

/// Create a CDN client with configured fallbacks
pub async fn create_cdn_client_with_config(
    primary_cdns: Vec<String>,
) -> Result<CdnClientWithFallback, Box<dyn Error>> {
    let mut builder = CdnClientWithFallback::builder();

    // Add primary CDNs (Blizzard servers)
    builder = builder.add_primary_cdns(primary_cdns);

    // Check if community CDN fallbacks are enabled
    let use_community_cdns = get_config_bool("use_community_cdn_fallbacks").unwrap_or(true);
    builder = builder.use_default_backups(use_community_cdns);

    // Add custom CDN fallbacks from configuration
    if let Some(custom_cdns_str) = get_config_string("custom_cdn_fallbacks") {
        if !custom_cdns_str.is_empty() {
            let custom_cdns: Vec<String> = custom_cdns_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            builder = builder.add_custom_cdns(custom_cdns);
        }
    }

    Ok(builder.build()?)
}

/// Get a boolean configuration value
fn get_config_bool(key: &str) -> Option<bool> {
    // TODO: Replace with actual config loading
    match key {
        "use_community_cdn_fallbacks" => Some(true),
        _ => None,
    }
}

/// Get a string configuration value
fn get_config_string(key: &str) -> Option<String> {
    // TODO: Replace with actual config loading
    match key {
        "custom_cdn_fallbacks" => Some(String::new()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_cdn_client_with_config() {
        let primary_cdns = vec![
            "blzddist1-a.akamaihd.net".to_string(),
            "level3.blizzard.com".to_string(),
        ];

        let client = create_cdn_client_with_config(primary_cdns).await.unwrap();
        let hosts = client.get_all_cdn_hosts();

        // Should include primary CDNs
        assert!(hosts.contains(&"blzddist1-a.akamaihd.net".to_string()));
        assert!(hosts.contains(&"level3.blizzard.com".to_string()));

        // Should include community CDNs by default
        assert!(hosts.contains(&"cdn.arctium.tools".to_string()));
        assert!(hosts.contains(&"tact.mirror.reliquaryhq.com".to_string()));
    }
}

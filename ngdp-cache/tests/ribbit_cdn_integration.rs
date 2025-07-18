//! Integration test that combines CachedRibbitClient and CachedCdnClient
//!
//! This test demonstrates real-world usage by:
//! 1. Fetching version information from Ribbit
//! 2. Getting CDN server lists
//! 3. Attempting to download actual files from CDNs with fallback

use ngdp_cache::{cached_cdn_client::CachedCdnClient, cached_ribbit_client::CachedRibbitClient};
use ribbit_client::{Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region};
use std::time::Duration;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

/// Products to test
const PRODUCTS: &[&str] = &["agent", "wow", "wow_classic", "wow_classic_era"];

/// Helper to parse version response using typed response
fn get_typed_versions(
    response: &ribbit_client::Response,
) -> Result<ProductVersionsResponse, Box<dyn std::error::Error>> {
    use ribbit_client::TypedResponse;
    Ok(ProductVersionsResponse::from_response(response)?)
}

/// Helper to parse CDN response using typed response
fn get_typed_cdns(
    response: &ribbit_client::Response,
) -> Result<ProductCdnsResponse, Box<dyn std::error::Error>> {
    use ribbit_client::TypedResponse;
    Ok(ProductCdnsResponse::from_response(response)?)
}

/// Try to download a file from multiple CDN hosts
async fn download_with_fallback(
    cdn_client: &CachedCdnClient,
    hosts: &[&str],
    path: &str,
    hash: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut last_error = None;

    for (i, host) in hosts.iter().enumerate() {
        info!(
            "Attempting download from CDN {} ({}/{})",
            host,
            i + 1,
            hosts.len()
        );

        match cdn_client.download(host, path, hash, "").await {
            Ok(response) => {
                let is_cached = response.is_from_cache();
                let data = response.bytes().await?;
                info!(
                    "Successfully downloaded {} bytes from {} (cached: {})",
                    data.len(),
                    host,
                    is_cached
                );
                return Ok(data.to_vec());
            }
            Err(e) => {
                warn!("Failed to download from {}: {}", host, e);
                last_error = Some(e);

                // Add a small delay before trying next host
                if i < hosts.len() - 1 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    Err(format!(
        "Failed to download from all {} CDN hosts. Last error: {:?}",
        hosts.len(),
        last_error
    )
    .into())
}

#[tokio::test]
#[ignore] // This test requires internet connection and live Blizzard servers
async fn test_ribbit_cdn_integration() {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_test_writer()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    // Create cached clients
    let ribbit_client = CachedRibbitClient::new(Region::US)
        .await
        .expect("Failed to create Ribbit client");
    let cdn_client = CachedCdnClient::new()
        .await
        .expect("Failed to create CDN client");

    // Test each product
    for product in PRODUCTS {
        info!("\n=== Testing product: {} ===", product);

        // Get versions
        let versions_endpoint = Endpoint::ProductVersions(product.to_string());
        match ribbit_client.request(&versions_endpoint).await {
            Ok(response) => {
                let _text = match &response.data {
                    Some(data) => data.clone(),
                    None => {
                        error!("No data in versions response for {}", product);
                        continue;
                    }
                };
                match get_typed_versions(&response) {
                    Ok(versions) => {
                        info!(
                            "Found {} version entries for {}",
                            versions.entries.len(),
                            product
                        );

                        // Get CDNs
                        let cdns_endpoint = Endpoint::ProductCdns(product.to_string());
                        match ribbit_client.request(&cdns_endpoint).await {
                            Ok(cdn_response) => {
                                let _cdn_text = match &cdn_response.data {
                                    Some(data) => data.clone(),
                                    None => {
                                        error!("No data in CDN response for {}", product);
                                        continue;
                                    }
                                };
                                match get_typed_cdns(&cdn_response) {
                                    Ok(cdns) => {
                                        info!(
                                            "Found {} CDN entries for {}",
                                            cdns.entries.len(),
                                            product
                                        );

                                        // Try to download BuildConfig from first version
                                        if let Some(version) = versions.entries.first() {
                                            if let Some(cdn) = cdns.entries.first() {
                                                let hosts: Vec<&str> =
                                                    cdn.hosts.iter().map(|s| s.as_str()).collect();
                                                info!(
                                                    "Using {} CDN hosts for {}",
                                                    hosts.len(),
                                                    product
                                                );

                                                // Try to download BuildConfig
                                                let build_config_hash = &version.build_config;
                                                if !build_config_hash.is_empty()
                                                    && build_config_hash != "0"
                                                {
                                                    info!(
                                                        "Attempting to download BuildConfig: {}",
                                                        build_config_hash
                                                    );

                                                    match download_with_fallback(
                                                        &cdn_client,
                                                        &hosts,
                                                        &cdn.path,
                                                        build_config_hash,
                                                    )
                                                    .await
                                                    {
                                                        Ok(data) => {
                                                            info!(
                                                                "Successfully downloaded BuildConfig for {} ({} bytes)",
                                                                product,
                                                                data.len()
                                                            );
                                                        }
                                                        Err(e) => {
                                                            // This is expected for some products/configs
                                                            warn!(
                                                                "Could not download BuildConfig for {}: {}",
                                                                product, e
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => error!("Failed to parse CDNs for {}: {}", product, e),
                                }
                            }
                            Err(e) => error!("Failed to get CDNs for {}: {}", product, e),
                        }
                    }
                    Err(e) => error!("Failed to parse versions for {}: {}", product, e),
                }
            }
            Err(e) => error!("Failed to get versions for {}: {}", product, e),
        }
    }

    // Check cache statistics
    match cdn_client.cache_stats().await {
        Ok(stats) => {
            info!("\n=== Cache Statistics ===");
            info!("Total cached files: {}", stats.total_files);
            info!("Total cache size: {}", stats.total_size_human());
            info!(
                "Config files: {} ({})",
                stats.config_files,
                stats.config_size_human()
            );
        }
        Err(e) => warn!("Failed to get cache stats: {}", e),
    }
}

#[tokio::test]
async fn test_mock_ribbit_cdn_integration() {
    // This test uses mock data to ensure the integration logic works
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_test_writer()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    // Create clients
    let cdn_client = CachedCdnClient::new()
        .await
        .expect("Failed to create CDN client");

    // Mock CDN hosts (these won't actually work)
    let mock_hosts = vec!["cdn1.example.com", "cdn2.example.com", "cdn3.example.com"];
    let mock_path = "tpr/configs/data";
    let mock_hash = "1234567890abcdef1234567890abcdef";

    // Test fallback behavior
    match download_with_fallback(&cdn_client, &mock_hosts, mock_path, mock_hash).await {
        Ok(_) => panic!("Should not succeed with mock hosts"),
        Err(e) => {
            info!("Expected failure with mock hosts: {}", e);
            assert!(e.to_string().contains("Failed to download from all"));
        }
    }
}

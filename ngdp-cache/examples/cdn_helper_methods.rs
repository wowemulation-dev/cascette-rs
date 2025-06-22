//! Example demonstrating the CDN helper methods
//!
//! This example shows how to use the convenience methods for downloading
//! different types of content from the CDN.

use ngdp_cache::cached_cdn_client::CachedCdnClient;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== CDN Helper Methods Example ===\n");

    // Create cached CDN client
    let client = CachedCdnClient::new().await?;

    // Example CDN configuration (from a typical CDN response)
    let cdn_host = "blzddist1-a.akamaihd.net";
    let path = "tpr/wow"; // Regular path for data/config files
    let config_path = "tpr/configs/data"; // Special path for ProductConfig

    // Example hashes (these are just examples, may not exist)
    let build_config_hash = "e359107662e72559b4e1ab721b157cb0";
    let cdn_config_hash = "48c7c7dfe4ea7df9dac22f6937ecbf47";
    let product_config_hash = "53020d32e1a25648c8e1eafd5771935f";
    let key_ring_hash = "3ca57fe7319a297346440e4d2a03a0cd";
    let data_hash = "1234567890abcdef1234567890abcdef";
    let patch_hash = "fedcba9876543210fedcba9876543210";

    info!("Demonstrating different download methods:\n");

    // 1. Download BuildConfig
    info!("1. BuildConfig (stored at {}/config):", path);
    match client
        .download_build_config(cdn_host, path, build_config_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            info!(
                "   ✓ Downloaded {} bytes (cached: {})",
                data.len(),
                is_cached
            );
        }
        Err(e) => {
            info!("   ✗ Failed: {}", e);
        }
    }

    // 2. Download CDNConfig
    info!("\n2. CDNConfig (stored at {}/config):", path);
    match client
        .download_cdn_config(cdn_host, path, cdn_config_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            info!(
                "   ✓ Downloaded {} bytes (cached: {})",
                data.len(),
                is_cached
            );
        }
        Err(e) => {
            info!("   ✗ Failed: {}", e);
        }
    }

    // 3. Download ProductConfig
    info!("\n3. ProductConfig (stored at {}):", config_path);
    match client
        .download_product_config(cdn_host, config_path, product_config_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            info!(
                "   ✓ Downloaded {} bytes (cached: {})",
                data.len(),
                is_cached
            );
        }
        Err(e) => {
            info!("   ✗ Failed: {}", e);
        }
    }

    // 4. Download KeyRing
    info!("\n4. KeyRing (stored at {}/config):", path);
    match client
        .download_key_ring(cdn_host, path, key_ring_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            info!(
                "   ✓ Downloaded {} bytes (cached: {})",
                data.len(),
                is_cached
            );
        }
        Err(e) => {
            info!("   ✗ Failed: {}", e);
        }
    }

    // 5. Download Data file
    info!("\n5. Data file (stored at {}/data):", path);
    match client.download_data(cdn_host, path, data_hash).await {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            info!(
                "   ✓ Downloaded {} bytes (cached: {})",
                data.len(),
                is_cached
            );
        }
        Err(e) => {
            info!("   ✗ Failed: {} (example hash may not exist)", e);
        }
    }

    // 6. Download Patch file
    info!("\n6. Patch file (stored at {}/patch):", path);
    match client.download_patch(cdn_host, path, patch_hash).await {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            info!(
                "   ✓ Downloaded {} bytes (cached: {})",
                data.len(),
                is_cached
            );
        }
        Err(e) => {
            info!("   ✗ Failed: {} (example hash may not exist)", e);
        }
    }

    info!("\n✅ Helper methods make it easy to download the right content type!");
    info!("   No need to manually construct paths anymore!");

    Ok(())
}

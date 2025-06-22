//! Example demonstrating the CDN cache directory structure
//!
//! This example shows how cached files are organized based on CDN paths,
//! preserving the CDN's path structure in the local cache.

use ngdp_cache::{cached_cdn_client::CachedCdnClient, cached_ribbit_client::CachedRibbitClient};
use ribbit_client::{Endpoint, ProductCdnsResponse, Region, TypedResponse};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== CDN Cache Structure Example ===\n");

    // Create clients
    let ribbit_client = CachedRibbitClient::new(Region::US).await?;
    let cdn_client = CachedCdnClient::new().await?;

    // Get CDN information for WoW
    info!("Fetching CDN information for WoW...");
    let cdns_endpoint = Endpoint::ProductCdns("wow".to_string());
    let cdn_response = ribbit_client.request(&cdns_endpoint).await?;
    let cdns = ProductCdnsResponse::from_response(&cdn_response)?;

    if let Some(cdn) = cdns.entries.first() {
        info!("\nCDN Configuration:");
        info!("  Name: {}", cdn.name);
        info!("  Data path: {}", cdn.path);
        info!("  Config path: {}", cdn.config_path);
        info!(
            "  First host: {}",
            cdn.hosts.first().unwrap_or(&String::new())
        );

        // Example hashes (these would normally come from version data)
        let config_hash = "e359107662e72559b4e1ab721b157cb0";
        let data_hash = "1234567890abcdef1234567890abcdef";

        info!("\nCache directory structure:");
        let cache_base = cdn_client.cache_dir();
        info!("  Base: {:?}", cache_base);

        // Show where different config types will be cached
        info!(
            "\n  BuildConfig/CDNConfig/KeyRing (using path/config: {}/config):",
            cdn.path
        );
        let build_config_cache_path = cache_base
            .join(&cdn.path)
            .join("config")
            .join(&config_hash[..2])
            .join(&config_hash[2..4])
            .join(config_hash);
        info!("    Example: {:?}", build_config_cache_path);

        info!(
            "\n  ProductConfig (using config_path: {}):",
            cdn.config_path
        );
        let product_config_cache_path = cache_base
            .join(&cdn.config_path)
            .join(&config_hash[..2])
            .join(&config_hash[2..4])
            .join(config_hash);
        info!("    Example: {:?}", product_config_cache_path);

        // Show where data files will be cached
        info!("\n  Data files (using path: {}):", cdn.path);
        let data_cache_path = cache_base
            .join(&cdn.path)
            .join("data")
            .join(&data_hash[..2])
            .join(&data_hash[2..4])
            .join(data_hash);
        info!("    Example: {:?}", data_cache_path);

        // Actually download a config to demonstrate
        if let Some(host) = cdn.hosts.first() {
            info!("\nDownloading a config file to demonstrate caching...");

            // Try to download a config file
            // Try to download a BuildConfig file (uses path/config)
            match cdn_client
                .download(host, &format!("{}/config", cdn.path), config_hash)
                .await
            {
                Ok(response) => {
                    let is_cached = response.is_from_cache();
                    let data = response.bytes().await?;
                    info!(
                        "  Downloaded {} bytes (from cache: {})",
                        data.len(),
                        is_cached
                    );

                    // Verify the file exists in the expected location
                    let expected_path = cache_base
                        .join(&cdn.path)
                        .join("config")
                        .join(&config_hash[..2])
                        .join(&config_hash[2..4])
                        .join(config_hash);

                    if expected_path.exists() {
                        info!("  ✓ File cached at expected location!");
                    } else {
                        info!("  ✗ File not found at expected location");
                    }
                }
                Err(e) => {
                    info!(
                        "  Failed to download: {} (this is normal for example hashes)",
                        e
                    );
                }
            }
        }
    } else {
        info!("No CDN entries found for WoW");
    }

    info!("\n✅ Cache structure example completed!");
    info!("   The cache preserves CDN paths for better organization:");
    info!("   - BuildConfig/CDNConfig/KeyRing: {{path}}/config");
    info!("   - ProductConfig: {{config_path}}");
    info!("   - Data/patch files: {{path}}/data or {{path}}/patch");

    Ok(())
}

//! Example of using CachedRibbitClient and CachedCdnClient together
//!
//! This example demonstrates real-world usage:
//! 1. Fetching version information from Ribbit for multiple products
//! 2. Getting CDN server lists
//! 3. Downloading config files from CDNs with automatic fallback
//! 4. All responses are cached for improved performance

use ngdp_cache::{cached_cdn_client::CachedCdnClient, cached_ribbit_client::CachedRibbitClient};
use ngdp_cdn::CdnClientTrait;
use ribbit_client::{
    Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region, TypedResponse,
};
use std::time::{Duration, Instant};
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

/// Products to demonstrate
const PRODUCTS: &[&str] = &["wow", "wow_classic"];

/// Try downloading BuildConfig from multiple CDN hosts with fallback
async fn download_build_config_with_fallback(
    cdn_client: &CachedCdnClient,
    hosts: &[String],
    path: &str,
    hash: &str,
    product: &str,
) -> Option<u64> {
    for (i, host) in hosts.iter().enumerate() {
        info!("  Trying CDN host {} of {}: {}", i + 1, hosts.len(), host);

        match cdn_client.download_build_config(host, path, hash).await {
            Ok(response) => {
                let is_cached = response.is_from_cache();
                let data = response.to_inner();
                let size = data.metadata().await.map(|m| m.len()).unwrap_or_default();
                info!(
                    "  ✓ Successfully downloaded {} bytes from {} (cached: {})",
                    size, host, is_cached
                );
                return Some(size);
            }
            Err(e) => {
                warn!("  ✗ Failed to download from {}: {}", host, e);
            }
        }

        // Small delay before trying next host
        if i < hosts.len() - 1 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    error!(
        "  Failed to download {} from all {} CDN hosts",
        product,
        hosts.len()
    );
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== Ribbit + CDN Download Example ===\n");

    // Create cached clients
    info!("Creating cached clients...");
    let ribbit_client = CachedRibbitClient::new(Region::US).await?;
    let cdn_client = CachedCdnClient::new().await?;

    // Track download statistics
    let mut total_downloads = 0;
    let mut successful_downloads = 0;
    let mut total_bytes = 0;

    // Process each product
    for product in PRODUCTS {
        info!("\n📦 Processing product: {}", product);
        let start = Instant::now();

        // Step 1: Get version information using typed response
        info!("  Fetching version information...");
        let versions_endpoint = Endpoint::ProductVersions(product.to_string());

        match ribbit_client.request(&versions_endpoint).await {
            Ok(response) => {
                match ProductVersionsResponse::from_response(&response) {
                    Ok(versions) => {
                        info!("  Found {} version entries", versions.entries.len());

                        // Step 2: Get CDN information using typed response
                        info!("  Fetching CDN information...");
                        let cdns_endpoint = Endpoint::ProductCdns(product.to_string());

                        match ribbit_client.request(&cdns_endpoint).await {
                            Ok(cdn_response) => {
                                match ProductCdnsResponse::from_response(&cdn_response) {
                                    Ok(cdns) => {
                                        info!("  Found {} CDN configurations", cdns.entries.len());

                                        // Step 3: Try to download build configs
                                        // We'll try the first few versions as an example
                                        let versions_to_try: Vec<_> = versions
                                            .entries
                                            .iter()
                                            .filter(|v| v.region == "us" || v.region == "eu")
                                            .take(2)
                                            .collect();

                                        if let Some(cdn) = cdns.entries.first() {
                                            info!("  Using CDN path: {}", cdn.path);
                                            info!("  Using config path: {}", cdn.config_path);
                                            info!("  Available hosts: {}", cdn.hosts.len());

                                            for (i, version) in versions_to_try.iter().enumerate() {
                                                if !version.build_config.is_empty()
                                                    && version.build_config != "0"
                                                {
                                                    info!(
                                                        "\n  📄 Downloading BuildConfig {} ({}/{})",
                                                        i + 1,
                                                        i + 1,
                                                        versions_to_try.len()
                                                    );
                                                    info!("  Region: {}", version.region);
                                                    info!("  Version: {}", version.versions_name);
                                                    info!("  Hash: {}", version.build_config);

                                                    total_downloads += 1;

                                                    if let Some(size) =
                                                        download_build_config_with_fallback(
                                                            &cdn_client,
                                                            &cdn.hosts,
                                                            &cdn.path,
                                                            &version.build_config,
                                                            product,
                                                        )
                                                        .await
                                                    {
                                                        successful_downloads += 1;
                                                        total_bytes += size;
                                                    }
                                                }
                                            }
                                        } else {
                                            warn!("  No CDN entries found for {}", product);
                                        }
                                    }
                                    Err(e) => {
                                        error!("  Failed to parse CDN response: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("  Failed to fetch CDN information: {}", e);
                            }
                        }

                        let elapsed = start.elapsed();
                        info!("\n  Product {} completed in {:.2?}", product, elapsed);
                    }
                    Err(e) => {
                        error!("  Failed to parse version response: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("  Failed to fetch version information: {}", e);
            }
        }
    }

    // Display cache statistics
    info!("\n📊 Cache Statistics:");
    match cdn_client.cache_stats().await {
        Ok(stats) => {
            info!("  Total cached files: {}", stats.total_files);
            info!("  Total cache size: {}", stats.total_size_human());
            info!(
                "  - Config files: {} ({})",
                stats.config_files,
                stats.config_size_human()
            );
            info!(
                "  - Data files: {} ({})",
                stats.data_files,
                stats.data_size_human()
            );
            info!(
                "  - Patch files: {} ({})",
                stats.patch_files,
                stats.patch_size_human()
            );
        }
        Err(e) => {
            warn!("  Failed to get cache statistics: {}", e);
        }
    }

    // Summary
    info!("\n📈 Download Summary:");
    info!("  Total download attempts: {}", total_downloads);
    info!("  Successful downloads: {}", successful_downloads);
    info!("  Total bytes downloaded: {} bytes", total_bytes);
    info!(
        "  Success rate: {:.1}%",
        if total_downloads > 0 {
            (successful_downloads as f64 / total_downloads as f64) * 100.0
        } else {
            0.0
        }
    );

    info!("\n✅ Example completed!");
    info!("   Run again to see cache performance improvements!");

    Ok(())
}

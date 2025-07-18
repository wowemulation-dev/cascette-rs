//! Basic usage example for ngdp-cdn

use ngdp_cdn::{build_path, CdnClient, DummyCacheProvider, StaticHostList};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("NGDP CDN Client Example");
    info!("======================");

    // Example CDN hosts (these would typically come from TACT CDN manifest)
    let cdn_hosts = StaticHostList(vec![
        "blzddist1-a.akamaihd.net".to_string(),
        "level3.blizzard.com".to_string(),
        "cdn.blizzard.com".to_string(),
    ]);

    // Create a CDN client with default configuration
    let client: CdnClient<StaticHostList, DummyCacheProvider> = CdnClient::builder().hosts(cdn_hosts.clone()).build()?;
    info!("Created CDN client with default configuration");

    // Example content hash (this would come from TACT manifests)
    let example_hash = "2e9c1e3b5f5a0c9d9e8f1234567890ab";
    let path = "tpr/wow";

    info!("Attempting to download content:");
    info!("  Hash: {}", example_hash);
    info!("  Path: {}", path);
    info!("  CDN hosts: {:?}", client.hosts());

    // Try each CDN host until one succeeds
    let mut success = false;
    match client.download(path, example_hash, "").await {
        Ok(response) => {
            let content_length = response.content_length().unwrap_or(0);
            info!("✓ Success! Content length: {} bytes", content_length);

            // In a real application, you would process the content
            let bytes = response.bytes().await?;
            info!("Downloaded {} bytes", bytes.len());

            success = true;
        }
        Err(e) => {
            error!("✗ Failed: {}", e);
        }
    }

    if !success {
        error!("Failed to download from any CDN host");
    }

    // Example with custom configuration
    info!("\nCreating client with custom configuration...");
    let _custom_client: CdnClient<StaticHostList, DummyCacheProvider> = CdnClient::builder()
        .max_retries(5)
        .initial_backoff_ms(200)
        .max_backoff_ms(30_000)
        .connect_timeout(60)
        .request_timeout(300)
        .pool_max_idle_per_host(50)
        .hosts(cdn_hosts)
        .build()?;

    info!("Custom client configuration:");
    info!("  Max retries: 5");
    info!("  Initial backoff: 200ms");
    info!("  Max backoff: 30s");
    info!("  Connect timeout: 60s");
    info!("  Request timeout: 300s");
    info!("  Max idle connections per host: 50");

    // Example of building URLs manually
    let url = build_path(path, example_hash, "")?;
    info!("\nBuilding CDN URLs manually: {url}");

    Ok(())
}

//! Basic usage example for ngdp-cdn

use ngdp_cdn::CdnClient;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("NGDP CDN Client Example");
    info!("======================");

    // Create a CDN client with default configuration
    let client = CdnClient::new()?;
    info!("Created CDN client with default configuration");

    // Example CDN hosts (these would typically come from TACT CDN manifest)
    let cdn_hosts = vec![
        "blzddist1-a.akamaihd.net",
        "level3.blizzard.com",
        "cdn.blizzard.com",
    ];

    // Example content hash (this would come from TACT manifests)
    let example_hash = "2e9c1e3b5f5a0c9d9e8f1234567890ab";
    let path = "tpr/wow";

    info!("Attempting to download content:");
    info!("  Hash: {}", example_hash);
    info!("  Path: {}", path);
    info!("  CDN hosts: {:?}", cdn_hosts);

    // Try each CDN host until one succeeds
    let mut success = false;
    for cdn_host in &cdn_hosts {
        info!("Trying CDN host: {}", cdn_host);

        match client.download(cdn_host, path, example_hash).await {
            Ok(response) => {
                let content_length = response.content_length().unwrap_or(0);
                info!("✓ Success! Content length: {} bytes", content_length);

                // In a real application, you would process the content
                let bytes = response.bytes().await?;
                info!("Downloaded {} bytes", bytes.len());

                success = true;
                break;
            }
            Err(e) => {
                error!("✗ Failed: {}", e);
                continue;
            }
        }
    }

    if !success {
        error!("Failed to download from any CDN host");
    }

    // Example with custom configuration
    info!("\nCreating client with custom configuration...");
    let _custom_client = CdnClient::builder()
        .max_retries(5)
        .initial_backoff_ms(200)
        .max_backoff_ms(30_000)
        .connect_timeout(60)
        .request_timeout(300)
        .pool_max_idle_per_host(50)
        .build()?;

    info!("Custom client configuration:");
    info!("  Max retries: 5");
    info!("  Initial backoff: 200ms");
    info!("  Max backoff: 30s");
    info!("  Connect timeout: 60s");
    info!("  Request timeout: 300s");
    info!("  Max idle connections per host: 50");

    // Example of building URLs manually
    info!("\nBuilding CDN URLs manually:");
    for cdn_host in &cdn_hosts[..2] {
        let url = CdnClient::build_url(cdn_host, path, example_hash)?;
        info!("  {}: {}", cdn_host, url);
    }

    Ok(())
}

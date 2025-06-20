//! Example demonstrating retry functionality in TACT HTTP client

use tact_client::{HttpClient, ProtocolVersion, Region};
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("TACT HTTP Client Retry Example");
    info!("==============================");

    // Create a client with aggressive retry settings for demonstration
    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?
        .with_max_retries(3)
        .with_initial_backoff_ms(500)
        .with_max_backoff_ms(5000)
        .with_backoff_multiplier(1.5)
        .with_jitter_factor(0.2);

    info!("Configuration:");
    info!("- Max retries: 3");
    info!("- Initial backoff: 500ms");
    info!("- Max backoff: 5000ms");
    info!("- Backoff multiplier: 1.5x");
    info!("- Jitter factor: 0.2 (20%)");
    info!("");

    // Test with a real product that should succeed
    info!("Testing with wow product (should succeed)...");
    match client.get_versions_parsed("wow").await {
        Ok(versions) => {
            info!("✓ Successfully retrieved {} version entries", versions.len());
            if let Some(first) = versions.first() {
                debug!("First entry: Region={:?}", first.region);
            }
        }
        Err(e) => {
            info!("✗ Failed: {}", e);
        }
    }

    info!("");

    // Test with a non-existent product to see error handling
    info!("Testing with non-existent product (should fail after retries)...");
    match client.get_versions_parsed("nonexistent_product_xyz").await {
        Ok(_) => {
            info!("✓ Unexpectedly succeeded");
        }
        Err(e) => {
            info!("✗ Failed as expected: {}", e);
        }
    }

    info!("");

    // Test CDN endpoint
    info!("Testing CDN endpoint...");
    match client.get_cdns_parsed("wow").await {
        Ok(cdns) => {
            info!("✓ Successfully retrieved {} CDN entries", cdns.len());
            if let Some(first) = cdns.first() {
                debug!("First CDN: Name={}, Hosts={}", first.name, first.hosts.len());
            }
        }
        Err(e) => {
            info!("✗ Failed: {}", e);
        }
    }

    info!("");

    // Demonstrate different retry strategies
    info!("Retry Strategy Examples:");
    info!("------------------------");

    // Conservative strategy
    let _conservative = HttpClient::new(Region::EU, ProtocolVersion::V1)?
        .with_max_retries(2)
        .with_initial_backoff_ms(1000)
        .with_backoff_multiplier(2.0);
    info!("Conservative: 2 retries, 1s initial backoff, 2x multiplier");

    // Aggressive strategy
    let _aggressive = HttpClient::new(Region::EU, ProtocolVersion::V1)?
        .with_max_retries(5)
        .with_initial_backoff_ms(100)
        .with_backoff_multiplier(1.5)
        .with_jitter_factor(0.3);
    info!("Aggressive: 5 retries, 100ms initial backoff, 1.5x multiplier, 30% jitter");

    // No retry (default)
    let _no_retry = HttpClient::new(Region::EU, ProtocolVersion::V1)?;
    info!("No retry: Default configuration (0 retries)");

    Ok(())
}
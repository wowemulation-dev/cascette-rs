//! Example demonstrating the automatic Ribbit to TACT fallback functionality

use ngdp_client::fallback_client::FallbackClient;
use ribbit_client::{Endpoint, ProductVersionsResponse, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Creating fallback client for US region...");
    let client = FallbackClient::new(Region::US).await?;

    // Test with product versions endpoint
    let endpoint = Endpoint::ProductVersions("wow".to_string());

    println!("\nRequesting WoW versions (will try Ribbit first, fall back to TACT if needed)...");
    match client.request(&endpoint).await {
        Ok(response) => {
            if let Some(data) = &response.data {
                println!("✓ Successfully retrieved data!");
                println!("  Data length: {} bytes", data.len());

                // Also try typed request
                match client
                    .request_typed::<ProductVersionsResponse>(&endpoint)
                    .await
                {
                    Ok(versions) => {
                        println!("✓ Parsed {} version entries", versions.entries.len());
                        for entry in versions.entries.iter().take(3) {
                            println!(
                                "  - {}: {} (build {})",
                                entry.region, entry.versions_name, entry.build_id
                            );
                        }
                    }
                    Err(e) => eprintln!("Failed to parse versions: {e}"),
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Both Ribbit and TACT failed: {e}");
        }
    }

    // Test caching control
    println!("\nDisabling caching...");
    let mut client = client;
    client.set_caching_enabled(false);

    println!("Making another request (bypassing cache)...");
    match client.request(&endpoint).await {
        Ok(_) => println!("✓ Request succeeded without cache"),
        Err(e) => eprintln!("✗ Request failed: {e}"),
    }

    // Clear expired cache entries
    println!("\nCleaning up expired cache entries...");
    if let Err(e) = client.clear_expired().await {
        eprintln!("Failed to clear expired entries: {e}");
    } else {
        println!("✓ Expired cache entries cleared");
    }

    Ok(())
}

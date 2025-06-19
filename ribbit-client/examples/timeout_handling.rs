//! Example demonstrating timeout handling for unreachable regions
//!
//! This example shows how the Ribbit client handles connection timeouts
//! when trying to connect to region-restricted servers like CN (China).

use ribbit_client::{Endpoint, Error, Region, RibbitClient};

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("Testing connection timeout handling...\n");

    // Test CN region (often unreachable from outside China)
    test_region(Region::CN).await;

    println!("\n---\n");

    // Test US region (should work globally)
    test_region(Region::US).await;
}

async fn test_region(region: Region) {
    println!(
        "Testing {} region ({})...",
        region.as_str(),
        region.hostname()
    );

    let client = RibbitClient::new(region);

    match client.request(&Endpoint::Summary).await {
        Ok(response) => {
            println!("✓ Successfully connected to {} region", region.as_str());
            if let Some(data) = response.as_text() {
                let lines: Vec<_> = data.lines().take(3).collect();
                println!("  First few lines of response:");
                for line in lines {
                    println!("    {}", line);
                }
            }
        }
        Err(Error::ConnectionTimeout {
            host,
            port,
            timeout_secs,
        }) => {
            println!("✗ Connection timed out after {}s", timeout_secs);
            println!("  Failed to connect to {}:{}", host, port);
            println!("  This server may be region-restricted or unavailable");
        }
        Err(Error::ConnectionFailed { host, port }) => {
            println!("✗ Connection failed");
            println!("  Failed to connect to {}:{}", host, port);
        }
        Err(e) => {
            println!("✗ Error: {}", e);
        }
    }
}

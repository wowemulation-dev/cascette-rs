//! Example demonstrating the cached Ribbit client
//!
//! This example shows how to use the CachedRibbitClient to cache certificate
//! endpoint requests using the Blizzard MIME filename convention.

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== Cached Ribbit Client Example ===\n");

    // Create a cached client
    let mut client = CachedRibbitClient::new(Region::US).await?;
    println!("Created cached Ribbit client for US region");

    // Test certificate endpoint caching
    let cert_hash = "5168ff90af0207753cccd9656462a212b859723b";
    let cert_endpoint = Endpoint::Cert(cert_hash.to_string());

    println!("\n1. First request (cache miss):");
    let start = std::time::Instant::now();
    let raw_response = client.request_raw(&cert_endpoint).await?;
    let duration = start.elapsed();
    println!(
        "   Received {} bytes in {:?} (fetched from server)",
        raw_response.len(),
        duration
    );

    // Check if it's a certificate
    let response_str = String::from_utf8_lossy(&raw_response);
    if response_str.contains("-----BEGIN CERTIFICATE-----") {
        println!("   ✓ Response contains a PEM certificate");
    }

    println!("\n2. Second request (cache hit):");
    let start = std::time::Instant::now();
    let cached_response = client.request_raw(&cert_endpoint).await?;
    let duration = start.elapsed();
    println!(
        "   Received {} bytes in {:?} (from cache)",
        cached_response.len(),
        duration
    );

    // Verify they match
    if raw_response == cached_response {
        println!("   ✓ Cached response matches original");
    }

    // Test with regular endpoint (shorter TTL)
    let versions_endpoint = Endpoint::ProductVersions("wow".to_string());

    println!("\n3. Testing regular endpoint caching:");
    let start = std::time::Instant::now();
    let raw_response = client.request_raw(&versions_endpoint).await?;
    let duration = start.elapsed();
    println!(
        "   Received {} bytes in {:?} (fetched from server)",
        raw_response.len(),
        duration
    );

    // Cache should work immediately
    let start = std::time::Instant::now();
    let cached_response = client.request_raw(&versions_endpoint).await?;
    let duration = start.elapsed();
    println!(
        "   Received {} bytes in {:?} (from cache)",
        cached_response.len(),
        duration
    );

    // Demonstrate cache control
    println!("\n4. Testing cache control:");
    client.set_caching_enabled(false);
    println!("   Disabled caching");

    let start = std::time::Instant::now();
    let _response = client.request_raw(&versions_endpoint).await?;
    let duration = start.elapsed();
    println!(
        "   Request completed in {:?} (caching disabled, always fetches)",
        duration
    );

    // Re-enable and test expiration cleanup
    client.set_caching_enabled(true);
    println!("\n5. Testing cache cleanup:");
    client.clear_expired().await?;
    println!("   Cleared expired entries");

    // Show cache directory structure
    let cache_dir = dirs::cache_dir()
        .unwrap()
        .join("ngdp")
        .join("ribbit")
        .join("us");

    if cache_dir.exists() {
        println!("\n6. Cache files created:");
        let mut entries = tokio::fs::read_dir(&cache_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(filename) = path.file_name() {
                println!("   - {}", filename.to_string_lossy());
            }
        }
    }

    Ok(())
}

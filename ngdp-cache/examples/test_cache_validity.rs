//! Test that cached data is still valid and being used

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cache Validity Test ===\n");

    // Test that cache is still valid after previous runs
    let client = CachedRibbitClient::new(Region::US).await?;

    // Certificate endpoint - should be cached from previous runs
    let cert_endpoint = Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string());

    println!("Testing certificate cache (30-day TTL):");
    let start = std::time::Instant::now();
    let data = client.request_raw(&cert_endpoint).await?;
    let duration = start.elapsed();

    println!("  Request completed in {:?}", duration);
    println!("  Response size: {} bytes", data.len());

    if duration.as_micros() < 1000 {
        // Less than 1ms
        println!("  ✓ Cache hit confirmed (sub-millisecond response)");
    } else {
        println!("  ✗ Likely cache miss (took more than 1ms)");
    }

    // Product versions - should also be cached if run within 5 minutes
    let versions_endpoint = Endpoint::ProductVersions("wow".to_string());

    println!("\nTesting product versions cache (5-minute TTL):");
    let start = std::time::Instant::now();
    let data = client.request_raw(&versions_endpoint).await?;
    let duration = start.elapsed();

    println!("  Request completed in {:?}", duration);
    println!("  Response size: {} bytes", data.len());

    if duration.as_micros() < 1000 {
        // Less than 1ms
        println!("  ✓ Cache hit confirmed (sub-millisecond response)");
    } else {
        println!("  ✗ Likely cache miss or expired (took more than 1ms)");
    }

    // Display cache file information
    println!("\nCache file locations:");
    let cache_dir = dirs::cache_dir().unwrap().join("ngdp/ribbit/cached/us");

    println!(
        "  Certificate: {}/certs-5168ff90af0207753cccd9656462a212b859723b-0.bmime",
        cache_dir.display()
    );
    println!("  Versions: {}/versions-wow-0.bmime", cache_dir.display());

    Ok(())
}

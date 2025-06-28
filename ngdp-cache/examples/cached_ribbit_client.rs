//! Example demonstrating the cached Ribbit client
//!
//! This example shows how to use the CachedRibbitClient to:
//! - Cache certificate endpoint requests using the Blizzard MIME filename convention
//! - Cache full Response objects, not just raw bytes
//! - Work as a drop-in replacement for RibbitClient
//! - Parse responses as typed data (BPSV format)

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, ProductVersionsResponse, Region, TypedResponse};

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
        "   Request completed in {duration:?} (caching disabled, always fetches)"
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

    // Test Response object caching (not just raw bytes)
    println!("\n7. Testing Response object caching:");
    let response = client.request(&versions_endpoint).await?;
    println!("   Response object cached with:");
    println!("     - Raw data: {} bytes", response.raw.len());
    println!("     - Has parsed data: {}", response.data.is_some());
    println!("     - Has MIME parts: {}", response.mime_parts.is_some());

    // Test typed response parsing
    println!("\n8. Testing typed response parsing:");
    if let Ok(bpsv) = response.as_bpsv() {
        println!("   ✓ Successfully parsed as BPSV");
        println!("   Sequence number: {:?}", bpsv.sequence_number());
        println!("   Row count: {}", bpsv.rows().len());

        // Parse as typed response
        let typed = ProductVersionsResponse::from_bpsv(&bpsv)?;
        println!("   Found {} version entries", typed.entries.len());
        if let Some(first) = typed.entries.first() {
            println!(
                "   First entry: {} - Build {}",
                first.region, first.build_id
            );
        }
    } else {
        println!("   Note: V1 responses need additional parsing");
    }

    // Test with V2 protocol for immediate parsed data
    println!("\n9. Testing with V2 protocol:");
    let mut v2_client = CachedRibbitClient::new(Region::US).await?;
    v2_client
        .inner_mut()
        .set_protocol_version(ribbit_client::ProtocolVersion::V2);

    let v2_response = v2_client.request(&versions_endpoint).await?;
    if let Some(data) = &v2_response.data {
        println!("   ✓ V2 response has parsed data immediately");
        println!(
            "   Data preview: {}...",
            &data.chars().take(50).collect::<String>()
        );
    }

    // Compare cache performance
    println!("\n10. Cache performance comparison:");
    println!("   Testing multiple rapid requests...");
    let mut total_cached = 0u128;
    let mut total_fresh = 0u128;

    // Cached requests
    for i in 0..5 {
        let start = std::time::Instant::now();
        let _ = client.request(&versions_endpoint).await?;
        let elapsed = start.elapsed().as_micros();
        total_cached += elapsed;
        println!("   Cached request {}: {:?}", i + 1, start.elapsed());
    }

    // Fresh requests (with caching disabled)
    client.set_caching_enabled(false);
    for i in 0..5 {
        let start = std::time::Instant::now();
        let _ = client.request(&versions_endpoint).await?;
        let elapsed = start.elapsed().as_micros();
        total_fresh += elapsed;
        println!("   Fresh request {}: {:?}", i + 1, start.elapsed());
    }

    println!("\n   Average cached: {} µs", total_cached / 5);
    println!("   Average fresh: {} µs", total_fresh / 5);
    if total_fresh > 0 {
        println!(
            "   Cache speedup: {:.1}x faster",
            total_fresh as f64 / total_cached as f64
        );
    }

    Ok(())
}

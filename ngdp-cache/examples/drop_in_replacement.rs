//! Example demonstrating CachedRibbitClient as a drop-in replacement for RibbitClient
//!
//! This example shows how CachedRibbitClient implements the same API as RibbitClient,
//! making it easy to add caching to existing code by just changing the client type.

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Drop-in Replacement Example ===\n");

    // Instead of: let client = RibbitClient::new(Region::US);
    // We use:     let client = CachedRibbitClient::new(Region::US).await?;
    let client = CachedRibbitClient::new(Region::US).await?;
    
    // All the same methods work!
    
    println!("1. Using convenience methods (with automatic caching):\n");
    
    // Get summary (first call hits network, subsequent calls use cache)
    let start = std::time::Instant::now();
    let summary = client.get_summary().await?;
    println!("   Summary fetched in {:?}", start.elapsed());
    println!("   Found {} products", summary.products.len());
    
    // Second call should be instant
    let start = std::time::Instant::now();
    let _summary2 = client.get_summary().await?;
    println!("   Second summary fetch in {:?} (cached!)", start.elapsed());
    
    // Get product versions
    println!("\n2. Product versions:");
    let versions = client.get_product_versions("wow").await?;
    println!("   WoW has {} version entries", versions.entries.len());
    if let Some(us_version) = versions.entries.iter().find(|e| e.region == "us") {
        println!("   US Build: {} ({})", us_version.build_id, us_version.versions_name);
    }
    
    // Get CDN info
    println!("\n3. CDN configuration:");
    let cdns = client.get_product_cdns("wow").await?;
    if let Some(us_cdn) = cdns.entries.iter().find(|e| e.name == "us") {
        println!("   US CDN hosts: {}", us_cdn.hosts.join(", "));
    }
    
    // Using typed requests directly
    println!("\n4. Using request_typed (also cached):");
    let bgdl = client.request_typed::<ribbit_client::ProductBgdlResponse>(
        &Endpoint::ProductBgdl("wow".to_string())
    ).await?;
    println!("   Background download entries: {}", bgdl.entries.len());
    
    // Raw requests still work
    println!("\n5. Raw requests (cached at byte level):");
    let raw = client.request_raw(&Endpoint::ProductVersions("d4".to_string())).await?;
    println!("   D4 versions raw response: {} bytes", raw.len());
    
    // Full Response objects
    println!("\n6. Full Response objects (with caching):");
    let response = client.request(&Endpoint::Summary).await?;
    println!("   Response has {} bytes raw data", response.raw.len());
    if let Some(data) = response.data {
        println!("   Parsed data: {} characters", data.len());
    }
    
    // Access to underlying client for configuration
    println!("\n7. Configuration access:");
    println!("   Current region: {:?}", client.inner().region());
    println!("   Protocol version: {:?}", client.inner().protocol_version());
    
    // Demonstrate cache control
    println!("\n8. Cache control:");
    println!("   Clearing expired entries...");
    client.clear_expired().await?;
    
    println!("\n✓ CachedRibbitClient provides the complete RibbitClient API with transparent caching!");
    
    Ok(())
}
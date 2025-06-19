//! Example demonstrating the cached request method that returns Response objects
//!
//! This example shows how CachedRibbitClient can cache full Response objects,
//! not just raw bytes, making it a drop-in replacement for RibbitClient.

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region, TypedResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== Cached Request Example ===\n");

    // Create a cached client
    let client = CachedRibbitClient::new(Region::US).await?;

    // Test with product versions endpoint
    let endpoint = Endpoint::ProductVersions("wow".to_string());

    println!("1. First request (cache miss):");
    let start = std::time::Instant::now();
    let response = client.request(&endpoint).await?;
    let duration = start.elapsed();

    println!("   Request completed in {:?}", duration);
    println!("   Raw data size: {} bytes", response.raw.len());

    // Check parsed data
    if let Some(data) = &response.data {
        println!("   Parsed data available: {} chars", data.len());

        // Try to parse as BPSV
        if let Ok(bpsv) = response.as_bpsv() {
            println!("   ✓ Successfully parsed as BPSV");
            println!("   Sequence number: {:?}", bpsv.sequence_number());
            println!("   Row count: {}", bpsv.rows().len());
        }
    } else {
        println!("   Note: V1 responses need additional parsing");
    }

    println!("\n2. Second request (cache hit):");
    let start = std::time::Instant::now();
    let cached_response = client.request(&endpoint).await?;
    let duration = start.elapsed();

    println!("   Request completed in {:?}", duration);
    println!("   Raw data size: {} bytes", cached_response.raw.len());

    // Verify they match
    if response.raw == cached_response.raw {
        println!("   ✓ Cached raw data matches original");
    }

    // Test with V2 protocol
    println!("\n3. Testing with V2 protocol:");
    let mut v2_client = CachedRibbitClient::new(Region::US).await?;
    v2_client
        .inner_mut()
        .set_protocol_version(ribbit_client::ProtocolVersion::V2);

    let start = std::time::Instant::now();
    let v2_response = v2_client.request(&endpoint).await?;
    let duration = start.elapsed();

    println!("   V2 request completed in {:?}", duration);
    if let Some(data) = &v2_response.data {
        println!("   ✓ V2 response has parsed data immediately");
        println!(
            "   Data preview: {}...",
            &data.chars().take(50).collect::<String>()
        );
    }

    // Compare protocols
    println!("\n4. Protocol comparison:");
    println!("   V1 Response:");
    println!("     - Has raw data: {}", response.raw.is_empty() == false);
    println!("     - Has parsed data: {}", response.data.is_some());
    println!("     - Has MIME parts: {}", response.mime_parts.is_some());

    println!("   V2 Response:");
    println!(
        "     - Has raw data: {}",
        v2_response.raw.is_empty() == false
    );
    println!("     - Has parsed data: {}", v2_response.data.is_some());
    println!(
        "     - Has MIME parts: {}",
        v2_response.mime_parts.is_some()
    );

    // Demonstrate typed response compatibility
    println!("\n5. Typed response compatibility:");
    use ribbit_client::ProductVersionsResponse;

    // This would work with either cached or uncached client
    let typed: ProductVersionsResponse = match response.as_bpsv() {
        Ok(bpsv) => ProductVersionsResponse::from_bpsv(&bpsv)?,
        Err(e) => {
            println!("   Could not parse typed response: {}", e);
            return Ok(());
        }
    };

    println!("   Found {} version entries", typed.entries.len());
    if let Some(first) = typed.entries.first() {
        println!(
            "   First entry: {} - Build {}",
            first.region, first.build_id
        );
    }

    Ok(())
}

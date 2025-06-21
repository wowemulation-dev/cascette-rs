//! Example demonstrating the cached TACT client
//!
//! This example shows how to use the CachedTactClient to cache TACT
//! protocol responses with automatic sequence number tracking.

use ngdp_cache::cached_tact_client::CachedTactClient;
use tact_client::{ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== Cached TACT Client Example ===\n");

    // Create a cached client for US region with V1 protocol
    let mut client = CachedTactClient::new(Region::US, ProtocolVersion::V1).await?;
    println!("Created cached TACT client for US region (protocol v1)");

    // Test versions endpoint caching
    println!("\n1. Testing versions endpoint:");
    let product = "wow";
    
    println!("   First request (cache miss):");
    let start = std::time::Instant::now();
    match client.get_versions_parsed(product).await {
        Ok(versions) => {
            let duration = start.elapsed();
            println!(
                "   ✓ Received {} versions in {:?} (fetched from server)",
                versions.len(),
                duration
            );
            if let Some(first) = versions.first() {
                println!(
                    "     Latest: {} (Build: {})",
                    first.versions_name, first.build_id
                );
            }
        }
        Err(e) => println!("   ✗ Failed to get versions: {}", e),
    }

    println!("\n   Second request (cache hit):");
    let start = std::time::Instant::now();
    match client.get_versions_parsed(product).await {
        Ok(versions) => {
            let duration = start.elapsed();
            println!(
                "   ✓ Received {} versions in {:?} (from cache)",
                versions.len(),
                duration
            );
        }
        Err(e) => println!("   ✗ Failed to get versions: {}", e),
    }

    // Test CDN endpoint caching
    println!("\n2. Testing CDN endpoint:");
    let start = std::time::Instant::now();
    match client.get_cdns_parsed(product).await {
        Ok(cdns) => {
            let duration = start.elapsed();
            println!(
                "   ✓ Received {} CDN configs in {:?}",
                cdns.len(),
                duration
            );
            if let Some(first) = cdns.first() {
                println!("     First CDN: {} (Path: {})", first.name, first.path);
            }
        }
        Err(e) => println!("   ✗ Failed to get CDNs: {}", e),
    }

    // Test with V2 protocol
    println!("\n3. Testing V2 protocol:");
    let client_v2 = CachedTactClient::new(Region::US, ProtocolVersion::V2).await?;
    match client_v2.get_versions_parsed(product).await {
        Ok(versions) => {
            println!("   ✓ V2 protocol: Found {} versions", versions.len());
        }
        Err(e) => println!("   ✗ V2 protocol failed: {}", e),
    }

    // Test different products
    println!("\n4. Testing different products:");
    for product in ["d3", "agent", "hero"] {
        match client.get_versions_parsed(product).await {
            Ok(versions) => {
                println!("   ✓ {}: {} versions", product, versions.len());
            }
            Err(e) => {
                println!("   ✗ {}: {}", product, e);
            }
        }
    }

    // Demonstrate cache control
    println!("\n5. Testing cache control:");
    client.set_caching_enabled(false);
    println!("   Disabled caching");

    let start = std::time::Instant::now();
    let _result = client.get_versions_parsed(product).await;
    let duration = start.elapsed();
    println!(
        "   Request completed in {:?} (caching disabled, always fetches)",
        duration
    );

    // Clear expired entries
    client.set_caching_enabled(true);
    println!("\n6. Cache maintenance:");
    client.clear_expired().await?;
    println!("   ✓ Cleared expired entries");

    // Show cache directory structure
    let cache_dir = dirs::cache_dir()
        .unwrap()
        .join("ngdp")
        .join("tact")
        .join("us")
        .join("v1");

    if cache_dir.exists() {
        println!("\n7. Cache files created:");
        if let Ok(mut products) = tokio::fs::read_dir(&cache_dir).await {
            while let Some(entry) = products.next_entry().await? {
                let product_name = entry.file_name();
                println!("   Product: {}", product_name.to_string_lossy());
                
                let product_dir = entry.path();
                if let Ok(mut files) = tokio::fs::read_dir(&product_dir).await {
                    while let Some(file) = files.next_entry().await? {
                        let filename = file.file_name();
                        println!("     - {}", filename.to_string_lossy());
                    }
                }
            }
        }
    }

    println!("\n✓ Example completed successfully!");
    Ok(())
}
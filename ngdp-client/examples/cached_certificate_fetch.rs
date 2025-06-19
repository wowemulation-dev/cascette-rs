//! Example showing how to use ngdp-cache with ngdp-client for certificate caching
//!
//! This demonstrates integrating the CachedRibbitClient into ngdp-client
//! workflows for caching certificate endpoint requests.

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};

/// Fetch a certificate using the cached client
async fn fetch_certificate_cached(hash: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Create cached client
    let client = CachedRibbitClient::new(Region::US).await?;
    
    // Request the certificate
    let endpoint = Endpoint::Cert(hash.to_string());
    let raw_data = client.request_raw(&endpoint).await?;
    
    // For certificates, the response is typically plain text PEM
    Ok(String::from_utf8(raw_data)?)
}

/// Example of how ngdp-client commands could use cached requests
async fn verify_signature_with_cache(ski: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Verifying signature with SKI: {}", ski);
    
    // In a real implementation, you would:
    // 1. Extract the certificate hash from a mapping of SKI to cert hash
    // 2. Use the cached client to fetch the certificate
    // 3. Verify the signature
    
    // For this example, we'll use a known certificate hash
    let cert_hash = "5168ff90af0207753cccd9656462a212b859723b";
    
    println!("Fetching certificate: {}", cert_hash);
    let cert_pem = fetch_certificate_cached(cert_hash).await?;
    
    if cert_pem.contains("-----BEGIN CERTIFICATE-----") {
        println!("✓ Successfully retrieved certificate (possibly from cache)");
        // Here you would parse the certificate and verify the signature
    } else {
        println!("✗ Response doesn't contain a valid certificate");
    }
    
    Ok(())
}

/// Demonstrate how to integrate caching into existing workflows
async fn cached_product_info(product: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = CachedRibbitClient::new(Region::US).await?;
    
    // Get product versions (with caching)
    let endpoint = Endpoint::ProductVersions(product.to_string());
    let raw_data = client.request_raw(&endpoint).await?;
    
    // Parse the response (in real code, you'd use proper parsing)
    let data = String::from_utf8_lossy(&raw_data);
    println!("Product versions for {}:", product);
    for line in data.lines().take(5) {
        println!("  {}", line);
    }
    println!("  ...");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== ngdp-client Certificate Caching Example ===\n");

    // Example 1: Certificate verification with caching
    println!("1. Certificate verification workflow:");
    verify_signature_with_cache("example-ski").await?;

    // Example 2: Product info with caching
    println!("\n2. Product information with caching:");
    cached_product_info("wow").await?;

    // Example 3: Demonstrating cache benefits
    println!("\n3. Performance comparison:");
    
    // Without cache (create new client each time)
    let start = std::time::Instant::now();
    for i in 0..3 {
        let client = CachedRibbitClient::new(Region::US).await?;
        let mut cached_client = client; // Make mutable
        cached_client.set_caching_enabled(false); // Disable cache for comparison
        let endpoint = Endpoint::ProductVersions("wow".to_string());
        let _ = cached_client.request_raw(&endpoint).await?;
        println!("   Request {} without cache: {:?}", i + 1, start.elapsed());
    }
    
    // With cache
    println!("\n   With caching enabled:");
    let client = CachedRibbitClient::new(Region::US).await?;
    let start = std::time::Instant::now();
    for i in 0..3 {
        let endpoint = Endpoint::ProductVersions("wow".to_string());
        let _ = client.request_raw(&endpoint).await?;
        println!("   Request {} with cache: {:?}", i + 1, start.elapsed());
    }

    println!("\n✓ Caching significantly improves performance for repeated requests");

    Ok(())
}
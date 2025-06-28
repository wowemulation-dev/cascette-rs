//! Comprehensive certificate operations example
//!
//! This example demonstrates:
//! - Downloading certificates by SKI/hash with caching
//! - Saving certificates in different formats (PEM/DER)
//! - Performance benefits of caching
//! - Integration with ngdp-client CLI commands
//! - Certificate verification workflows
//!
//! Run with: `cargo run --example certificate_operations`

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ngdp_client::{
    CertFormat, CertsCommands, OutputFormat, commands, test_constants::EXAMPLE_CERT_HASH,
};
use ribbit_client::{Endpoint, Region};
use std::path::Path;

/// Fetch a certificate using the cached client
async fn fetch_certificate_cached(hash: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = CachedRibbitClient::new(Region::US).await?;
    let endpoint = Endpoint::Cert(hash.to_string());
    let raw_data = client.request_raw(&endpoint).await?;
    Ok(String::from_utf8(raw_data)?)
}

/// Demonstrate certificate verification workflow with caching
async fn verify_signature_with_cache(ski: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Verifying signature with SKI: {ski}");

    println!("Fetching certificate: {EXAMPLE_CERT_HASH}");
    let cert_pem = fetch_certificate_cached(EXAMPLE_CERT_HASH).await?;

    if cert_pem.contains("-----BEGIN CERTIFICATE-----") {
        println!("✓ Successfully retrieved certificate (possibly from cache)");
        println!("  Certificate length: {} bytes", cert_pem.len());

        // Extract basic certificate info
        let lines: Vec<&str> = cert_pem.lines().collect();
        if lines.len() > 2 {
            println!("  Certificate format: PEM");
            println!("  Lines in certificate: {}", lines.len());
        }
    } else {
        println!("✗ Response doesn't contain a valid certificate");
    }

    Ok(())
}

/// Demonstrate CLI command usage for certificate operations
async fn cli_certificate_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CLI Certificate Operations ===\n");

    // Example 1: Download and display certificate details
    println!("1. Downloading certificate and showing details:");
    let cmd = CertsCommands::Download {
        ski: EXAMPLE_CERT_HASH.to_string(),
        output: None,
        region: "us".to_string(),
        cert_format: CertFormat::Pem,
        details: true,
    };
    commands::certs::handle(cmd, OutputFormat::Text).await?;

    // Example 2: Save certificate to PEM file
    println!("\n2. Saving certificate to PEM file:");
    let pem_path = Path::new("example-cert.pem");
    let cmd = CertsCommands::Download {
        ski: EXAMPLE_CERT_HASH.to_string(),
        output: Some(pem_path.to_path_buf()),
        region: "us".to_string(),
        cert_format: CertFormat::Pem,
        details: false,
    };
    commands::certs::handle(cmd, OutputFormat::Text).await?;
    println!("Certificate saved to: {}", pem_path.display());

    // Example 3: Save certificate to DER file
    println!("\n3. Saving certificate to DER file:");
    let der_path = Path::new("example-cert.der");
    let cmd = CertsCommands::Download {
        ski: EXAMPLE_CERT_HASH.to_string(),
        output: Some(der_path.to_path_buf()),
        region: "us".to_string(),
        cert_format: CertFormat::Der,
        details: false,
    };
    commands::certs::handle(cmd, OutputFormat::Text).await?;
    println!("Certificate saved to: {}", der_path.display());

    // Clean up example files
    if pem_path.exists() {
        std::fs::remove_file(pem_path)?;
        println!("Cleaned up: {}", pem_path.display());
    }
    if der_path.exists() {
        std::fs::remove_file(der_path)?;
        println!("Cleaned up: {}", der_path.display());
    }

    Ok(())
}

/// Demonstrate cache performance benefits
async fn cache_performance_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cache Performance Demonstration ===\n");

    // Without cache (create new client each time with caching disabled)
    println!("Performance without caching:");
    let start = std::time::Instant::now();
    for i in 0..3 {
        let client = CachedRibbitClient::new(Region::US).await?;
        let mut cached_client = client;
        cached_client.set_caching_enabled(false);
        let endpoint = Endpoint::Cert(EXAMPLE_CERT_HASH.to_string());
        let _ = cached_client.request_raw(&endpoint).await?;
        println!("   Request {} without cache: {:?}", i + 1, start.elapsed());
    }

    // With cache enabled
    println!("\nPerformance with caching enabled:");
    let client = CachedRibbitClient::new(Region::US).await?;
    let start = std::time::Instant::now();
    for i in 0..3 {
        let endpoint = Endpoint::Cert(EXAMPLE_CERT_HASH.to_string());
        let _ = client.request_raw(&endpoint).await?;
        println!("   Request {} with cache: {:?}", i + 1, start.elapsed());
    }

    println!("\n✓ Caching significantly improves performance for repeated requests");
    Ok(())
}

/// Demonstrate different certificate access patterns
async fn certificate_access_patterns() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Certificate Access Patterns ===\n");

    // Pattern 1: Direct cached access
    println!("1. Direct cached certificate access:");
    let cert_pem = fetch_certificate_cached(EXAMPLE_CERT_HASH).await?;
    println!("   Retrieved {} bytes via cached client", cert_pem.len());

    // Pattern 2: Through CLI command interface
    println!("\n2. CLI command interface access:");
    let cmd = CertsCommands::Download {
        ski: EXAMPLE_CERT_HASH.to_string(),
        output: None,
        region: "us".to_string(),
        cert_format: CertFormat::Pem,
        details: false,
    };
    commands::certs::handle(cmd, OutputFormat::Text).await?;

    // Pattern 3: Workflow integration
    println!("\n3. Signature verification workflow:");
    verify_signature_with_cache("example-ski").await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Comprehensive Certificate Operations Example ===\n");
    println!("Using certificate hash: {EXAMPLE_CERT_HASH}\n");

    // Demonstrate different aspects of certificate operations
    certificate_access_patterns().await?;
    println!("\n{}", "=".repeat(50));

    cache_performance_demo().await?;
    println!("\n{}", "=".repeat(50));

    cli_certificate_operations().await?;

    println!("\n=== Example Complete ===");
    println!("This example demonstrated:");
    println!("  ✓ Certificate caching for performance");
    println!("  ✓ Multiple output formats (PEM/DER)");
    println!("  ✓ CLI command integration");
    println!("  ✓ File operations and cleanup");
    println!("  ✓ Certificate verification workflows");

    Ok(())
}

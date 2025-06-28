//! Basic usage example for the Ribbit client
//!
//! This example demonstrates how to:
//! - Create a Ribbit client
//! - Query different endpoints using typed responses
//! - Handle responses with type safety
//!
//! Run with: `cargo run --example basic_usage`

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    // Create a client for the US region
    println!("Creating Ribbit client for US region...");
    let client = RibbitClient::new(Region::US);

    // Example 1: Get summary of all products (typed response)
    println!("\n1. Fetching product summary (typed)...");
    match client.get_summary().await {
        Ok(summary) => {
            println!("Total products: {}", summary.products.len());
            println!("First 5 products:");
            for product in summary.products.iter().take(5) {
                println!("  - {} (seqn: {})", product.product, product.seqn);
            }
        }
        Err(e) => println!("Error: {e}"),
    }

    // Example 2: Get WoW version information (typed response)
    println!("\n2. Fetching WoW versions (typed)...");
    match client.get_product_versions("wow").await {
        Ok(versions) => {
            println!("WoW Versions (sequence: {:?}):", versions.sequence_number);
            for entry in &versions.entries {
                println!(
                    "  {}: {} (build {})",
                    entry.region, entry.versions_name, entry.build_id
                );
            }
        }
        Err(e) => println!("Error: {e}"),
    }

    // Example 3: Get version info for different products
    println!("\n3. Fetching version info for different products...");
    let products = ["agent", "wow", "wow_classic", "wow_classic_era"];

    for product in products {
        println!("\n  Product: {product}");

        match client.get_product_versions(product).await {
            Ok(versions) => {
                // Get unique version names
                let unique_versions: std::collections::HashSet<_> =
                    versions.entries.iter().map(|e| &e.versions_name).collect();

                if let Some(version) = unique_versions.iter().next() {
                    println!("    Latest version: {version}");
                    println!("    Available in {} regions", versions.entries.len());
                }
            }
            Err(e) => println!("    Error: {e}"),
        }
    }

    // Example 4: Get CDN information (typed response)
    println!("\n4. Fetching CDN information...");
    match client.get_product_cdns("wow").await {
        Ok(cdns) => {
            println!("CDN Configuration:");
            for entry in &cdns.entries {
                println!("  {} -> {} hosts", entry.name, entry.hosts.len());
            }

            // Show all unique hosts
            let all_hosts = cdns.all_hosts();
            println!("\nUnique CDN hosts ({} total):", all_hosts.len());
            for host in all_hosts.iter().take(3) {
                println!("  - {host}");
            }
        }
        Err(e) => println!("Error: {e}"),
    }

    // Example 5: Raw response access (for certificates or custom endpoints)
    println!("\n5. Fetching certificate (raw response)...");
    let cert_hash = "5168ff90af0207753cccd9656462a212b859723b";
    match client.request(&Endpoint::Cert(cert_hash.to_string())).await {
        Ok(response) => {
            if let Some(text) = response.as_text() {
                if text.contains("BEGIN CERTIFICATE") {
                    println!("Successfully retrieved certificate for hash: {cert_hash}");
                    println!("Certificate size: {} bytes", text.len());
                }
            }
        }
        Err(e) => println!("Error: {e}"),
    }

    Ok(())
}

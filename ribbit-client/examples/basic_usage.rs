//! Basic usage example for the Ribbit client
//!
//! This example demonstrates how to:
//! - Create a Ribbit client
//! - Query different endpoints
//! - Handle responses
//!
//! Run with: `cargo run --example basic_usage`

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    // Create a client for the US region
    println!("Creating Ribbit client for US region...");
    let client = RibbitClient::new(Region::US);

    // Example 1: Get summary of all products
    println!("\n1. Fetching product summary (V1)...");
    match client.request_raw(&Endpoint::Summary).await {
        Ok(data) => {
            println!("Received {} bytes", data.len());
            // Print first 200 chars of response
            let preview = String::from_utf8_lossy(&data);
            println!(
                "Preview: {}...",
                &preview.chars().take(200).collect::<String>()
            );
        }
        Err(e) => println!("Error: {}", e),
    }

    // Example 2: Get WoW version information with V2 protocol
    println!("\n2. Fetching WoW versions (V2)...");
    let client_v2 = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);
    match client_v2
        .request_raw(&Endpoint::ProductVersions("wow".to_string()))
        .await
    {
        Ok(data) => {
            let response = String::from_utf8_lossy(&data);
            println!("WoW Versions (first 3 lines):");
            for line in response.lines().take(3) {
                println!("  {}", line);
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    // Example 3: Get version info for different products
    println!("\n3. Fetching version info for different products...");
    let products = ["agent", "wow", "wow_classic", "wow_classic_era"];

    for product in products {
        println!("\n  Product: {}", product);
        let endpoint = Endpoint::ProductVersions(product.to_string());

        match client.request_raw(&endpoint).await {
            Ok(data) => {
                let response = String::from_utf8_lossy(&data);
                // Extract version from first data line
                for line in response.lines() {
                    if line.contains("|") && !line.starts_with("Region") && !line.starts_with("#") {
                        if let Some(version) = line.split('|').nth(5) {
                            println!("    Latest version: {}", version);
                            break;
                        }
                    }
                }
            }
            Err(e) => println!("    Error: {}", e),
        }
    }

    // Example 4: Get a certificate
    println!("\n4. Fetching certificate...");
    let cert_hash = "5168ff90af0207753cccd9656462a212b859723b";
    match client
        .request_raw(&Endpoint::Cert(cert_hash.to_string()))
        .await
    {
        Ok(data) => {
            let response = String::from_utf8_lossy(&data);
            if response.contains("BEGIN CERTIFICATE") {
                println!("Successfully retrieved certificate for hash: {}", cert_hash);
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    Ok(())
}

//! Example demonstrating Ribbit queries for WoW-related products
//!
//! This example shows how to query information for:
//! - agent (Battle.net Agent)
//! - wow (World of Warcraft Retail)
//! - wow_classic (World of Warcraft Classic)
//! - wow_classic_era (World of Warcraft Classic Era)
//!
//! Run with: `cargo run --example wow_products`

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};
use std::collections::HashMap;

/// Product information structure
#[derive(Debug)]
struct ProductInfo {
    name: &'static str,
    latest_version: Option<String>,
    build_id: Option<String>,
    cdn_hosts: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Querying WoW Product Information via Ribbit\n");
    println!("{:=<60}", "");

    let products = [
        ("agent", "Battle.net Agent"),
        ("wow", "World of Warcraft (Retail)"),
        ("wow_classic", "World of Warcraft Classic"),
        ("wow_classic_era", "World of Warcraft Classic Era"),
    ];

    let mut product_infos: HashMap<&str, ProductInfo> = HashMap::new();

    // Initialize product info
    for (id, name) in &products {
        product_infos.insert(
            id,
            ProductInfo {
                name,
                latest_version: None,
                build_id: None,
                cdn_hosts: Vec::new(),
            },
        );
    }

    let client = RibbitClient::new(Region::US);

    // Fetch version information
    println!("Fetching version information...\n");
    for (product_id, _) in &products {
        let endpoint = Endpoint::ProductVersions(product_id.to_string());

        match client.request_raw(&endpoint).await {
            Ok(data) => {
                let response = String::from_utf8_lossy(&data);

                // Parse the first data line (usually US region)
                for line in response.lines() {
                    if line.contains("|") && !line.starts_with("Region") && !line.starts_with("#") {
                        let fields: Vec<&str> = line.split('|').collect();
                        if fields.len() >= 6 {
                            if let Some(info) = product_infos.get_mut(product_id) {
                                info.build_id = Some(fields[4].to_string());
                                info.latest_version = Some(fields[5].to_string());
                            }
                        }
                        break;
                    }
                }
            }
            Err(e) => eprintln!("Failed to fetch versions for {}: {}", product_id, e),
        }
    }

    // Fetch CDN information
    println!("Fetching CDN information...\n");
    let cdn_client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    for (product_id, _) in &products {
        let endpoint = Endpoint::ProductCdns(product_id.to_string());

        match cdn_client.request_raw(&endpoint).await {
            Ok(data) => {
                let response = String::from_utf8_lossy(&data);

                // Parse CDN hosts
                for line in response.lines() {
                    if line.contains("|") && !line.starts_with("Name") && !line.starts_with("#") {
                        let fields: Vec<&str> = line.split('|').collect();
                        if fields.len() >= 3 {
                            if let Some(info) = product_infos.get_mut(product_id) {
                                let hosts: Vec<String> = fields[2]
                                    .split_whitespace()
                                    .take(2) // Take first 2 hosts
                                    .map(|s| s.to_string())
                                    .collect();
                                info.cdn_hosts = hosts;
                            }
                        }
                        break;
                    }
                }
            }
            Err(e) => eprintln!("Failed to fetch CDNs for {}: {}", product_id, e),
        }
    }

    // Display results
    println!("{:=<60}", "");
    println!("Product Information Summary");
    println!("{:=<60}\n", "");

    for (product_id, info) in &product_infos {
        println!("Product: {} ({})", info.name, product_id);
        println!(
            "  Version: {}",
            info.latest_version.as_deref().unwrap_or("N/A")
        );
        println!("  Build ID: {}", info.build_id.as_deref().unwrap_or("N/A"));

        if !info.cdn_hosts.is_empty() {
            println!("  CDN Hosts:");
            for host in &info.cdn_hosts {
                println!("    - {}", host);
            }
        } else {
            println!("  CDN Hosts: N/A");
        }
        println!();
    }

    // Check for background download endpoints
    println!("{:=<60}", "");
    println!("Background Download Status");
    println!("{:=<60}\n", "");

    for (product_id, name) in &products {
        let endpoint = Endpoint::ProductBgdl(product_id.to_string());

        match client.request_raw(&endpoint).await {
            Ok(data) => {
                if data.is_empty() {
                    println!("{}: No BGDL data", name);
                } else {
                    let response = String::from_utf8_lossy(&data);
                    let has_data = response.lines().any(|line| {
                        line.contains("|") && !line.starts_with("Region") && !line.starts_with("#")
                    });
                    println!(
                        "{}: {}",
                        name,
                        if has_data {
                            "BGDL available"
                        } else {
                            "No BGDL data"
                        }
                    );
                }
            }
            Err(e) => println!("{}: Error - {}", name, e),
        }
    }

    println!("\n{:=<60}", "");

    Ok(())
}

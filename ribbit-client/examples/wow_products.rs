//! Example demonstrating Ribbit queries for WoW-related products
//!
//! This example shows how to query information for:
//! - agent (Battle.net Agent)
//! - wow (World of Warcraft Retail)
//! - wow_classic (World of Warcraft Classic)
//! - wow_classic_era (World of Warcraft Classic Era)
//!
//! Run with: `cargo run --example wow_products`

use ribbit_client::{Region, RibbitClient};
use std::collections::HashMap;

/// Product information structure
#[derive(Debug)]
struct ProductInfo {
    name: &'static str,
    latest_version: Option<String>,
    build_id: Option<u32>,
    cdn_hosts: Vec<String>,
    bgdl_available: bool,
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
                bgdl_available: false,
            },
        );
    }

    let client = RibbitClient::new(Region::US);

    // Fetch version information using typed API
    println!("Fetching version information...\n");
    for (product_id, _) in &products {
        match client.get_product_versions(product_id).await {
            Ok(versions) => {
                // Get the first entry (usually US region)
                if let Some(first_entry) = versions.entries.first() {
                    if let Some(info) = product_infos.get_mut(product_id) {
                        info.build_id = Some(first_entry.build_id);
                        info.latest_version = Some(first_entry.versions_name.clone());
                    }
                }
            }
            Err(e) => eprintln!("Failed to fetch versions for {}: {}", product_id, e),
        }
    }

    // Fetch CDN information using typed API
    println!("Fetching CDN information...\n");
    for (product_id, _) in &products {
        match client.get_product_cdns(product_id).await {
            Ok(cdns) => {
                // Get all unique hosts
                let all_hosts = cdns.all_hosts();
                if let Some(info) = product_infos.get_mut(product_id) {
                    info.cdn_hosts = all_hosts.into_iter().take(2).collect();
                }
            }
            Err(e) => eprintln!("Failed to fetch CDNs for {}: {}", product_id, e),
        }
    }

    // Check for background download endpoints using typed API
    println!("Checking background download availability...\n");
    for (product_id, _) in &products {
        match client.get_product_bgdl(product_id).await {
            Ok(bgdl) => {
                if let Some(info) = product_infos.get_mut(product_id) {
                    info.bgdl_available = !bgdl.entries.is_empty();
                }
            }
            Err(_) => {
                // BGDL might not be available for all products
                if let Some(info) = product_infos.get_mut(product_id) {
                    info.bgdl_available = false;
                }
            }
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
        println!(
            "  Build ID: {}",
            info.build_id
                .map(|b| b.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        );

        if !info.cdn_hosts.is_empty() {
            println!("  CDN Hosts:");
            for host in &info.cdn_hosts {
                println!("    - {}", host);
            }
        } else {
            println!("  CDN Hosts: N/A");
        }

        println!(
            "  BGDL: {}",
            if info.bgdl_available {
                "Available"
            } else {
                "Not available"
            }
        );
        println!();
    }

    // Additional analysis using convenience methods
    println!("{:=<60}", "");
    println!("Additional Analysis");
    println!("{:=<60}\n", "");

    // Compare build IDs across products
    println!("Build ID Comparison:");
    for (product_id, info) in &product_infos {
        if let Some(build_id) = info.build_id {
            println!("  {}: {}", product_id, build_id);
        }
    }

    // Check if classic products exist
    let classic_products = ["wow_classic", "wow_classic_era"];
    let active_classic_products = classic_products
        .iter()
        .filter(|&&p| product_infos.get(p).and_then(|i| i.build_id).is_some())
        .count();

    println!(
        "\nActive Classic products: {}/{}",
        active_classic_products,
        classic_products.len()
    );

    println!("\n{:=<60}", "");

    Ok(())
}

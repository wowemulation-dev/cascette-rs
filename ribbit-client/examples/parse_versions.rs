//! Example showing how to parse version information from Ribbit
//!
//! This example demonstrates:
//! - Using typed responses for version data
//! - Extracting specific fields with type safety
//! - Working with convenience methods
//!
//! Run with: `cargo run --example parse_versions`

use ribbit_client::{Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US);

    println!("Fetching WoW version information...\n");

    // Use the typed API for automatic parsing
    let versions = client.get_product_versions("wow").await?;

    // Display sequence number
    if let Some(seqn) = versions.sequence_number {
        println!("Sequence number: {}\n", seqn);
    }

    // Display version information by region
    println!("Version information by region:");
    println!("{:-<60}", "");

    for entry in &versions.entries {
        println!(
            "Region: {:6} | Build: {:6} | Version: {}",
            entry.region, entry.build_id, entry.versions_name
        );
    }

    println!("{:-<60}", "");

    // Use convenience methods
    println!("\n📊 Additional Analysis:");

    // Get specific region
    if let Some(us_version) = versions.get_region("us") {
        println!("\nUS Version Details:");
        println!("  Version: {}", us_version.versions_name);
        println!("  Build ID: {}", us_version.build_id);
        println!(
            "  Build Config: {}...{}",
            &us_version.build_config[..8],
            &us_version.build_config[us_version.build_config.len() - 8..]
        );
        println!(
            "  Product Config: {}...{}",
            &us_version.product_config[..8],
            &us_version.product_config[us_version.product_config.len() - 8..]
        );
    }

    // Get unique builds
    let builds = versions.build_ids();
    println!("\nUnique build IDs: {:?}", builds);

    // Check if all regions have the same build
    let all_same_build = versions
        .entries
        .iter()
        .all(|e| e.build_id == versions.entries[0].build_id);

    println!("\nAll regions on same build: {}", all_same_build);

    // Count regions with specific version name
    let version_name = &versions.entries[0].versions_name;
    let regions_with_version = versions
        .entries
        .iter()
        .filter(|e| &e.versions_name == version_name)
        .count();

    println!(
        "Regions with version {}: {}/{}",
        version_name,
        regions_with_version,
        versions.entries.len()
    );

    Ok(())
}

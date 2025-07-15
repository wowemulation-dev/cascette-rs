//! Example of using the Wago Tools API to fetch build history

use ngdp_client::wago_api;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see cache behavior
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Fetch all builds from Wago Tools
    println!("Fetching build history from Wago Tools API...");
    let start = Instant::now();
    let builds_response = wago_api::fetch_builds().await?;

    // Filter for WoW builds
    let wow_builds = wago_api::filter_builds_by_product(builds_response, "wow");
    let elapsed = start.elapsed();

    println!(
        "Found {} WoW builds (fetched in {:?})",
        wow_builds.len(),
        elapsed
    );

    // Show the 5 most recent builds
    println!("\nMost recent 5 builds:");
    for (i, build) in wow_builds.iter().take(5).enumerate() {
        println!("{}. Version: {}", i + 1, build.version);
        println!("   Created: {}", build.created_at);
        println!("   Build Config: {}", build.build_config);
        if let Some(cdn_config) = &build.cdn_config {
            println!("   CDN Config: {}", cdn_config);
        }
        if let Some(product_config) = &build.product_config {
            println!("   Product Config: {}", product_config);
        }
        println!("   Type: {}", if build.is_bgdl { "BGDL" } else { "Full" });
        println!();
    }

    // Demonstrate date parsing
    if let Some(first_build) = wow_builds.first() {
        if let Some(date) = wago_api::parse_wago_date(&first_build.created_at) {
            println!(
                "First build was created on: {}",
                date.format("%Y-%m-%d at %H:%M UTC")
            );
        }
    }

    // Demonstrate cache behavior
    println!("\n--- Testing cache behavior ---");
    println!("Fetching again (should use cache)...");
    let start = Instant::now();
    let _cached_response = wago_api::fetch_builds().await?;
    let cached_elapsed = start.elapsed();

    println!(
        "Second fetch completed in {:?} (should be much faster)",
        cached_elapsed
    );

    // Note: The cache has a 30-minute TTL
    println!("\nNote: The cache expires after 30 minutes");
    println!("Use --no-cache flag with ngdp CLI to bypass cache");

    Ok(())
}

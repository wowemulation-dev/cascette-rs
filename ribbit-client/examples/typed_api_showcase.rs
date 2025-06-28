//! Showcase the typed API - demonstrating simplicity similar to Ribbit.NET
//!
//! This example shows how easy it is to work with typed responses

use ribbit_client::{Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US);

    println!("🎮 Ribbit Client - Typed API Showcase\n");

    // 1. Get summary of all products (simple as Ribbit.NET)
    println!("📋 All Blizzard Products:");
    let summary = client.get_summary().await?;
    for product in &summary.products {
        println!("  • {} (seqn: {})", product.product, product.seqn);
    }

    // 2. Get WoW versions with typed access
    println!("\n🌍 WoW Version Information:");
    let versions = client.get_product_versions("wow").await?;

    // Direct field access - no parsing needed!
    if let Some(us_version) = versions.get_region("us") {
        println!(
            "  US: {} (build {})",
            us_version.versions_name, us_version.build_id
        );
        println!("      Build Config: {}", &us_version.build_config[..8]);
        println!("      CDN Config: {}", &us_version.cdn_config[..8]);
    }

    // Get unique builds
    let builds = versions.build_ids();
    println!("  Unique builds: {builds:?}");

    // 3. Get CDN information with convenience methods
    println!("\n🌐 WoW CDN Servers:");
    let cdns = client.get_product_cdns("wow").await?;

    // List all unique hosts
    let all_hosts = cdns.all_hosts();
    println!("  Total unique hosts: {}", all_hosts.len());
    for host in all_hosts.iter().take(3) {
        println!("    - {host}");
    }

    // 4. Raw response access still available
    println!("\n📄 Raw Response Access:");
    let response = client.request(&ribbit_client::Endpoint::Summary).await?;

    // Simple text access like Ribbit.NET's ToString()
    if let Some(text) = response.as_text() {
        let preview = text.lines().take(3).collect::<Vec<_>>().join("\n");
        println!("  Preview: {preview}");
    }

    // Or parse as BPSV for advanced usage
    let bpsv = response.as_bpsv()?;
    println!("  BPSV rows: {}", bpsv.row_count());

    println!("\n✅ Done!");

    Ok(())
}

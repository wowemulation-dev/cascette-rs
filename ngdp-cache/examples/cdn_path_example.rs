//! Example showing CDN path construction for given hashes
//!
//! This example demonstrates how CDN paths are constructed for different
//! types of content (config, data, patch) following the standard CDN structure.

use ngdp_cache::cdn::CdnCache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CDN Path Construction Example ===\n");

    // Given hashes
    let build_config = "d762957a75867e5ebe68b7b9ecbb9ff7";
    let cdn_config = "cea70cf029e4ff7e8d4fbf497f87e50e";
    let product_config = "c9934edfc8f217a2e01c47e4deae8454";

    // Create CDN cache to demonstrate path construction
    let cdn = CdnCache::new().await?;

    println!("For US region CDN servers, the full download paths would be:\n");

    // Build config path
    let build_config_path = cdn.cache_path("tpr/wow/config", build_config, "");
    println!("Build Config:");
    println!("  Hash: {build_config}");
    println!("  Cache path: {build_config_path:?}");
    println!(
        "  CDN URL: http://{{cdn_host}}/{{product_path}}/config/{}/{}/{}",
        &build_config[0..2],
        &build_config[2..4],
        build_config
    );
    println!(
        "  Example: http://level3.blizzard.com/tpr/wow/config/{}/{}/{}",
        &build_config[0..2],
        &build_config[2..4],
        build_config
    );

    println!("\nCDN Config:");
    println!("  Hash: {cdn_config}");
    let cdn_config_path = cdn.cache_path("tpr/wow/config", cdn_config, "");
    println!("  Cache path: {cdn_config_path:?}");
    println!(
        "  CDN URL: http://{{cdn_host}}/{{product_path}}/config/{}/{}/{}",
        &cdn_config[0..2],
        &cdn_config[2..4],
        cdn_config
    );
    println!(
        "  Example: http://level3.blizzard.com/tpr/wow/config/{}/{}/{}",
        &cdn_config[0..2],
        &cdn_config[2..4],
        cdn_config
    );

    println!("\nProduct Config (uses different path!):");
    println!("  Hash: {product_config}");
    let product_config_path = cdn.cache_path("tpr/configs/data", cdn_config, "");
    println!("  Cache path: {product_config_path:?}");
    println!(
        "  CDN URL: http://{{cdn_host}}/{{config_path}}/{}/{}/{}",
        &product_config[0..2],
        &product_config[2..4],
        product_config
    );
    println!(
        "  Example: http://level3.blizzard.com/tpr/configs/data/{}/{}/{}",
        &product_config[0..2],
        &product_config[2..4],
        product_config
    );

    println!("\n=== Actual Full URLs for US Region ===\n");

    // Common CDN hosts for US region
    let cdn_hosts = vec![
        "level3.blizzard.com",
        "edgecast.blizzard.com",
        "cdn.blizzard.com",
    ];

    println!("Build Config ({build_config}):");
    for host in &cdn_hosts {
        println!(
            "  http://{}/tpr/wow/config/{}/{}/{}",
            host,
            &build_config[0..2],
            &build_config[2..4],
            build_config
        );
    }

    println!("\nCDN Config ({cdn_config}):");
    for host in &cdn_hosts {
        println!(
            "  http://{}/tpr/wow/config/{}/{}/{}",
            host,
            &cdn_config[0..2],
            &cdn_config[2..4],
            cdn_config
        );
    }

    println!("\nProduct Config ({product_config}) - uses config_path:");
    for host in &cdn_hosts {
        println!(
            "  http://{}/tpr/configs/data/{}/{}/{}",
            host,
            &product_config[0..2],
            &product_config[2..4],
            product_config
        );
    }

    println!("\n=== Path Components Breakdown ===\n");
    println!(
        "Pattern: http://{{cdn_host}}/{{product_path}}/{{content_type}}/{{hash[0:2]}}/{{hash[2:4]}}/{{hash}}"
    );
    println!("\nFor build config {build_config}:");
    println!("  cdn_host: level3.blizzard.com (or other CDN servers)");
    println!("  product_path: tpr/wow");
    println!("  content_type: config");
    println!("  hash[0:2]: {}", &build_config[0..2]);
    println!("  hash[2:4]: {}", &build_config[2..4]);
    println!("  full hash: {build_config}");

    Ok(())
}

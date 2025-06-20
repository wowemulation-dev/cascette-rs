//! Basic usage example for ngdp-cache

use ngdp_cache::{cdn::CdnCache, generic::GenericCache, ribbit::RibbitCache, tact::TactCache};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== NGDP Cache Example ===\n");

    // Generic cache example
    println!("1. Generic Cache:");
    let generic = GenericCache::new().await?;
    generic.write("my_key", b"Hello, World!").await?;
    let data = generic.read("my_key").await?;
    println!(
        "   Read from generic cache: {}",
        String::from_utf8_lossy(&data)
    );

    // TACT cache example
    println!("\n2. TACT Cache:");
    let tact = TactCache::new().await?;
    let config_hash = "abcdef1234567890abcdef1234567890";
    tact.write_config(config_hash, b"build-config-data").await?;
    println!("   Config cached at: {:?}", tact.config_path(config_hash));

    // CDN cache example
    println!("\n3. CDN Cache:");
    let cdn = CdnCache::for_product("wow").await?;
    let archive_hash = "1234567890abcdef1234567890abcdef";
    cdn.write_archive(archive_hash, b"archive-data").await?;
    println!("   Archive cached at: {:?}", cdn.archive_path(archive_hash));

    // Ribbit cache example
    println!("\n4. Ribbit Cache:");
    let ribbit = RibbitCache::new().await?;
    ribbit
        .write("us", "wow", "versions", b"version-data")
        .await?;
    if ribbit.is_valid("us", "wow", "versions").await {
        println!("   Ribbit response cached and valid!");
    }

    // Show cache directory
    println!("\n5. Cache Directory:");
    println!("   Base cache dir: {:?}", ngdp_cache::get_cache_dir()?);

    Ok(())
}

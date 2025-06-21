//! Basic usage example for ngdp-cache

use ngdp_cache::{cdn::CdnCache, generic::GenericCache, ribbit::RibbitCache};

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

    // CDN cache example
    println!("\n2. CDN Cache:");
    let cdn = CdnCache::for_product("wow").await?;
    
    // Cache config data
    let config_hash = "abcdef1234567890abcdef1234567890";
    cdn.write_config(config_hash, b"build-config-data").await?;
    println!("   Config cached at: {:?}", cdn.config_path(config_hash));
    
    // Cache data/archive
    let data_hash = "1234567890abcdef1234567890abcdef";
    cdn.write_data(data_hash, b"archive-data").await?;
    println!("   Data cached at: {:?}", cdn.data_path(data_hash));

    // Ribbit cache example
    println!("\n3. Ribbit Cache:");
    let ribbit = RibbitCache::new().await?;
    ribbit
        .write("us", "wow", "versions", b"version-data")
        .await?;
    if ribbit.is_valid("us", "wow", "versions").await {
        println!("   Ribbit response cached and valid!");
    }

    // Show cache directory
    println!("\n4. Cache Directory:");
    println!("   Base cache dir: {:?}", ngdp_cache::get_cache_dir()?);

    Ok(())
}

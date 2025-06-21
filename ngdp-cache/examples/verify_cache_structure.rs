//! Example to verify correct cache directory structure
//!
//! This example demonstrates that CachedRibbitClient now uses the correct
//! cache directory structure: ~/.cache/ngdp/ribbit/{region}/

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== Verifying Cache Directory Structure ===\n");

    // Create clients for different regions
    let regions = vec![Region::US, Region::EU, Region::KR, Region::CN];

    for region in regions {
        println!("Creating cached client for region: {:?}", region);
        let client = CachedRibbitClient::new(region).await?;

        // Make a request to create cache files
        let endpoint = Endpoint::Summary;
        match client.request_raw(&endpoint).await {
            Ok(data) => {
                println!("  ✓ Successfully fetched {} bytes", data.len());
            }
            Err(e) => {
                println!("  ✗ Failed to fetch data: {}", e);
            }
        }
    }

    // Show the expected cache directory structure
    let cache_base = dirs::cache_dir().unwrap().join("ngdp").join("ribbit");

    println!("\nExpected cache directory structure:");
    println!("  {}/", cache_base.display());

    if cache_base.exists() {
        show_directory_tree(&cache_base, 2).await?;
    } else {
        println!("  (Cache directory not created yet)");
    }

    // Verify no "cached" subdirectory exists
    let incorrect_path = cache_base.join("cached");
    if incorrect_path.exists() {
        println!("\n⚠️  WARNING: Found incorrect 'cached' subdirectory!");
        println!("   This should not exist. The cache should be directly under:");
        println!("   ~/.cache/ngdp/ribbit/{{region}}/");
    } else {
        println!("\n✓ Correct structure verified - no 'cached' subdirectory found");
    }

    Ok(())
}

/// Recursively show directory tree structure
#[allow(clippy::type_complexity)]
fn show_directory_tree(
    path: &PathBuf,
    indent: usize,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send + '_>,
> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let file_name = entry.file_name();

            // Print indentation
            print!("{}", " ".repeat(indent));

            if entry_path.is_dir() {
                println!("├── {}/", file_name.to_string_lossy());
                // Recurse into subdirectory
                show_directory_tree(&entry_path, indent + 2).await?;
            } else {
                println!("├── {}", file_name.to_string_lossy());
            }
        }

        Ok(())
    })
}

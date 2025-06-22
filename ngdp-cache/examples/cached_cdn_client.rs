//! Example of using CachedCdnClient to download CDN content with caching
//!
//! This example demonstrates:
//! - Creating a cached CDN client
//! - Downloading content that gets cached automatically
//! - Verifying cache hits on subsequent requests
//! - Checking cache statistics
//! - Streaming large files

use ngdp_cache::cached_cdn_client::CachedCdnClient;
use std::time::Instant;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Create a cached CDN client for WoW
    info!("Creating cached CDN client...");
    let client = CachedCdnClient::for_product("wow").await?;

    // Example CDN configuration (these would normally come from TACT)
    let cdn_host = "blzddist1-a.akamaihd.net";
    let config_hash = "2e9a3b4e0a0cfce3b8f3e8a2c1d6f9a7"; // Example config hash
    let data_hash = "c6e0c7b11f1e8c47a8ef1234567890ab"; // Example data hash

    // First download - will fetch from CDN
    info!("Downloading config file (first time - from CDN)...");
    let start = Instant::now();
    match client
        .download(cdn_host, "tpr/configs/data", config_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            let elapsed = start.elapsed();
            info!(
                "Downloaded {} bytes in {:?} (from cache: {})",
                data.len(),
                elapsed,
                is_cached
            );
        }
        Err(e) => {
            info!("Failed to download config (expected in example): {}", e);
        }
    }

    // Second download - should come from cache
    info!("Downloading config file again (should be cached)...");
    let start = Instant::now();
    match client
        .download(cdn_host, "tpr/configs/data", config_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let data = response.bytes().await?;
            let elapsed = start.elapsed();
            info!(
                "Downloaded {} bytes in {:?} (from cache: {})",
                data.len(),
                elapsed,
                is_cached
            );
        }
        Err(e) => {
            info!("Failed to download config: {}", e);
        }
    }

    // Example of downloading different content types
    info!("\nDemonstrating different content types:");

    // Data file example
    match client.download(cdn_host, "data", data_hash).await {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            let content_length = response.content_length();
            info!(
                "Data file download - from cache: {}, size: {} bytes",
                is_cached, content_length
            );
        }
        Err(e) => {
            info!("Failed to download data file (expected in example): {}", e);
        }
    }

    // Check cache statistics
    info!("\nCache statistics:");
    match client.cache_stats().await {
        Ok(stats) => {
            info!("Total files: {}", stats.total_files);
            info!("Total size: {}", stats.total_size_human());
            info!(
                "Config files: {} ({})",
                stats.config_files,
                stats.config_size_human()
            );
            info!(
                "Data files: {} ({})",
                stats.data_files,
                stats.data_size_human()
            );
            info!(
                "Patch files: {} ({})",
                stats.patch_files,
                stats.patch_size_human()
            );
        }
        Err(e) => {
            info!("Failed to get cache stats: {}", e);
        }
    }

    // Example of checking if content is cached before downloading
    info!("\nChecking cached content size:");
    match client.cached_size("tpr/configs/data", config_hash).await? {
        Some(size) => info!("Config file is cached, size: {} bytes", size),
        None => info!("Config file is not cached"),
    }

    // Example of disabling caching temporarily
    info!("\nDisabling caching temporarily:");
    let mut mutable_client = CachedCdnClient::for_product("wow").await?;
    mutable_client.set_caching_enabled(false);

    match mutable_client
        .download(cdn_host, "tpr/configs/data", config_hash)
        .await
    {
        Ok(response) => {
            let is_cached = response.is_from_cache();
            info!(
                "Downloaded with caching disabled - from cache: {}",
                is_cached
            );
        }
        Err(e) => {
            info!("Failed to download: {}", e);
        }
    }

    // Example with custom cache directory
    info!("\nUsing custom cache directory:");
    let temp_dir = std::env::temp_dir().join("ngdp-cache-example");
    let custom_client = CachedCdnClient::with_cache_dir(temp_dir.clone()).await?;
    info!("Cache directory: {:?}", custom_client.cache_dir());

    // Streaming example for large files
    info!("\nStreaming example:");
    let large_file_hash = "1234567890abcdef1234567890abcdef"; // Example large file
    match custom_client
        .download_stream(cdn_host, "data", large_file_hash)
        .await
    {
        Ok(mut stream) => {
            let mut buffer = vec![0u8; 1024];
            match tokio::io::AsyncReadExt::read(&mut *stream, &mut buffer).await {
                Ok(n) => info!("Read {} bytes from stream", n),
                Err(e) => info!("Failed to read from stream: {}", e),
            }
        }
        Err(e) => {
            info!("Failed to open stream (expected in example): {}", e);
        }
    }

    // Clean up custom cache directory
    if temp_dir.exists() {
        tokio::fs::remove_dir_all(&temp_dir).await?;
        info!("Cleaned up custom cache directory");
    }

    Ok(())
}

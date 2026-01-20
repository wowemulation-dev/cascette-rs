//! WoW Classic Product Information Example (Native)
//!
//! This example demonstrates querying WoW Classic product information
//! and downloading the core configuration files needed for installation.
//!
//! Run with: cargo run --example wow_classic_native
//!
//! This example uses full native features including:
//! - Ribbit TCP protocol fallback
//! - Connection pooling and HTTP/2
//! - Streaming downloads
//! - Disk caching

use cascette_protocol::{CdnClient, CdnConfig, ClientConfig, ContentType, RibbitTactClient};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("cascette_protocol=info")
        .init();

    println!("=== WoW Classic Product Information (Native) ===\n");

    // Step 1: Create the unified protocol client
    // This client automatically falls back: TACT HTTPS -> HTTP -> Ribbit TCP
    let config = ClientConfig::default();
    let client = RibbitTactClient::new(config)?;

    // Step 2: Query version information for wow_classic
    println!("Querying wow_classic versions...");
    let versions = client.query("v1/products/wow_classic/versions").await?;

    println!("Found {} region(s):\n", versions.rows().len());

    // Display version information for each region
    for row in versions.rows() {
        let region = row
            .get_by_name("Region", versions.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let version_name = row
            .get_by_name("VersionsName", versions.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let build_id = row
            .get_by_name("BuildId", versions.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let build_config = row
            .get_by_name("BuildConfig", versions.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let cdn_config = row
            .get_by_name("CDNConfig", versions.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        println!("  Region: {region}");
        println!("    Version: {version_name}");
        println!("    Build ID: {build_id}");
        println!("    BuildConfig: {build_config}");
        println!("    CDNConfig: {cdn_config}");
        println!();
    }

    // Step 3: Query CDN endpoints
    println!("Querying CDN endpoints...");
    let cdns = client.query("v1/products/wow_classic/cdns").await?;

    println!("Available CDN endpoints:\n");
    for row in cdns.rows() {
        let name = row
            .get_by_name("Name", cdns.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let hosts = row
            .get_by_name("Hosts", cdns.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let path = row
            .get_by_name("Path", cdns.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        println!("  {name}: {hosts} (path: {path})");
    }
    println!();

    // Step 4: Extract CDN endpoint for downloads
    let cdn_endpoint = CdnClient::endpoint_from_bpsv_row(
        cdns.rows().first().ok_or("No CDN configurations found")?,
        cdns.schema(),
    )?;

    println!("Using CDN: {}\n", cdn_endpoint.host);

    // Step 5: Get the first region's config hashes for download
    let first_version = versions.rows().first().ok_or("No versions found")?;

    let build_config_hash = first_version
        .get_by_name("BuildConfig", versions.schema())
        .and_then(|v| v.as_string())
        .ok_or("Missing BuildConfig")?;

    let cdn_config_hash = first_version
        .get_by_name("CDNConfig", versions.schema())
        .and_then(|v| v.as_string())
        .ok_or("Missing CDNConfig")?;

    // Step 6: Create CDN client and download configurations
    let cdn_client = CdnClient::new(client.cache().clone(), CdnConfig::default())?;

    // Download build config
    println!("Downloading build config ({build_config_hash})...");
    let build_config_key = hex::decode(build_config_hash)?;
    let build_config_data = cdn_client
        .download(&cdn_endpoint, ContentType::Config, &build_config_key)
        .await?;
    println!("  Downloaded {} bytes", build_config_data.len());

    // Download CDN config
    println!("Downloading CDN config ({cdn_config_hash})...");
    let cdn_config_key = hex::decode(cdn_config_hash)?;
    let cdn_config_data = cdn_client
        .download(&cdn_endpoint, ContentType::Config, &cdn_config_key)
        .await?;
    println!("  Downloaded {} bytes\n", cdn_config_data.len());

    // Step 7: Parse build config to extract core file references
    println!("=== Build Configuration Contents ===\n");
    let build_config_text = String::from_utf8_lossy(&build_config_data);

    // Extract key fields from the config
    for line in build_config_text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once(" = ") {
            match key {
                "root" => println!("Root CKey: {value}"),
                "encoding" => {
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() >= 2 {
                        println!("Encoding CKey: {}", parts[0]);
                        println!("Encoding EKey: {} (use this for CDN download)", parts[1]);
                    }
                }
                "install" => {
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() >= 2 {
                        println!("Install CKey: {}", parts[0]);
                        println!("Install EKey: {} (use this for CDN download)", parts[1]);
                    }
                }
                "download" => {
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() >= 2 {
                        println!("Download CKey: {}", parts[0]);
                        println!("Download EKey: {} (use this for CDN download)", parts[1]);
                    }
                }
                "build-name" => println!("Build Name: {value}"),
                "build-product" => println!("Product: {value}"),
                _ => {}
            }
        }
    }

    println!("\n=== CDN Configuration Contents ===\n");
    let cdn_config_text = String::from_utf8_lossy(&cdn_config_data);

    // Extract archive information
    for line in cdn_config_text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once(" = ") {
            match key {
                "archives" => {
                    let archives: Vec<&str> = value.split_whitespace().collect();
                    println!("Archives: {} total", archives.len());
                    if !archives.is_empty() {
                        println!("  First: {}", archives[0]);
                        if archives.len() > 1 {
                            println!("  Last: {}", archives[archives.len() - 1]);
                        }
                    }
                }
                "archive-group" => println!("Archive Group: {value}"),
                "patch-archives" => {
                    let patch_count = value.split_whitespace().count();
                    println!("Patch Archives: {patch_count} total");
                }
                _ => {}
            }
        }
    }

    // Step 8: Demonstrate downloading a core file (encoding file)
    println!("\n=== Downloading Core Files ===\n");

    // Find the encoding EKey from the build config
    let encoding_ekey = build_config_text
        .lines()
        .find(|line| line.starts_with("encoding = "))
        .and_then(|line| line.strip_prefix("encoding = "))
        .and_then(|value| value.split_whitespace().nth(1));

    if let Some(ekey) = encoding_ekey {
        println!("Downloading encoding file ({ekey})...");
        let encoding_key = hex::decode(ekey)?;

        // Download with progress tracking
        let encoding_data = cdn_client
            .download_with_progress(
                &cdn_endpoint,
                ContentType::Data,
                &encoding_key,
                #[allow(clippy::cast_precision_loss)] // Precision loss acceptable for progress %
                |downloaded, total| {
                    if total > 0 {
                        let pct = (downloaded as f64 / total as f64) * 100.0;
                        print!("\r  Progress: {pct:.1}% ({downloaded} / {total} bytes)");
                    }
                },
            )
            .await?;

        println!(
            "\n  Encoding file: {} bytes (BLTE compressed)",
            encoding_data.len()
        );
    }

    // Step 9: Show cache statistics
    println!("\n=== Cache Statistics ===\n");
    let stats = client.cache().stats()?;
    println!("  Hit rate: {:.1}%", stats.hit_rate());
    println!("  Memory usage: {} KB", stats.memory_usage / 1024);
    println!("  Total entries: {}", stats.entries);

    println!("\n=== Summary ===\n");
    println!("To install WoW Classic, you would need to:");
    println!("1. Download the encoding file (maps CKey -> EKey)");
    println!("2. Download the root file (maps FileDataID -> CKey)");
    println!("3. Download the install manifest (lists required files)");
    println!("4. Use install manifest to identify needed files");
    println!("5. Resolve files through: FileDataID -> Root -> CKey -> Encoding -> EKey");
    println!("6. Download files from CDN archives using EKey + archive index");

    Ok(())
}

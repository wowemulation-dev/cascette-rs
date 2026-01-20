//! WoW Classic Product Information Example (WASM)
//!
//! This example demonstrates querying WoW Classic product information
//! in a WASM-compatible way, suitable for browser applications.
//!
//! This example is designed to compile for wasm32-unknown-unknown target.
//! It uses only WASM-compatible features:
//! - TACT HTTPS/HTTP via browser Fetch API
//! - localStorage for caching
//! - gloo-timers for async sleep
//!
//! To test locally (requires wasm-pack):
//!   wasm-pack build --target web
//!
//! Note: This example uses conditional compilation to demonstrate
//! both native testing and WASM deployment patterns.

use cascette_protocol::{CdnClient, CdnConfig, ClientConfig, ContentType, RibbitTactClient};

/// Main entry point for WASM
///
/// In a real WASM application, this would be called from JavaScript
/// using wasm-bindgen. For demonstration, we show the async function
/// that would be exposed.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() -> Result<(), JsValue> {
    // Set up console logging for WASM
    console_error_panic_hook::set_once();

    query_wow_classic()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Query WoW Classic product information
///
/// This function works on both native and WASM platforms.
/// On WASM, it uses the browser's Fetch API instead of native sockets.
pub async fn query_wow_classic() -> Result<(), Box<dyn std::error::Error>> {
    log("=== WoW Classic Product Information (WASM Compatible) ===\n");

    // Step 1: Create the protocol client
    // On WASM, this only uses TACT HTTPS/HTTP (no Ribbit TCP)
    let config = ClientConfig::default();
    let client = RibbitTactClient::new(config)?;

    // Step 2: Query version information
    // The same API works on both native and WASM
    log("Querying wow_classic versions...");
    let versions = client.query("v1/products/wow_classic/versions").await?;

    log(&format!("Found {} region(s):\n", versions.rows().len()));

    // Extract and display version information
    let mut build_config_hash = String::new();
    let mut cdn_config_hash = String::new();

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

        // Store first region's config hashes for later download
        if build_config_hash.is_empty() {
            if let Some(hash) = row
                .get_by_name("BuildConfig", versions.schema())
                .and_then(|v| v.as_string())
            {
                build_config_hash = hash.to_string();
            }
            if let Some(hash) = row
                .get_by_name("CDNConfig", versions.schema())
                .and_then(|v| v.as_string())
            {
                cdn_config_hash = hash.to_string();
            }
        }

        log(&format!("  Region: {region}"));
        log(&format!("    Version: {version_name}"));
        log(&format!("    Build ID: {build_id}"));
    }

    // Step 3: Query CDN endpoints
    log("\nQuerying CDN endpoints...");
    let cdns = client.query("v1/products/wow_classic/cdns").await?;

    log(&format!(
        "Found {} CDN configuration(s):\n",
        cdns.rows().len()
    ));

    for row in cdns.rows() {
        let name = row
            .get_by_name("Name", cdns.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        let path = row
            .get_by_name("Path", cdns.schema())
            .and_then(|v| v.as_string())
            .unwrap_or("unknown");

        log(&format!("  {name}: path={path}"));
    }

    // Step 4: Extract CDN endpoint and create CDN client
    let cdn_endpoint = CdnClient::endpoint_from_bpsv_row(
        cdns.rows().first().ok_or("No CDN found")?,
        cdns.schema(),
    )?;

    log(&format!("\nUsing CDN: {}", cdn_endpoint.host));

    let cdn_client = CdnClient::new(client.cache().clone(), CdnConfig::default())?;

    // Step 5: Download build configuration
    if !build_config_hash.is_empty() {
        log(&format!(
            "\nDownloading build config ({build_config_hash})..."
        ));

        let build_config_key = hex::decode(&build_config_hash)?;

        // On WASM, download_with_progress does a full download (no streaming)
        // but still provides progress callback at completion
        let build_config_data = cdn_client
            .download_with_progress(
                &cdn_endpoint,
                ContentType::Config,
                &build_config_key,
                #[allow(clippy::cast_precision_loss)] // Precision loss acceptable for progress %
                |downloaded, total| {
                    let pct = if total > 0 {
                        (downloaded as f64 / total as f64) * 100.0
                    } else {
                        100.0
                    };
                    log(&format!("  Progress: {pct:.0}%"));
                },
            )
            .await?;

        log(&format!("  Downloaded {} bytes", build_config_data.len()));

        // Parse and display key information
        let config_text = String::from_utf8_lossy(&build_config_data);
        log("\n=== Build Configuration ===\n");

        for line in config_text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once(" = ") {
                match key {
                    "root" => log(&format!("Root CKey: {value}")),
                    "encoding" => {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 2 {
                            log(&format!("Encoding EKey: {}", parts[1]));
                        }
                    }
                    "install" => {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 2 {
                            log(&format!("Install EKey: {}", parts[1]));
                        }
                    }
                    "build-name" => log(&format!("Build Name: {value}")),
                    _ => {}
                }
            }
        }
    }

    // Step 6: Download CDN configuration
    if !cdn_config_hash.is_empty() {
        log(&format!("\nDownloading CDN config ({cdn_config_hash})..."));

        let cdn_config_key = hex::decode(&cdn_config_hash)?;
        let cdn_config_data = cdn_client
            .download(&cdn_endpoint, ContentType::Config, &cdn_config_key)
            .await?;

        log(&format!("  Downloaded {} bytes", cdn_config_data.len()));

        // Parse archive count
        let config_text = String::from_utf8_lossy(&cdn_config_data);
        for line in config_text.lines() {
            if let Some(value) = line.strip_prefix("archives = ") {
                let count = value.split_whitespace().count();
                log(&format!("  Archives available: {count}"));
                break;
            }
        }
    }

    // Step 7: Show what would come next
    log("\n=== Next Steps for Installation ===\n");
    log("1. Download encoding file using EKey from build config");
    log("2. Download root file (need encoding file to resolve EKey)");
    log("3. Download install manifest for file list");
    log("4. Resolve files: FileDataID -> Root -> Encoding -> CDN");
    log("5. Download required files from CDN archives");

    log("\n=== WASM Limitations ===\n");
    log("- No Ribbit TCP (browsers lack raw socket access)");
    log("- No streaming downloads (full download with progress)");
    log("- Cache uses localStorage (~5-10MB browser limit)");
    log("- For large downloads, use IndexedDB via cascette-cache");

    Ok(())
}

/// Cross-platform logging function
///
/// On native: prints to stdout
/// On WASM: logs to browser console
fn log(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&msg.into());

    #[cfg(not(target_arch = "wasm32"))]
    println!("{msg}");
}

/// Native test runner
///
/// Allows testing the WASM-compatible code on native platforms.
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("cascette_protocol=info")
        .init();

    query_wow_classic().await
}

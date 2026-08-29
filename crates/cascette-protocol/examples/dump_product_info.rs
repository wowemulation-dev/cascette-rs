//! Dump full product information from Ribbit and CDN configs.
//!
//! This replicates BuildBackup's `dumpinfo` command: query Ribbit for the
//! live version and CDN data, then download and parse the build config and
//! CDN config to show the complete picture of a build.
//!
//! Reference tools:
//! - BuildBackup: `BuildBackup dumpinfo <product> <buildconfig> <cdnconfig>`
//! - TACTSharp:   `TACTTool --product wow_classic_era --mode list`
//!
//! Usage:
//!   cargo run -p cascette-protocol --example dump_product_info
//!   cargo run -p cascette-protocol --example dump_product_info -- wow_classic_era us

use std::error::Error;

use cascette_formats::bpsv::{BpsvRow, BpsvSchema};
use cascette_protocol::{
    CdnClient, CdnConfig, CdnEndpoint, ClientConfig, ContentType, RibbitTactClient,
};

/// Extract a hash field from a BPSV row as a lowercase hex string.
///
/// BPSV columns typed `HEX:N` (like BuildConfig, CDNConfig) parse as
/// `BpsvValue::Hex`. Columns typed `STRING:0` parse as `BpsvValue::String`.
/// This helper handles both cases.
fn field_as_hex(row: &BpsvRow, name: &str, schema: &BpsvSchema) -> Option<String> {
    let val = row.get_by_name(name, schema)?;
    // Hash fields arrive as Hex bytes; fall back to String for flexibility.
    if let Some(bytes) = val.as_hex() {
        Some(hex::encode(bytes))
    } else {
        val.as_string().map(str::to_string)
    }
}

/// Extract a string field from a BPSV row.
fn field_as_str<'a>(row: &'a BpsvRow, name: &str, schema: &BpsvSchema) -> Option<&'a str> {
    row.get_by_name(name, schema)?.as_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("cascette_protocol=warn")
        .init();

    let args: Vec<String> = std::env::args().collect();
    let product = args.get(1).map_or("wow_classic_era", String::as_str);
    let region = args.get(2).map_or("us", String::as_str);

    println!("=== Product Info Dump ===");
    println!("Product: {product}");
    println!("Region:  {region}");
    println!();

    // ── Step 1: Query Ribbit for versions ─────────────────────────────────
    // This is the entry point for every TACT workflow: fetch the live version
    // manifest, which maps regions to (BuildConfig, CDNConfig, VersionName).
    // The BPSV schema for versions is:
    //   Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|...
    println!("Step 1: Querying Ribbit for {product}/versions ...");
    let config = ClientConfig::default();
    let client = RibbitTactClient::new(config)?;

    let versions = client
        .query(&format!("v1/products/{product}/versions"))
        .await?;

    println!("  Found {} region(s)", versions.rows().len());

    // Find the row for our target region
    let row = versions
        .rows()
        .iter()
        .find(|r| field_as_str(r, "Region", versions.schema()).is_some_and(|s| s == region))
        .ok_or_else(|| format!("region '{region}' not found in versions"))?;

    // BuildConfig and CDNConfig are HEX:16 typed — use field_as_hex()
    let build_config =
        field_as_hex(row, "BuildConfig", versions.schema()).ok_or("BuildConfig field missing")?;

    let cdn_config =
        field_as_hex(row, "CDNConfig", versions.schema()).ok_or("CDNConfig field missing")?;

    // VersionsName is STRING:0; BuildId is DEC:4
    let version_name = field_as_str(row, "VersionsName", versions.schema())
        .unwrap_or("unknown")
        .to_string();

    let build_id = row
        .get_by_name("BuildId", versions.schema())
        .and_then(cascette_formats::bpsv::BpsvValue::as_dec)
        .map_or_else(|| "unknown".to_string(), |n| n.to_string());

    println!();
    println!("  Version:      {version_name}");
    println!("  Build ID:     {build_id}");
    println!("  BuildConfig:  {build_config}");
    println!("  CDNConfig:    {cdn_config}");

    // ── Step 2: Query Ribbit for CDN endpoints ────────────────────────────
    println!();
    println!("Step 2: Querying Ribbit for {product}/cdns ...");

    let cdns = client.query(&format!("v1/products/{product}/cdns")).await?;

    // CDN schema: Name!STRING:0|Path!STRING:0|Hosts!STRING:0|...
    // Pick the CDN row for our region (fall back to first available)
    let cdn_row = cdns
        .rows()
        .iter()
        .find(|r| field_as_str(r, "Name", cdns.schema()).is_some_and(|s| s == region))
        .or_else(|| cdns.rows().first())
        .ok_or("no CDN entries found")?;

    let cdn_path = field_as_str(cdn_row, "Path", cdns.schema())
        .ok_or("Path field missing")?
        .to_string();

    let cdn_hosts_raw = field_as_str(cdn_row, "Hosts", cdns.schema())
        .unwrap_or("")
        .to_string();

    let cdn_hosts: Vec<&str> = cdn_hosts_raw.split_whitespace().collect();
    println!("  CDN path:  {cdn_path}");
    println!("  CDN hosts: {} official", cdn_hosts.len());
    for h in &cdn_hosts {
        println!("    - {h}");
    }

    // Community mirrors for historical builds (when official CDN may have removed files)
    let community_mirrors = ["casc.wago.tools", "cdn.arctium.tools", "archive.wow.tools"];
    println!("  Community mirrors: {}", community_mirrors.len());

    // Build endpoint list: official first, then community fallbacks
    let mut endpoints: Vec<CdnEndpoint> = cdn_hosts
        .iter()
        .map(|h| CdnEndpoint {
            host: h.to_string(),
            path: cdn_path.clone(),
            product_path: None,
            scheme: Some("https".to_string()),
            is_fallback: false,
            strict: false,
            max_hosts: Some(4),
        })
        .collect();

    for mirror in &community_mirrors {
        endpoints.push(CdnEndpoint {
            host: mirror.to_string(),
            path: cdn_path.clone(),
            product_path: None,
            scheme: Some("https".to_string()),
            is_fallback: true,
            strict: false,
            max_hosts: None,
        });
    }

    // ── Step 3: Download and parse build config ───────────────────────────
    // The build config is a plaintext key=value file that holds references
    // to every manifest file (encoding, root, install, download) for the build.
    println!();
    println!("Step 3: Downloading build config ({build_config}) ...");

    let cdn_client = CdnClient::new(client.cache().clone(), CdnConfig::default())?;

    let build_config_key = hex::decode(&build_config)?;
    let primary_endpoint = endpoints[0].clone();

    let build_config_data = cdn_client
        .download(&primary_endpoint, ContentType::Config, &build_config_key)
        .await?;

    println!("  Downloaded {} bytes", build_config_data.len());

    // Parse the build config text
    let build_config_text = String::from_utf8_lossy(&build_config_data);
    println!();
    println!("  === Build Config Contents ===");

    let important_keys = [
        "build-name",
        "build-product",
        "build-uid",
        "encoding",
        "root",
        "install",
        "download",
        "patch",
    ];

    for line in build_config_text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(" = ")
            && important_keys.contains(&key)
        {
            // Multi-value fields (like "encoding") have format "CKey EKey"
            let parts: Vec<&str> = value.split_whitespace().collect();
            match parts.len() {
                0 => {}
                1 => println!("  {key}: {}", parts[0]),
                2 => println!("  {key}: ckey={} ekey={}", parts[0], parts[1]),
                _ => println!("  {key}: {} ({} values)", parts[0], parts.len()),
            }
        }
    }

    // ── Step 4: Download and parse CDN config ─────────────────────────────
    // The CDN config lists all archive files that make up the build's data.
    // BuildBackup's dumpinfo shows archive counts; TACTSharp uses this to
    // build its GroupIndex for file resolution.
    println!();
    println!("Step 4: Downloading CDN config ({cdn_config}) ...");

    let cdn_config_key = hex::decode(&cdn_config)?;
    let cdn_config_data = cdn_client
        .download(&primary_endpoint, ContentType::Config, &cdn_config_key)
        .await?;

    println!("  Downloaded {} bytes", cdn_config_data.len());

    let cdn_config_text = String::from_utf8_lossy(&cdn_config_data);
    println!();
    println!("  === CDN Config Summary ===");

    for line in cdn_config_text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(" = ") {
            match key {
                "archives" => {
                    let archives: Vec<&str> = value.split_whitespace().collect();
                    println!("  Archives: {} total", archives.len());
                    if let Some(first) = archives.first() {
                        println!("    First: {first}");
                    }
                    if archives.len() > 1 {
                        println!("    Last:  {}", archives[archives.len() - 1]);
                    }
                }
                "archive-group" => println!("  Archive group: {value}"),
                "patch-archives" => {
                    let count = value.split_whitespace().count();
                    println!("  Patch archives: {count}");
                }
                "file-index" => println!("  File index: {value}"),
                "patch-file-index" => println!("  Patch file index: {value}"),
                _ => {}
            }
        }
    }

    // ── Summary ───────────────────────────────────────────────────────────
    println!();
    println!("=== Summary ===");
    println!("Product:     {product} ({region})");
    println!("Version:     {version_name} (build {build_id})");
    println!("BuildConfig: {build_config}");
    println!("CDNConfig:   {cdn_config}");
    println!("CDN path:    {cdn_path}");
    println!("Endpoints:   {}", endpoints.len());
    println!();
    println!("To extract files from this build:");
    println!("  1. Download the encoding file (EKey from build config)");
    println!("  2. Download the root file (CKey from build config)");
    println!("  3. Resolve: filename -> root -> CKey -> encoding -> EKey -> archive");

    Ok(())
}

//! CDN archive size reporting: download archive indices and report sizes.
//!
//! This replicates BuildBackup's `dumpsizes` command: connect to the CDN,
//! download archive index files (.index), parse them, and report the total
//! data size across all archives for a build.
//!
//! Archive indices are small metadata files (~few KB each) that describe
//! the contents of each archive. The actual archive files can be hundreds
//! of MB to GB each. This example only downloads the index files.
//!
//! Reference tools:
//! - BuildBackup: `dumpsizes <product> <buildconfig> <cdnconfig>`
//! - TACTSharp:   archive size summary in verify mode
//!
//! Usage:
//!   cargo run -p cascette-protocol --example cdn_archive_sizes
//!   cargo run -p cascette-protocol --example cdn_archive_sizes -- wow_classic_era us

use std::error::Error;

use cascette_formats::bpsv::{BpsvRow, BpsvSchema};
use cascette_protocol::{
    CdnClient, CdnConfig, CdnEndpoint, ClientConfig, ContentType, RibbitTactClient,
};

fn bpsv_hash_field(row: &BpsvRow, name: &str, schema: &BpsvSchema) -> Option<String> {
    let val = row.get_by_name(name, schema)?;
    if let Some(bytes) = val.as_hex() {
        Some(hex::encode(bytes))
    } else {
        val.as_string().map(str::to_string)
    }
}

fn bpsv_str<'a>(row: &'a BpsvRow, name: &str, schema: &'a BpsvSchema) -> Option<&'a str> {
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
    // Limit number of archive indices to download (full builds have 1000+)
    let max_archives: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    println!("=== CDN Archive Sizes ===");
    println!("Product:       {product}");
    println!("Region:        {region}");
    println!("Max archives:  {max_archives} (pass a 4th arg to change)");
    println!();

    // ── Step 1: Resolve version and CDN info via Ribbit ───────────────────
    let config = ClientConfig::default();
    let client = RibbitTactClient::new(config)?;

    let versions = client
        .query(&format!("v1/products/{product}/versions"))
        .await?;

    let row = versions
        .rows()
        .iter()
        .find(|r| bpsv_str(r, "Region", versions.schema()).is_some_and(|s| s == region))
        .or_else(|| versions.rows().first())
        .ok_or("no version rows")?;

    let cdn_config_hash =
        bpsv_hash_field(row, "CDNConfig", versions.schema()).ok_or("CDNConfig missing")?;

    let version_name = bpsv_str(row, "VersionsName", versions.schema()).unwrap_or("unknown");

    println!("Version:   {version_name}");
    println!("CDNConfig: {cdn_config_hash}");

    // ── Step 2: Resolve CDN endpoints ─────────────────────────────────────
    let cdns = client.query(&format!("v1/products/{product}/cdns")).await?;

    let cdn_row = cdns
        .rows()
        .iter()
        .find(|r| bpsv_str(r, "Name", cdns.schema()).is_some_and(|s| s == region))
        .or_else(|| cdns.rows().first())
        .ok_or("no CDN rows")?;

    let cdn_path = bpsv_str(cdn_row, "Path", cdns.schema())
        .ok_or("Path missing")?
        .to_string();

    let cdn_hosts_raw = bpsv_str(cdn_row, "Hosts", cdns.schema()).unwrap_or("");
    let cdn_hosts: Vec<&str> = cdn_hosts_raw.split_whitespace().collect();

    // Use community mirrors as fallback
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

    for mirror in &["casc.wago.tools", "cdn.arctium.tools", "archive.wow.tools"] {
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

    let cdn_client = CdnClient::new(client.cache().clone(), CdnConfig::default())?;
    let primary = endpoints[0].clone();

    // ── Step 3: Download CDN config to get archive list ────────────────────
    let cdn_config_key = hex::decode(&cdn_config_hash)?;
    let cdn_config_data = cdn_client
        .download(&primary, ContentType::Config, &cdn_config_key)
        .await?;

    let cdn_config_text = String::from_utf8_lossy(&cdn_config_data);

    // Parse archive hashes from CDN config
    let archives: Vec<&str> = cdn_config_text
        .lines()
        .find(|l| l.starts_with("archives = "))
        .and_then(|l| l.strip_prefix("archives = "))
        .map(|v| v.split_whitespace().collect())
        .unwrap_or_default();

    println!("Total archives in build: {}", archives.len());
    println!();

    // ── Step 4: Download archive indices and sum sizes ─────────────────────
    // Each archive has a corresponding .index file at:
    //   {cdn_path}/data/{archive_hash[0..2]}/{archive_hash[2..4]}/{archive_hash}.index
    //
    // The index file contains entries showing the size of each file chunk
    // stored in the archive. Summing these gives the archive's total data size.
    // BuildBackup's dumpsizes does exactly this: download all indices, sum up.
    println!(
        "Downloading first {} archive indices ...",
        max_archives.min(archives.len())
    );
    println!();

    let mut total_entries: u64 = 0;
    let mut total_data_bytes: u64 = 0;
    let sample_count = max_archives.min(archives.len());

    println!(
        "  {:<32}  {:>8}  {:>15}",
        "Archive hash", "Entries", "Data size"
    );
    println!("  {:-<32}  {:->8}  {:->15}", "", "", "");

    for archive_hash in archives.iter().take(sample_count) {
        // Download the archive index (.index file)
        let index_data = cdn_client
            .download_archive_index(&primary, archive_hash)
            .await;

        match index_data {
            Ok(data) => {
                // Parse the index: each entry is 18 bytes (9-byte key, 5-byte location, 4-byte size)
                // The last 28 bytes are the footer (skip them)
                let entry_data_len = data.len().saturating_sub(28);
                let entry_count = entry_data_len / 18;
                let data_size: u64 = (0..entry_count)
                    .map(|i| {
                        let offset = i * 18 + 14; // size is last 4 bytes of each 18-byte entry
                        if offset + 4 <= data.len() {
                            // Size is little-endian u32 (last 4 bytes of 18-byte entry)
                            u64::from(u32::from_le_bytes([
                                data[offset],
                                data[offset + 1],
                                data[offset + 2],
                                data[offset + 3],
                            ]))
                        } else {
                            0
                        }
                    })
                    .sum();

                total_entries += entry_count as u64;
                total_data_bytes += data_size;

                println!(
                    "  {archive_hash:<32}  {:>8}  {:>15}",
                    entry_count,
                    format_bytes(data_size)
                );
            }
            Err(e) => {
                println!("  {archive_hash:<32}  [error: {e}]");
            }
        }
    }

    if archives.len() > sample_count {
        println!(
            "  ... ({} more archives not shown)",
            archives.len() - sample_count
        );
    }

    println!();
    println!(
        "=== Size Summary ({sample_count} of {} archives sampled) ===",
        archives.len()
    );
    println!("  Sampled entries:    {total_entries}");
    println!("  Sampled data size:  {}", format_bytes(total_data_bytes));

    if sample_count < archives.len() {
        // Extrapolate full build size
        #[allow(clippy::cast_precision_loss)]
        let estimated_total =
            total_data_bytes as f64 * (archives.len() as f64 / sample_count as f64);
        println!(
            "  Estimated full build: {} (extrapolated)",
            format_bytes(estimated_total as u64)
        );
    }

    println!();
    println!("=== Reference Tool Equivalence ===");
    println!("  BuildBackup dumpsizes -> cdn_client.download_archive_index() per archive");
    println!("  TACTSharp verify      -> same index download + entry validation");

    Ok(())
}

/// Format byte counts as human-readable strings.
#[allow(clippy::cast_precision_loss)]
fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    const KB: u64 = 1_024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

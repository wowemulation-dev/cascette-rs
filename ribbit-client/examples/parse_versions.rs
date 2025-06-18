//! Example showing how to parse version information from Ribbit
//!
//! This example demonstrates:
//! - Fetching version data
//! - Parsing PSV format
//! - Extracting specific fields
//!
//! Run with: `cargo run --example parse_versions`

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    println!("Fetching WoW version information...\n");

    let response = client
        .request_raw(&Endpoint::ProductVersions("wow".to_string()))
        .await?;

    let data = String::from_utf8(response)?;

    // Parse PSV format
    let lines: Vec<&str> = data.lines().collect();
    if lines.len() < 2 {
        println!("No version data found");
        return Ok(());
    }

    // Parse headers
    let headers: Vec<&str> = lines[0]
        .split('|')
        .map(|h| h.split('!').next().unwrap_or(h))
        .collect();

    println!("Headers: {:?}\n", headers);

    // Find sequence number
    let seqn = lines
        .iter()
        .find(|line| line.starts_with("## seqn = "))
        .and_then(|line| line.strip_prefix("## seqn = "))
        .unwrap_or("unknown");

    println!("Sequence number: {}\n", seqn);

    // Parse version data for each region
    println!("Version information by region:");
    println!("{:-<60}", "");

    for line in lines.iter().skip(1) {
        if line.starts_with("##") || line.is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() >= headers.len() {
            let mut record: HashMap<&str, &str> = HashMap::new();
            for (i, header) in headers.iter().enumerate() {
                record.insert(header, fields[i]);
            }

            // Display formatted information
            if let (Some(region), Some(build_id), Some(version)) = (
                record.get("Region"),
                record.get("BuildId"),
                record.get("VersionsName"),
            ) {
                println!(
                    "Region: {:6} | Build: {:6} | Version: {}",
                    region, build_id, version
                );
            }
        }
    }

    println!("{:-<60}", "");

    Ok(())
}

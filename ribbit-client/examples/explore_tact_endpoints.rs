//! Explore TACT-related endpoints to understand data formats
//!
//! This example tests various endpoints to see which ones return BPSV format

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    println!("=== Exploring TACT/CDN Endpoints ===\n");

    let endpoints_to_test = [
        (
            "Product Versions (WoW)",
            Endpoint::ProductVersions("wow".to_string()),
        ),
        (
            "Product CDNs (WoW)",
            Endpoint::ProductCdns("wow".to_string()),
        ),
        (
            "Product BGDL (WoW)",
            Endpoint::ProductBgdl("wow".to_string()),
        ),
        ("Summary", Endpoint::Summary),
        (
            "Product Versions (Agent)",
            Endpoint::ProductVersions("agent".to_string()),
        ),
        (
            "Product CDNs (Agent)",
            Endpoint::ProductCdns("agent".to_string()),
        ),
    ];

    for (name, endpoint) in &endpoints_to_test {
        println!("🔍 Testing: {}", name);
        println!("   Endpoint: {}", endpoint.as_path());

        match client.request(endpoint).await {
            Ok(response) => {
                if let Some(data) = &response.data {
                    let lines: Vec<&str> = data.lines().take(5).collect();
                    println!("   Status: ✅ Success");
                    println!("   Data length: {} bytes", data.len());
                    println!("   Format analysis:");

                    // Check if it looks like BPSV
                    let is_bpsv = data.contains('|')
                        && (data.contains("!STRING")
                            || data.contains("!HEX")
                            || data.contains("!DEC"));
                    let has_seqn = data.contains("## seqn");

                    if is_bpsv {
                        println!("   📊 Format: BPSV (Pipe-Separated Values)");
                        if has_seqn {
                            // Extract sequence number
                            if let Some(seqn_line) =
                                data.lines().find(|line| line.starts_with("## seqn"))
                            {
                                println!("   📈 {}", seqn_line);
                            }
                        }

                        // Show headers
                        if let Some(header_line) = data.lines().find(|line| line.contains('!')) {
                            let headers: Vec<&str> = header_line
                                .split('|')
                                .map(|h| h.split('!').next().unwrap_or(h))
                                .collect();
                            println!("   📋 Headers: {:?}", headers);
                        }

                        // Count data rows (non-header, non-comment lines)
                        let data_rows = data
                            .lines()
                            .filter(|line| {
                                !line.trim().is_empty()
                                    && !line.starts_with("##")
                                    && !line.contains('!')
                            })
                            .count();
                        println!("   📊 Data rows: {}", data_rows);
                    } else {
                        println!("   📄 Format: Unknown/Plain text");
                    }

                    println!("   Sample (first 5 lines):");
                    for (i, line) in lines.iter().enumerate() {
                        println!("     {}: {}", i + 1, line);
                    }
                } else {
                    println!("   Status: ⚠️  No data returned");
                }
            }
            Err(e) => {
                println!("   Status: ❌ Error: {}", e);
            }
        }
        println!();
    }

    println!("📋 Summary:");
    println!("- BPSV format is used for structured data with pipe-separated values");
    println!("- Headers contain type information (STRING, HEX, DEC)");
    println!("- Sequence numbers track data versions");
    println!("- This format is critical for TACT/CDN configuration data");

    Ok(())
}

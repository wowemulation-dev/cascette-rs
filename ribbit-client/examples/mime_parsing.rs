//! Example demonstrating MIME parsing in Ribbit V1 responses
//!
//! This example shows how the client parses MIME responses and
//! validates checksums for V1 protocol responses.
//!
//! Run with: `cargo run --example mime_parsing`

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    let client = RibbitClient::new(Region::US);

    println!("Ribbit V1 MIME Parsing Examples");
    println!("{:=<60}\n", "");

    // Example 1: Certificate endpoint (simple MIME)
    println!("1. Fetching certificate (simple MIME structure)...");
    let cert_endpoint = Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string());

    match client.request(&cert_endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            if let Some(mime_parts) = response.mime_parts {
                println!("✓ MIME parsed successfully");

                // Display checksum
                if let Some(checksum) = mime_parts.checksum {
                    println!(
                        "  Checksum: {}...{}",
                        &checksum[..8],
                        &checksum[checksum.len() - 8..]
                    );
                    println!("  Checksum validation: PASSED");
                }

                // Display certificate info
                if mime_parts.data.contains("-----BEGIN CERTIFICATE-----") {
                    println!("  Certificate: Found PEM-encoded certificate");

                    // Extract subject info if possible
                    if let Some(cn_start) = mime_parts.data.find("CN=") {
                        let cn_end = mime_parts.data[cn_start..]
                            .find(',')
                            .or_else(|| mime_parts.data[cn_start..].find('\n'))
                            .unwrap_or(30);
                        let cn = &mime_parts.data[cn_start..cn_start + cn_end];
                        println!("  Subject: {}", cn);
                    }
                }
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    // Example 2: Product versions (multipart MIME with signature)
    println!("\n2. Fetching product versions (multipart MIME)...");
    let versions_endpoint = Endpoint::ProductVersions("wow".to_string());

    match client.request(&versions_endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            if let Some(mime_parts) = response.mime_parts {
                println!("✓ MIME parsed successfully");

                // Display checksum
                if let Some(checksum) = mime_parts.checksum {
                    println!(
                        "  Checksum: {}...{}",
                        &checksum[..8],
                        &checksum[checksum.len() - 8..]
                    );
                }

                // Display signature info
                if let Some(signature) = mime_parts.signature {
                    println!("  Signature: {} bytes (ASN.1 format)", signature.len());
                }

                // Display data preview
                if !mime_parts.data.is_empty() {
                    let lines: Vec<&str> = mime_parts.data.lines().take(3).collect();
                    println!("  Data preview:");
                    for line in lines {
                        println!("    {}", line);
                    }
                }
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    // Example 3: Summary endpoint
    println!("\n3. Fetching summary (all products)...");
    let summary_endpoint = Endpoint::Summary;

    match client.request(&summary_endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            if let Some(mime_parts) = response.mime_parts {
                println!("✓ MIME parsed successfully");

                // Count products
                let product_count = mime_parts
                    .data
                    .lines()
                    .filter(|line| line.contains('|') && !line.starts_with('#'))
                    .count();
                println!("  Products found: {}", product_count);

                // Show WoW-related products
                println!("  WoW products:");
                for line in mime_parts.data.lines() {
                    if line.contains("wow") && line.contains('|') {
                        let parts: Vec<&str> = line.split('|').collect();
                        if parts.len() >= 3 {
                            println!("    - {} (seqn: {})", parts[0], parts[1]);
                        }
                    }
                }
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    // Example 4: Demonstrate raw vs parsed access
    println!("\n4. Comparing raw vs parsed data...");
    let endpoint = Endpoint::ProductCdns("agent".to_string());

    match client.request(&endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            // Raw data includes full MIME structure
            println!("  Raw response size: {} bytes", response.raw.len());

            // Parsed data contains just the content
            if let Some(data) = &response.data {
                println!("  Parsed data size: {} bytes", data.len());
                println!(
                    "  Size reduction: {:.1}%",
                    (1.0 - data.len() as f32 / response.raw.len() as f32) * 100.0
                );
            }

            // MIME parts provide structured access
            if let Some(mime_parts) = &response.mime_parts {
                println!("  Checksum present: {}", mime_parts.checksum.is_some());
                println!("  Signature present: {}", mime_parts.signature.is_some());
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    println!("\n{:=<60}", "");
    Ok(())
}

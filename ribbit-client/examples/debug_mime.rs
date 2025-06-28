//! Debug MIME structure to see what's in the responses
//!
//! Run with: `cargo run --example debug_mime`

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable debug logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("Debugging MIME Structure in Ribbit V1 Responses");
    println!("{:=<60}\n", "");

    // Try different endpoints to find one with a signature
    let endpoints = vec![
        (
            "Product Versions",
            Endpoint::ProductVersions("wow".to_string()),
        ),
        ("Product CDNs", Endpoint::ProductCdns("wow".to_string())),
        ("Product BGDL", Endpoint::ProductBgdl("wow".to_string())),
        ("Summary", Endpoint::Summary),
    ];

    for (name, endpoint) in endpoints {
        println!("Fetching {name}...");

        match client.request(&endpoint).await {
            Ok(response) => {
                println!("✓ Response received ({} bytes)", response.raw.len());

                // Show raw response structure
                let raw_str = String::from_utf8_lossy(&response.raw);

                // Look for MIME boundaries
                if raw_str.contains("boundary=") {
                    println!("  MIME multipart detected");

                    // Extract boundary
                    if let Some(boundary_start) = raw_str.find("boundary=\"") {
                        let boundary_end = raw_str[boundary_start + 10..].find('"').unwrap_or(0);
                        let boundary =
                            &raw_str[boundary_start + 10..boundary_start + 10 + boundary_end];
                        println!("  Boundary: {boundary}");
                    }

                    // Count parts
                    let part_count = raw_str.matches("Content-Type:").count();
                    println!("  Parts found: {part_count}");

                    // Check for signature indicators
                    if raw_str.contains("Content-Disposition: signature") {
                        println!("  ✓ Signature part found!");

                        // Extract the signature part
                        if let Some(sig_start) = raw_str.find("Content-Disposition: signature") {
                            let content_start = raw_str[sig_start..]
                                .find("\r\n\r\n")
                                .map(|i| sig_start + i + 4);
                            if let Some(start) = content_start {
                                let next_boundary = raw_str[start..].find("--").unwrap_or(100);
                                let sig_preview = &raw_str[start..start + next_boundary.min(100)];
                                println!("  Signature preview: {sig_preview:?}");
                            }
                        }
                    }
                }

                // Check parsed MIME parts
                if let Some(mime_parts) = &response.mime_parts {
                    println!("\n  Parsed MIME parts:");
                    println!("    Data length: {} bytes", mime_parts.data.len());
                    println!(
                        "    Signature: {}",
                        if mime_parts.signature.is_some() {
                            "Present"
                        } else {
                            "None"
                        }
                    );
                    println!(
                        "    Checksum: {}",
                        mime_parts
                            .checksum
                            .as_ref()
                            .map(|c| &c[..8])
                            .unwrap_or("None")
                    );

                    if let Some(sig) = &mime_parts.signature {
                        println!("\n  Signature details:");
                        println!("    Size: {} bytes", sig.len());
                        println!("    First 32 bytes: {:02x?}", &sig[..sig.len().min(32)]);

                        if let Some(info) = &mime_parts.signature_info {
                            println!("    Parsed info: {info:?}");
                        }
                    }
                }
            }
            Err(e) => println!("✗ Error: {e}"),
        }

        println!("\n{:-<60}\n", "");
    }

    Ok(())
}

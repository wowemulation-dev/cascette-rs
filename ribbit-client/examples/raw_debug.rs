//! Debug raw MIME structure
//!
//! Run with: `cargo run --example raw_debug`

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US);

    println!("Raw MIME Debug");
    println!("{:=<60}\n", "");

    // Get product versions which should have a signature
    let endpoint = Endpoint::ProductVersions("wow".to_string());

    match client.request_raw(&endpoint).await {
        Ok(raw_bytes) => {
            println!("Raw response: {} bytes", raw_bytes.len());

            // Convert to string for analysis
            let raw_str = String::from_utf8_lossy(&raw_bytes);

            // Find MIME boundaries
            if let Some(boundary_pos) = raw_str.find("boundary=") {
                let boundary_start = boundary_pos + 10; // Skip 'boundary="'
                let boundary_end = raw_str[boundary_start..].find('"').unwrap_or(20);
                let boundary = &raw_str[boundary_start..boundary_start + boundary_end];
                println!("Boundary: {}", boundary);

                // Split by boundary
                let parts: Vec<&str> = raw_str.split(&format!("--{}", boundary)).collect();
                println!("Found {} parts", parts.len());

                for (i, part) in parts.iter().enumerate() {
                    println!("\nPart {}:", i);

                    // Find headers
                    if let Some(header_end) = part.find("\r\n\r\n") {
                        let headers = &part[..header_end];
                        println!("Headers:\n{}", headers);

                        // Check if this is the signature part
                        if headers.contains("signature") {
                            let content = &part[header_end + 4..];
                            let content_preview = if content.len() > 100 {
                                &content[..100]
                            } else {
                                content
                            };
                            println!("Signature content (first 100 chars):\n{}", content_preview);

                            // Check if it's base64
                            let trimmed = content.trim();
                            let is_base64 = trimmed.chars().all(|c| {
                                c.is_ascii_alphanumeric()
                                    || c == '+'
                                    || c == '/'
                                    || c == '='
                                    || c.is_whitespace()
                            });
                            println!("Appears to be base64: {}", is_base64);
                        }
                    }
                }
            }

            // Also check the parsed response
            match client.request(&endpoint).await {
                Ok(response) => {
                    if let Some(mime_parts) = response.mime_parts {
                        println!("\nParsed MIME parts:");
                        println!("  Has signature: {}", mime_parts.signature.is_some());
                        if let Some(sig) = &mime_parts.signature {
                            println!("  Signature size: {} bytes", sig.len());
                            println!("  First 16 bytes: {:02x?}", &sig[..16.min(sig.len())]);
                        }
                    }
                }
                Err(e) => println!("Parse error: {}", e),
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    Ok(())
}

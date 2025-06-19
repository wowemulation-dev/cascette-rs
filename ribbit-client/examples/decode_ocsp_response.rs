//! Decode and analyze the OCSP response from Ribbit
//!
//! This decodes the Base64 OCSP response and analyzes its contents.

use base64::Engine;
use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("=== OCSP Response Decoding ===\n");

    let ski = "782a8a710b950421127250a3e91b751ca356e202";
    let ocsp_endpoint = Endpoint::Ocsp(ski.to_string());

    match client.request(&ocsp_endpoint).await {
        Ok(response) => {
            if let Some(_mime_parts) = &response.mime_parts {
                if let Some(data) = &response.data {
                    // Extract base64 content from the MIME part
                    let lines: Vec<&str> = data.lines().collect();
                    let mut in_base64 = false;
                    let mut base64_lines = Vec::new();

                    for line in lines {
                        if line.trim().is_empty() && !in_base64 {
                            in_base64 = true;
                            continue;
                        }
                        if in_base64 && line.starts_with("--") {
                            break;
                        }
                        if in_base64 {
                            base64_lines.push(line);
                        }
                    }

                    let base64_clean = base64_lines.join("");

                    if !base64_clean.is_empty() {
                        println!(
                            "Base64 OCSP response extracted: {} chars",
                            base64_clean.len()
                        );

                        // Decode from base64
                        match base64::engine::general_purpose::STANDARD.decode(&base64_clean) {
                            Ok(ocsp_der) => {
                                println!("✓ Decoded OCSP response: {} bytes", ocsp_der.len());
                                analyze_ocsp_response(&ocsp_der, ski)?;
                            }
                            Err(e) => {
                                println!("✗ Failed to decode base64: {}", e);
                            }
                        }
                    } else {
                        println!("No base64 content found in response");
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ OCSP request failed: {}", e);
        }
    }

    println!("\n📝 Summary:");
    println!("The OCSP endpoint returns standard OCSP responses in ASN.1 DER format,");
    println!("wrapped in Ribbit's MIME multipart format. The response confirms the");
    println!("certificate status for the given SKI.");

    Ok(())
}

fn analyze_ocsp_response(ocsp_der: &[u8], ski: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Analyze the DER structure
    println!("\nDER Analysis:");
    if ocsp_der.starts_with(&[0x30, 0x82]) {
        println!("  ✓ Valid ASN.1 SEQUENCE");

        // OCSP Response structure starts with responseStatus
        if ocsp_der.len() > 6 && ocsp_der[4] == 0x0a && ocsp_der[5] == 0x01 {
            let status = ocsp_der[6];
            println!(
                "  Response Status: {}",
                match status {
                    0 => "successful (0)",
                    1 => "malformedRequest (1)",
                    2 => "internalError (2)",
                    3 => "tryLater (3)",
                    5 => "sigRequired (5)",
                    6 => "unauthorized (6)",
                    _ => "unknown",
                }
            );
        }

        // Look for certificate ID (the SKI we queried)
        let ski_bytes = hex::decode(ski)?;
        if let Some(pos) = find_bytes(ocsp_der, &ski_bytes) {
            println!("  ✓ Found SKI in response at position {}", pos);
        }

        // Look for certificate status
        // CertStatus is an implicit tag [0] for good, [1] for revoked
        for i in 0..ocsp_der.len() - 1 {
            if ocsp_der[i] == 0x80 && ocsp_der[i + 1] == 0x00 {
                println!("  ✓ Certificate Status: GOOD (not revoked)");
                break;
            } else if ocsp_der[i] == 0xa1 {
                println!("  ❌ Certificate Status: REVOKED");
                break;
            }
        }

        // Look for timestamps (GeneralizedTime format)
        find_timestamps(ocsp_der);

        // Show hex dump of first 100 bytes
        println!("\nFirst 100 bytes (hex):");
        let hex_preview: Vec<String> = ocsp_der[..ocsp_der.len().min(100)]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        for chunk in hex_preview.chunks(16) {
            println!("  {}", chunk.join(" "));
        }
    }

    Ok(())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn find_timestamps(data: &[u8]) {
    // GeneralizedTime starts with tag 0x18
    for i in 0..data.len() - 15 {
        if data[i] == 0x18 && data[i + 1] == 0x0f {
            // GeneralizedTime is 15 bytes for YYYYMMDDHHMMSSz
            let timestamp = &data[i + 2..i + 17];
            if let Ok(ts_str) = std::str::from_utf8(timestamp) {
                if ts_str.ends_with('Z') {
                    println!("  Timestamp found: {}", ts_str);
                }
            }
        }
    }
}

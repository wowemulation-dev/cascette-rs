//! Check what the OCSP endpoint returns for a SKI
//!
//! OCSP (Online Certificate Status Protocol) is typically used to check
//! if a certificate has been revoked.

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("=== OCSP Endpoint Test ===\n");

    // The SKI we found from signatures
    let ski = "782a8a710b950421127250a3e91b751ca356e202";
    println!("Testing SKI with OCSP endpoint: {}\n", ski);

    // Create OCSP endpoint
    let ocsp_endpoint = Endpoint::Ocsp(ski.to_string());
    println!("Endpoint: {}", ocsp_endpoint.as_path());

    // First try raw request to see what we get
    println!("\n1. Raw request (no checksum validation):");
    match client.request_raw(&ocsp_endpoint).await {
        Ok(raw_response) => {
            println!("✓ Response received: {} bytes", raw_response.len());

            // Check if it's text or binary
            let response_str = String::from_utf8_lossy(&raw_response);

            // Check for common OCSP response patterns
            if raw_response.starts_with(&[0x30]) {
                println!("  Looks like ASN.1/DER encoded data (starts with SEQUENCE tag)");

                // OCSP responses are typically DER-encoded
                // Try to parse basic structure
                println!("\n  First 32 bytes (hex):");
                let hex_preview: Vec<String> = raw_response[..raw_response.len().min(32)]
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                println!("  {}", hex_preview.join(" "));

                // Look for OCSP response OIDs
                // OCSP Response OID: 1.3.6.1.5.5.7.48.1.1
                let ocsp_basic_oid = vec![0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01, 0x01];
                if contains_sequence(&raw_response, &ocsp_basic_oid) {
                    println!("\n  ✓ Contains OCSP Basic Response OID (1.3.6.1.5.5.7.48.1.1)");
                    println!("  This is likely a valid OCSP response!");
                }

                // Try to find certificate status
                // OCSP statuses: good (0x80), revoked (0x81), unknown (0x82)
                if let Some(pos) = raw_response.iter().position(|&b| b == 0x80) {
                    if pos > 0 && raw_response[pos - 1] == 0x0a {
                        println!("\n  📋 Certificate Status: GOOD (not revoked)");
                    }
                }
            } else if response_str.contains("Region") || response_str.contains("BuildConfig") {
                println!("  ✗ Response contains version/build data, not OCSP");
                println!("\n  First 500 characters:");
                println!("  {}", &response_str[..response_str.len().min(500)]);
            } else if response_str.trim().is_empty() {
                println!("  ✗ Empty response");
            } else {
                println!("  Unknown response type");
                println!("\n  First 200 characters:");
                println!("  {}", &response_str[..response_str.len().min(200)]);
            }
        }
        Err(e) => {
            println!("✗ Raw request failed: {}", e);
        }
    }

    // Try normal request
    println!("\n2. Normal request (with checksum validation):");
    match client.request(&ocsp_endpoint).await {
        Ok(response) => {
            println!("✓ Request succeeded");
            if let Some(data) = &response.data {
                println!("  Data length: {} bytes", data.len());
            }
        }
        Err(e) => {
            println!("✗ Request failed: {}", e);
        }
    }

    // Also test with a known certificate hash to compare
    println!("\n3. Testing with known certificate fingerprint for comparison:");
    let known_cert = "5168ff90af0207753cccd9656462a212b859723b";
    let ocsp_known = Endpoint::Ocsp(known_cert.to_string());

    match client.request_raw(&ocsp_known).await {
        Ok(raw_response) => {
            println!(
                "✓ Response for {}: {} bytes",
                known_cert,
                raw_response.len()
            );

            if raw_response.starts_with(&[0x30]) {
                println!("  Also appears to be ASN.1/DER encoded");
            }
        }
        Err(e) => {
            println!("✗ Request failed: {}", e);
        }
    }

    println!("\n📝 Summary:");
    println!("The OCSP endpoint appears to accept both:");
    println!("- Subject Key Identifiers (SKI)");
    println!("- Certificate fingerprints");
    println!("And returns OCSP responses about certificate revocation status.");

    Ok(())
}

fn contains_sequence(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

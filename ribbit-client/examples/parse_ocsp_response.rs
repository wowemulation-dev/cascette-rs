//! Parse the OCSP endpoint response to understand its format
//!
//! It appears the OCSP endpoint returns MIME data, not standard OCSP responses.

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("=== OCSP Response Format Analysis ===\n");

    // The SKI we found from signatures
    let ski = "782a8a710b950421127250a3e91b751ca356e202";

    // Fetch OCSP response
    let ocsp_endpoint = Endpoint::Ocsp(ski.to_string());

    match client.request(&ocsp_endpoint).await {
        Ok(response) => {
            println!("✓ OCSP request succeeded\n");

            // Check if it's V1 MIME format
            if let Some(mime_parts) = &response.mime_parts {
                println!("Response is MIME formatted (V1 protocol)");

                if let Some(data) = &response.data {
                    println!("\nData content:");
                    println!("{}", data);

                    // Parse the response content
                    if data.contains("HTTP/1.1 200 OK") {
                        println!("\n📋 Analysis: This appears to be an OCSP response status!");

                        // Look for OCSP-specific information
                        if data.contains("Good") {
                            println!("  Certificate Status: GOOD ✓");
                        } else if data.contains("Revoked") {
                            println!("  Certificate Status: REVOKED ❌");
                        } else if data.contains("Unknown") {
                            println!("  Certificate Status: UNKNOWN ⚠️");
                        }

                        // Look for timestamps
                        if let Some(this_update) = find_field(data, "This Update:") {
                            println!("  This Update: {}", this_update);
                        }
                        if let Some(next_update) = find_field(data, "Next Update:") {
                            println!("  Next Update: {}", next_update);
                        }
                        if let Some(produced_at) = find_field(data, "Produced At:") {
                            println!("  Produced At: {}", produced_at);
                        }
                    }
                }

                // Check for signature
                if mime_parts.signature.is_some() {
                    println!("\n✓ Response is signed");
                }

                // Check for checksum
                if let Some(checksum) = &mime_parts.checksum {
                    println!("✓ Response has checksum: {}", checksum);
                }
            } else {
                println!("Response is not MIME formatted");
                if let Some(data) = &response.data {
                    println!("Raw data length: {} bytes", data.len());
                }
            }
        }
        Err(e) => {
            println!("✗ OCSP request failed: {}", e);
        }
    }

    // Compare with a regular certificate response
    println!("\n\n=== Comparison with Certificate Response ===\n");

    let cert_endpoint = Endpoint::Cert(ski.to_string());
    match client.request(&cert_endpoint).await {
        Ok(cert_response) => {
            if let Some(data) = &cert_response.data {
                if data.contains("-----BEGIN CERTIFICATE-----") {
                    println!("Certificate endpoint returns: PEM certificate");
                } else {
                    println!(
                        "Certificate endpoint returns: {}",
                        &data[..data.len().min(100)]
                    );
                }
            }
        }
        Err(e) => {
            println!("Certificate request failed: {}", e);
        }
    }

    println!("\n📝 Summary:");
    println!("The OCSP endpoint in Ribbit doesn't follow standard OCSP protocol.");
    println!("Instead, it returns MIME-formatted responses with certificate status information.");
    println!("This is consistent with Ribbit's custom protocol design.");

    Ok(())
}

fn find_field(data: &str, field_name: &str) -> Option<String> {
    if let Some(pos) = data.find(field_name) {
        let start = pos + field_name.len();
        if let Some(end) = data[start..].find('\n') {
            return Some(data[start..start + end].trim().to_string());
        }
    }
    None
}

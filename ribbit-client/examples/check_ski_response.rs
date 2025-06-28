//! Check what's actually returned when using SKI with certs endpoint
//!
//! This bypasses checksum validation to see the actual response.

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("=== Checking SKI Response Content ===\n");

    // The SKI we found from signatures
    let ski = "782a8a710b950421127250a3e91b751ca356e202";
    println!("Testing SKI: {ski}\n");

    // Make raw request to see what we get
    let endpoint = Endpoint::Cert(ski.to_string());

    match client.request_raw(&endpoint).await {
        Ok(raw_response) => {
            println!("Raw response received: {} bytes", raw_response.len());

            // Convert to string
            let response_str = String::from_utf8_lossy(&raw_response);

            // Check what type of content we got
            if response_str.contains("-----BEGIN CERTIFICATE-----") {
                println!("✓ Response contains a PEM certificate!");

                // Extract subject line if present
                if let Some(subject_start) = response_str.find("Subject:") {
                    if let Some(subject_end) = response_str[subject_start..].find('\n') {
                        let subject = &response_str[subject_start..subject_start + subject_end];
                        println!("  {subject}");
                    }
                }
            } else if response_str.contains("Region") || response_str.contains("BuildConfig") {
                println!("✗ Response contains version/build data, not a certificate");
                println!("\nFirst 500 characters:");
                println!("{}", &response_str[..response_str.len().min(500)]);
            } else {
                println!("✗ Unknown response type");
                println!("\nFirst 500 characters:");
                println!("{}", &response_str[..response_str.len().min(500)]);
            }

            // Look for any other SKIs or certificate references
            println!("\nSearching for other certificate references...");

            // Look for hex strings that might be certificate fingerprints
            let hex_pattern = regex::Regex::new(r"\b[a-fA-F0-9]{40}\b").unwrap();
            let matches: Vec<&str> = hex_pattern
                .find_iter(&response_str)
                .map(|m| m.as_str())
                .collect();

            if !matches.is_empty() {
                println!("Found potential certificate fingerprints:");
                for (i, fingerprint) in matches.iter().enumerate() {
                    if i < 5 {
                        // Show first 5
                        println!("  - {fingerprint}");
                    }
                }

                // Test if any of these work as certificate endpoints
                println!("\nTesting first fingerprint as certificate endpoint...");
                if let Some(first_fingerprint) = matches.first() {
                    let cert_endpoint = Endpoint::Cert(first_fingerprint.to_string());
                    match client.request(&cert_endpoint).await {
                        Ok(cert_response) => {
                            if let Some(data) = &cert_response.data {
                                if data.contains("-----BEGIN CERTIFICATE-----") {
                                    println!(
                                        "✓ {first_fingerprint} is a valid certificate endpoint!"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            println!("✗ Failed to fetch {first_fingerprint}: {e}");
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ Request failed: {e}");
        }
    }

    Ok(())
}

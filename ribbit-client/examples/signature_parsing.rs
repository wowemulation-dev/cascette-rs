//! Example demonstrating ASN.1 signature parsing in Ribbit V1 responses
//!
//! This example shows how the client parses ASN.1 signatures from MIME
//! messages and extracts basic information about them.
//!
//! Run with: `cargo run --example signature_parsing`

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    let client = RibbitClient::new(Region::US);

    println!("Ribbit V1 Signature Parsing Examples");
    println!("{:=<60}\n", "");

    // Example 1: Product versions with signature
    println!("1. Fetching product versions (usually contains signature)...");
    let versions_endpoint = Endpoint::ProductVersions("wow".to_string());

    match client.request(&versions_endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            if let Some(mime_parts) = response.mime_parts {
                if let Some(signature) = mime_parts.signature {
                    println!("✓ Signature found: {} bytes", signature.len());

                    if let Some(sig_info) = mime_parts.signature_info {
                        println!("\nSignature Information:");
                        println!("  Format: {}", sig_info.format);
                        println!("  Size: {} bytes", sig_info.size);
                        println!("  Algorithm: {}", sig_info.algorithm);
                        println!("  Signers: {}", sig_info.signer_count);
                        println!("  Certificates: {}", sig_info.certificate_count);
                    } else {
                        println!("  ⚠ Signature parsing failed or not implemented");
                    }

                    // Show hex dump of first few bytes
                    println!("\n  Signature bytes (first 32):");
                    let preview = &signature[..signature.len().min(32)];
                    println!("    {:02x?}", preview);
                } else {
                    println!("✗ No signature found in response");
                }

                // Also show checksum if present
                if let Some(checksum) = mime_parts.checksum {
                    println!("\n  SHA-256 Checksum: {}", checksum);
                }
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    // Example 2: CDN configuration (may or may not have signature)
    println!("\n2. Fetching CDN configuration...");
    let cdn_endpoint = Endpoint::ProductCdns("wow".to_string());

    match client.request(&cdn_endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            if let Some(mime_parts) = response.mime_parts {
                match mime_parts.signature {
                    Some(sig) => {
                        println!("✓ Signature present: {} bytes", sig.len());
                        if let Some(info) = mime_parts.signature_info {
                            println!("  Format: {}", info.format);
                        }
                    }
                    None => println!("  No signature in this response"),
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
                match mime_parts.signature {
                    Some(sig) => println!("✓ Signature present: {} bytes", sig.len()),
                    None => println!("  No signature in this response"),
                }

                // Count products in the data
                let product_count = mime_parts
                    .data
                    .lines()
                    .filter(|line| line.contains('|') && !line.starts_with('#'))
                    .count();
                println!("  Products listed: {}", product_count);
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    // Example 4: Certificate endpoint (typically no signature)
    println!("\n4. Fetching certificate...");
    let cert_endpoint = Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string());

    match client.request(&cert_endpoint).await {
        Ok(response) => {
            println!("✓ Response received");

            if let Some(mime_parts) = response.mime_parts {
                match mime_parts.signature {
                    Some(sig) => println!("✓ Signature present: {} bytes", sig.len()),
                    None => println!("  No signature (expected for certificates)"),
                }

                if mime_parts.data.contains("-----BEGIN CERTIFICATE-----") {
                    println!("  Certificate data present");
                }
            }
        }
        Err(e) => println!("✗ Error: {}", e),
    }

    println!("\n{:=<60}", "");
    println!("\nNote: Full PKCS#7 parsing would extract certificates,");
    println!("signer information, and allow signature verification.");

    Ok(())
}

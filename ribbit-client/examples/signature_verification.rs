//! Enhanced signature verification example
//!
//! This example demonstrates the enhanced signature verification features
//! including certificate chain extraction and validation.

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create client with V1 protocol (which includes signatures)
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Enhanced Signature Verification Example ===\n");

    // Test with a product version endpoint
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("Requesting: {}", endpoint.as_path());

    match client.request(&endpoint).await {
        Ok(response) => {
            println!("✓ Response received successfully");

            if let Some(mime_parts) = &response.mime_parts {
                // Basic signature info (backward compatible)
                if let Some(sig_info) = &mime_parts.signature_info {
                    println!("\n📝 Basic Signature Info:");
                    println!("  Format: {}", sig_info.format);
                    println!("  Size: {} bytes", sig_info.size);
                    println!("  Algorithm: {}", sig_info.algorithm);
                    println!("  Signers: {}", sig_info.signer_count);
                    println!("  Certificates: {}", sig_info.certificate_count);
                }

                // Enhanced signature verification
                if let Some(sig_verify) = &mime_parts.signature_verification {
                    println!("\n🔐 Enhanced Signature Verification:");
                    println!("  Digest Algorithm: {}", sig_verify.digest_algorithm);
                    println!("  Signature Algorithm: {}", sig_verify.signature_algorithm);
                    println!(
                        "  Verification Status: {}",
                        if sig_verify.is_verified {
                            "✓ Verified"
                        } else {
                            "✗ Not Verified"
                        }
                    );

                    if !sig_verify.verification_errors.is_empty() {
                        println!("  ⚠️  Errors:");
                        for error in &sig_verify.verification_errors {
                            println!("    - {error}");
                        }
                    }

                    if !sig_verify.certificates.is_empty() {
                        println!(
                            "\n📜 Certificate Chain ({} certificates):",
                            sig_verify.certificates.len()
                        );
                        for (i, cert) in sig_verify.certificates.iter().enumerate() {
                            println!("\n  Certificate #{}:", i + 1);
                            println!("    Subject: {}", cert.subject);
                            println!("    Issuer: {}", cert.issuer);
                            println!("    Serial: {}", cert.serial_number);
                            println!("    Valid From: {}", cert.not_before);
                            println!("    Valid Until: {}", cert.not_after);
                            println!(
                                "    Currently Valid: {}",
                                if cert.is_valid { "✓ Yes" } else { "✗ No" }
                            );
                        }
                    }

                    // Timestamp information
                    if let Some(ts_info) = &sig_verify.timestamp_info {
                        println!("\n⏰ Timestamp Information:");
                        if let Some(signing_time) = &ts_info.signing_time {
                            println!("  Signing Time: {signing_time}");
                        }
                        if let Some(tsa) = &ts_info.timestamp_authority {
                            println!("  Timestamp Authority: {tsa}");
                        }
                        println!(
                            "  Timestamp Verified: {}",
                            if ts_info.is_verified {
                                "✓ Yes"
                            } else {
                                "✗ No"
                            }
                        );
                    }
                } else {
                    println!("\n⚠️  Enhanced signature verification not available");
                }

                // Checksum info
                if let Some(checksum) = &mime_parts.checksum {
                    println!("\n✓ Checksum: {checksum} (verified)");
                }
            } else {
                println!("\n⚠️  No MIME parts found (V2 protocol?)");
            }
        }
        Err(e) => {
            eprintln!("✗ Request failed: {e}");
        }
    }

    // Try with a certificate endpoint
    println!("\n\n=== Certificate Endpoint Test ===\n");
    let cert_endpoint = Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string());

    match client.request(&cert_endpoint).await {
        Ok(response) => {
            println!("✓ Certificate response received");

            if let Some(data) = &response.data {
                if data.contains("-----BEGIN CERTIFICATE-----") {
                    println!("✓ Contains PEM certificate data");
                }
            }

            if let Some(mime_parts) = &response.mime_parts {
                if mime_parts.signature_verification.is_some() {
                    println!("✓ Has signature verification data");
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Certificate request failed: {e}");
        }
    }

    Ok(())
}

//! Example demonstrating public key extraction from signer certificates
//!
//! This example shows how the enhanced CMS parser extracts public keys
//! from the signer's certificate in PKCS#7 signatures.

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create client with V1 protocol (which includes signatures)
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Public Key Extraction Example ===\n");

    // Test with a product version endpoint
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("Requesting: {}", endpoint.as_path());

    match client.request(&endpoint).await {
        Ok(response) => {
            println!("✓ Response received successfully");

            if let Some(mime_parts) = &response.mime_parts {
                // Check enhanced signature verification
                if let Some(sig_verify) = &mime_parts.signature_verification {
                    println!("\n🔐 Signature Information:");
                    println!("  Format: {}", sig_verify.format);
                    println!("  Digest Algorithm: {}", sig_verify.digest_algorithm);
                    println!("  Signature Algorithm: {}", sig_verify.signature_algorithm);
                    println!("  Signer Count: {}", sig_verify.signer_count);
                    println!("  Certificate Count: {}", sig_verify.certificates.len());

                    // Look for public key extraction
                    if sig_verify.signer_count > 0 {
                        println!("\n🔑 Public Key Extraction Status:");

                        // Try to parse with CMS parser directly
                        if let Some(sig_content) = &mime_parts.signature {
                            match ribbit_client::cms_parser::parse_cms_signature(sig_content) {
                                Ok(cms_info) => {
                                    println!("  ✓ Successfully parsed CMS signature");
                                    println!("  Found {} signers", cms_info.signers.len());

                                    for (i, signer) in cms_info.signers.iter().enumerate() {
                                        println!("\n  Signer #{}:", i + 1);
                                        println!("    Issuer: {}", signer.identifier.issuer);
                                        println!("    Serial: {}", signer.identifier.serial_number);
                                        println!(
                                            "    Digest Algorithm: {}",
                                            signer.digest_algorithm
                                        );
                                        println!(
                                            "    Signature Algorithm: {}",
                                            signer.signature_algorithm
                                        );

                                        if let Some(ref pk) = signer.public_key {
                                            println!("\n    📌 Public Key Extracted:");
                                            println!("      Algorithm: {}", pk.algorithm);
                                            println!("      Key Size: {} bits", pk.key_size);
                                            println!(
                                                "      Key Bytes: {} bytes",
                                                pk.key_bytes.len()
                                            );

                                            // Show first few bytes of the key
                                            let preview_len = pk.key_bytes.len().min(16);
                                            let key_preview: Vec<String> = pk.key_bytes
                                                [..preview_len]
                                                .iter()
                                                .map(|b| format!("{b:02x}"))
                                                .collect();
                                            println!(
                                                "      Key Preview: {}...",
                                                key_preview.join(" ")
                                            );
                                        } else {
                                            println!(
                                                "    ⚠️  No public key found (certificate not matched)"
                                            );
                                        }

                                        if let Some(ref cert) = signer.certificate {
                                            println!("\n    📜 Matched Certificate:");
                                            println!("      Subject: {}", cert.subject);
                                            println!("      Issuer: {}", cert.issuer);
                                        } else {
                                            println!("    ⚠️  No certificate matched to signer");
                                        }
                                    }

                                    println!("\n  📋 Available Certificates:");
                                    for (i, cert) in cms_info.certificates.iter().enumerate() {
                                        println!("    Certificate #{}:", i + 1);
                                        println!("      Subject: {}", cert.subject);
                                        println!("      Serial: {}", cert.serial_number);
                                        if let Some(ref pk) = cert.public_key {
                                            println!(
                                                "      Public Key: {} {} bits",
                                                pk.algorithm, pk.key_size
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("  ✗ CMS parsing failed: {e}");
                                }
                            }
                        }
                    } else {
                        println!("\n⚠️  No signers found in signature");
                    }
                } else {
                    println!("\n⚠️  No enhanced signature verification available");
                }
            } else {
                println!("\n⚠️  No MIME parts found (V2 protocol?)");
            }
        }
        Err(e) => {
            eprintln!("✗ Request failed: {e}");
        }
    }

    Ok(())
}

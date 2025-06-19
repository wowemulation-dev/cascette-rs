//! Complete signature verification example
//!
//! This demonstrates the full PKI and signature verification workflow:
//! 1. Fetch signed data from Ribbit
//! 2. Parse CMS/PKCS#7 signature
//! 3. Extract SKI and fetch certificate
//! 4. Verify signature using the certificate's public key
//! 5. Validate certificate status via OCSP

use ribbit_client::{
    Endpoint, ProtocolVersion, Region, RibbitClient,
    certificate_fetcher::fetch_signer_certificate,
    cms_parser::{parse_cms_signature, verify_with_public_key},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Ribbit Full Signature Verification Demo ===\n");

    // Test multiple endpoints
    let endpoints = vec![
        Endpoint::ProductVersions("wow".to_string()),
        Endpoint::ProductCdns("wow".to_string()),
        Endpoint::Summary,
    ];

    for endpoint in endpoints {
        println!("\n📋 Testing endpoint: {}", endpoint.as_path());
        println!("{}", "━".repeat(50));

        let response = match client.request(&endpoint).await {
            Ok(resp) => resp,
            Err(e) => {
                println!("❌ Failed to fetch: {}", e);
                continue;
            }
        };

        if let Some(mime_parts) = &response.mime_parts {
            if let Some(sig_bytes) = &mime_parts.signature {
                // Parse the signature
                let cms_info = match parse_cms_signature(sig_bytes) {
                    Ok(info) => info,
                    Err(e) => {
                        println!("❌ Failed to parse signature: {}", e);
                        continue;
                    }
                };

                println!("✓ Signature parsed successfully");
                println!("  Format: PKCS#7/CMS");
                println!("  Detached: {}", cms_info.signed_data.is_detached);
                println!("  Signers: {}", cms_info.signers.len());

                // Process each signer
                for (i, signer) in cms_info.signers.iter().enumerate() {
                    println!("\n  Signer #{}:", i);
                    println!(
                        "    Algorithm: {} with {}",
                        signer.signature_algorithm, signer.digest_algorithm
                    );

                    // Check if using SKI
                    if signer
                        .identifier
                        .issuer
                        .starts_with("SubjectKeyIdentifier:")
                    {
                        let ski = &signer.identifier.serial_number;
                        println!("    SKI: {}", ski);

                        // Fetch certificate
                        match fetch_signer_certificate(&client, ski).await {
                            Ok((cert, public_key)) => {
                                println!("    ✓ Certificate fetched");
                                println!("      Subject: {}", cert.subject);
                                println!("      Issuer: {}", cert.issuer);
                                println!(
                                    "      Public Key: {} {} bits",
                                    public_key.algorithm, public_key.key_size
                                );

                                // Verify signature
                                let verification_result = if signer.has_signed_attributes {
                                    // Verify using signed attributes
                                    if let Some(attrs_der) = &signer.signed_attributes_der {
                                        verify_with_public_key(
                                            &public_key,
                                            attrs_der,
                                            &signer.signature,
                                            &signer.digest_algorithm,
                                        )
                                    } else {
                                        Err(ribbit_client::error::Error::Asn1Error(
                                            "Missing signed attributes data".to_string(),
                                        ))
                                    }
                                } else {
                                    // Direct signature verification
                                    if let Some(data) = &response.data {
                                        verify_with_public_key(
                                            &public_key,
                                            data.as_bytes(),
                                            &signer.signature,
                                            &signer.digest_algorithm,
                                        )
                                    } else {
                                        Err(ribbit_client::error::Error::Asn1Error(
                                            "Missing data to verify".to_string(),
                                        ))
                                    }
                                };

                                match verification_result {
                                    Ok(true) => {
                                        println!("    ✅ SIGNATURE VERIFIED SUCCESSFULLY!");
                                    }
                                    Ok(false) => {
                                        println!("    ❌ Signature verification failed");
                                    }
                                    Err(e) => {
                                        println!("    ⚠️  Verification error: {}", e);
                                    }
                                }

                                // Check certificate status
                                let ocsp_endpoint = Endpoint::Ocsp(ski.to_string());
                                match client.request(&ocsp_endpoint).await {
                                    Ok(_) => {
                                        println!("    ✓ Certificate status checked (OCSP)");
                                    }
                                    Err(e) => {
                                        println!("    ⚠️  OCSP check failed: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("    ❌ Failed to fetch certificate: {}", e);
                            }
                        }
                    } else {
                        println!("    ⚠️  Not using SKI (would need embedded cert)");
                    }
                }
            } else {
                println!("  No signature present");
            }
        } else {
            println!("  Response is not MIME formatted (V2?)");
        }
    }

    println!("\n\n📊 Summary:");
    println!("{}", "━".repeat(50));
    println!("✓ Ribbit uses PKCS#7/CMS signatures with detached content");
    println!("✓ Signatures use SubjectKeyIdentifier (SKI) instead of embedded certs");
    println!("✓ Certificates can be fetched using the SKI");
    println!("✓ Signature verification uses signed attributes when present");
    println!("✓ OCSP can be used to check certificate revocation status");

    Ok(())
}

//! Test full signature verification workflow
//!
//! This example demonstrates the complete signature verification process:
//! 1. Fetch signed data from Ribbit
//! 2. Extract SKI from signature
//! 3. Fetch certificate using SKI
//! 4. Verify the signature with the public key

use ribbit_client::{
    Endpoint, ProtocolVersion, Region, RibbitClient, certificate_fetcher::fetch_signer_certificate,
    cms_parser::parse_cms_signature, cms_parser::verify_with_public_key,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Testing Full Signature Verification ===\n");

    // Fetch a signed response
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("1. Fetching signed data from: {}", endpoint.as_path());

    let response = client.request(&endpoint).await?;

    if let Some(mime_parts) = &response.mime_parts {
        if let Some(sig_bytes) = &mime_parts.signature {
            println!("   ✓ Got signature: {} bytes", sig_bytes.len());

            // Parse signature
            println!("\n2. Parsing signature...");
            let cms_info = parse_cms_signature(sig_bytes)?;
            println!("   ✓ Parsed successfully");
            println!("   Signers: {}", cms_info.signers.len());
            println!(
                "   Certificates in signature: {}",
                cms_info.certificates.len()
            );

            if let Some(first_signer) = cms_info.signers.first() {
                println!("\n3. Signer Information:");
                println!("   Identifier: {}", first_signer.identifier.issuer);
                println!("   Algorithm: {}", first_signer.signature_algorithm);
                println!("   Digest: {}", first_signer.digest_algorithm);

                // Check if it's using SKI
                if first_signer
                    .identifier
                    .issuer
                    .starts_with("SubjectKeyIdentifier:")
                {
                    let ski = &first_signer.identifier.serial_number;
                    println!("   SKI: {}", ski);

                    // Fetch certificate
                    println!("\n4. Fetching certificate using SKI...");
                    match fetch_signer_certificate(&client, ski).await {
                        Ok((cert, public_key)) => {
                            println!("   ✓ Certificate retrieved");
                            println!("   Subject: {}", cert.subject);
                            println!(
                                "   Public Key: {} {} bits",
                                public_key.algorithm, public_key.key_size
                            );

                            // Now verify the signature
                            println!("\n5. Verifying signature...");

                            // Check if we have signed attributes
                            if first_signer.has_signed_attributes {
                                println!("   Using signed attributes for verification");

                                if let Some(signed_attrs_der) = &first_signer.signed_attributes_der
                                {
                                    match verify_with_public_key(
                                        &public_key,
                                        signed_attrs_der,
                                        &first_signer.signature,
                                        &first_signer.digest_algorithm,
                                    ) {
                                        Ok(true) => {
                                            println!("   ✅ SIGNATURE VERIFICATION SUCCESSFUL!");
                                            println!("   The signature is valid and authentic.");
                                        }
                                        Ok(false) => {
                                            println!("   ❌ SIGNATURE VERIFICATION FAILED!");
                                            println!("   The signature does not match the data.");
                                        }
                                        Err(e) => {
                                            println!("   ⚠️  Verification error: {}", e);
                                        }
                                    }
                                } else {
                                    println!("   ⚠️  No signed attributes data");
                                }
                            } else {
                                // Direct signature over content
                                println!("   Using direct signature verification");

                                if let Some(data) = &response.data {
                                    let signed_bytes = data.as_bytes();

                                    match verify_with_public_key(
                                        &public_key,
                                        signed_bytes,
                                        &first_signer.signature,
                                        &first_signer.digest_algorithm,
                                    ) {
                                        Ok(true) => {
                                            println!("   ✅ SIGNATURE VERIFICATION SUCCESSFUL!");
                                            println!("   The signature is valid and authentic.");
                                        }
                                        Ok(false) => {
                                            println!("   ❌ SIGNATURE VERIFICATION FAILED!");
                                            println!("   The signature does not match the data.");
                                        }
                                        Err(e) => {
                                            println!("   ⚠️  Verification error: {}", e);
                                        }
                                    }
                                } else {
                                    println!("   ⚠️  No data to verify");
                                }
                            }
                        }
                        Err(e) => {
                            println!("   ✗ Failed to fetch certificate: {}", e);
                        }
                    }
                } else {
                    println!("   ⚠️  Not using SKI, would need embedded certificate");
                }
            }
        } else {
            println!("   ✗ No signature found in response");
        }
    } else {
        println!("   ✗ Response is not MIME formatted");
    }

    println!("\n📝 Summary:");
    println!("This demonstrates the complete signature verification workflow.");
    println!("The Ribbit protocol uses PKCS#7/CMS signatures with RSA and SHA-256.");

    Ok(())
}

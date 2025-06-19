//! Complete signature verification with certificate fetching
//!
//! This example demonstrates:
//! 1. Parsing a signature to extract the SKI
//! 2. Fetching the certificate using the SKI
//! 3. Extracting the public key
//! 4. Verifying the signature (placeholder for now)

use ribbit_client::{
    Endpoint, ProtocolVersion, Region, RibbitClient, certificate_fetcher::fetch_signer_certificate,
    cms_parser::parse_cms_signature,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Complete Signature Verification Example ===\n");

    // Step 1: Get a signed response
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("1. Fetching signed response from: {}", endpoint.as_path());

    let response = client.request(&endpoint).await?;

    if let Some(mime_parts) = &response.mime_parts {
        if let Some(sig_bytes) = &mime_parts.signature {
            println!("✓ Got signature: {} bytes", sig_bytes.len());

            // Step 2: Parse signature to get SKI
            println!("\n2. Parsing signature to extract signer info...");
            let cms_info = parse_cms_signature(sig_bytes)?;

            if let Some(first_signer) = cms_info.signers.first() {
                println!("✓ Found signer:");
                println!("  Identifier: {}", first_signer.identifier.issuer);
                println!("  Digest Algorithm: {}", first_signer.digest_algorithm);
                println!(
                    "  Signature Algorithm: {}",
                    first_signer.signature_algorithm
                );

                // Extract SKI
                if first_signer
                    .identifier
                    .issuer
                    .starts_with("SubjectKeyIdentifier:")
                {
                    let ski = &first_signer.identifier.serial_number;
                    println!("  SKI: {}", ski);

                    // Step 3: Fetch certificate using SKI
                    println!("\n3. Fetching certificate for SKI: {}", ski);
                    match fetch_signer_certificate(&client, ski).await {
                        Ok((cert, public_key)) => {
                            println!("✓ Certificate fetched successfully!");
                            println!("  Subject: {}", cert.subject);
                            println!("  Issuer: {}", cert.issuer);
                            println!("  Serial: {}", cert.serial_number);

                            println!("\n📌 Public Key Extracted:");
                            println!("  Algorithm: {}", public_key.algorithm);
                            println!("  Key Size: {} bits", public_key.key_size);
                            println!("  Key Bytes: {} bytes", public_key.key_bytes.len());

                            // Step 4: Verify signature (placeholder)
                            println!("\n4. Signature Verification:");
                            println!("  ⚠️  Full verification not yet implemented");
                            println!("  We now have all components needed:");
                            println!("  - The signed data");
                            println!("  - The signature bytes");
                            println!("  - The public key");
                            println!(
                                "  - The digest algorithm ({})",
                                first_signer.digest_algorithm
                            );

                            // Show what would be needed for full verification
                            println!("\n📋 Next Steps for Full Verification:");
                            println!(
                                "  1. Compute {} hash of the signed data",
                                first_signer.digest_algorithm
                            );
                            println!("  2. Decrypt signature with RSA public key");
                            println!("  3. Extract DigestInfo from decrypted signature");
                            println!("  4. Compare computed hash with signature hash");
                        }
                        Err(e) => {
                            println!("✗ Failed to fetch certificate: {}", e);
                        }
                    }
                } else {
                    println!("✗ Signer doesn't use SubjectKeyIdentifier");
                }
            } else {
                println!("✗ No signers found in signature");
            }
        } else {
            println!("✗ No signature in response");
        }
    } else {
        println!("✗ No MIME parts (using V2 protocol?)");
    }

    // Demonstrate certificate caching strategy
    println!("\n=== Certificate Caching Strategy ===");
    println!("For production use, consider:");
    println!("1. Cache certificates by SKI to avoid repeated fetches");
    println!("2. Set reasonable TTL (e.g., 24 hours)");
    println!("3. Handle certificate rotation gracefully");
    println!("4. Store Blizzard root certificates for chain validation");

    Ok(())
}

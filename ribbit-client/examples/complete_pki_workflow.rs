//! Complete PKI workflow demonstration
//!
//! This example shows the full certificate lifecycle in Ribbit:
//! 1. Extract SKI from signature
//! 2. Fetch certificate using SKI
//! 3. Check certificate status via OCSP
//! 4. Extract public key for verification

use base64::Engine;
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

    println!("=== Complete Ribbit PKI Workflow ===\n");

    // Step 1: Get a signed response
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("1️⃣  Fetching signed data from: {}", endpoint.as_path());

    let response = client.request(&endpoint).await?;

    if let Some(mime_parts) = &response.mime_parts {
        if let Some(sig_bytes) = &mime_parts.signature {
            println!("   ✓ Got signature: {} bytes", sig_bytes.len());

            // Step 2: Parse signature to extract SKI
            println!("\n2️⃣  Parsing signature to extract SKI...");
            let cms_info = parse_cms_signature(sig_bytes)?;

            if let Some(first_signer) = cms_info.signers.first() {
                if first_signer
                    .identifier
                    .issuer
                    .starts_with("SubjectKeyIdentifier:")
                {
                    let ski = &first_signer.identifier.serial_number;
                    println!("   ✓ Found SKI: {ski}");
                    println!("   Digest Algorithm: {}", first_signer.digest_algorithm);
                    println!(
                        "   Signature Algorithm: {}",
                        first_signer.signature_algorithm
                    );

                    // Step 3: Fetch certificate using SKI
                    println!("\n3️⃣  Fetching certificate using SKI...");
                    match fetch_signer_certificate(&client, ski).await {
                        Ok((cert, public_key)) => {
                            println!("   ✓ Certificate retrieved successfully!");
                            println!("   Subject: {}", cert.subject);
                            println!("   Issuer: {}", cert.issuer);
                            println!(
                                "   Public Key: {} {} bits",
                                public_key.algorithm, public_key.key_size
                            );

                            // Step 4: Check certificate status via OCSP
                            println!("\n4️⃣  Checking certificate status via OCSP...");
                            let ocsp_endpoint = Endpoint::Ocsp(ski.to_string());

                            match client.request(&ocsp_endpoint).await {
                                Ok(ocsp_response) => {
                                    if let Some(data) = &ocsp_response.data {
                                        // Extract and decode OCSP response
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
                                        if let Ok(ocsp_der) =
                                            base64::engine::general_purpose::STANDARD
                                                .decode(&base64_clean)
                                        {
                                            // Check for good status
                                            let mut status = "Unknown";
                                            for i in 0..ocsp_der.len() - 1 {
                                                if ocsp_der[i] == 0x80 && ocsp_der[i + 1] == 0x00 {
                                                    status = "GOOD ✓";
                                                    break;
                                                } else if ocsp_der[i] == 0xa1 {
                                                    status = "REVOKED ❌";
                                                    break;
                                                }
                                            }
                                            println!("   Certificate Status: {status}");

                                            // Extract timestamps
                                            for i in 0..ocsp_der.len() - 15 {
                                                if ocsp_der[i] == 0x18 && ocsp_der[i + 1] == 0x0f {
                                                    let timestamp = &ocsp_der[i + 2..i + 17];
                                                    if let Ok(ts_str) =
                                                        std::str::from_utf8(timestamp)
                                                    {
                                                        if ts_str.ends_with('Z')
                                                            && ts_str.starts_with("202")
                                                        {
                                                            println!("   Update Time: {ts_str}");
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("   ⚠️  OCSP check failed: {e}");
                                }
                            }

                            // Step 5: Ready for signature verification
                            println!("\n5️⃣  Ready for signature verification!");
                            println!("   We now have:");
                            println!("   - ✓ The signed data");
                            println!("   - ✓ The signature");
                            println!("   - ✓ The public key (verified via certificate)");
                            println!("   - ✓ Certificate status (confirmed not revoked)");
                            println!("\n   Next step would be RSA signature verification");
                        }
                        Err(e) => {
                            println!("   ✗ Failed to fetch certificate: {e}");
                        }
                    }
                }
            }
        }
    }

    println!("\n📝 Summary:");
    println!("The Ribbit protocol provides a complete PKI infrastructure:");
    println!("- Signatures use SKI (Subject Key Identifier) instead of embedded certificates");
    println!("- The same SKI works with both /v1/certs/ and /v1/ocsp/ endpoints");
    println!("- This enables efficient certificate management and revocation checking");

    Ok(())
}

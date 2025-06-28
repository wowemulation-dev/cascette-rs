//! Verify that the certificate from SKI endpoint matches the signature
//!
//! This tests if the certificate returned by the SKI endpoint has the same SKI.

use base64::Engine;
use der::Decode;
use ribbit_client::{Endpoint, Region, RibbitClient};
use x509_cert::Certificate;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("=== SKI Certificate Verification ===\n");

    // The SKI from the signature
    let ski_from_signature = "782a8a710b950421127250a3e91b751ca356e202";
    println!("SKI from signature: {ski_from_signature}\n");

    // Fetch the certificate using the SKI
    let endpoint = Endpoint::Cert(ski_from_signature.to_string());

    match client.request_raw(&endpoint).await {
        Ok(raw_response) => {
            let response_str = String::from_utf8_lossy(&raw_response);

            if response_str.contains("-----BEGIN CERTIFICATE-----") {
                println!("✓ Got certificate from SKI endpoint");

                // Extract the certificate
                if let Some(cert_start) = response_str.find("-----BEGIN CERTIFICATE-----") {
                    if let Some(cert_end) = response_str.find("-----END CERTIFICATE-----") {
                        let cert_pem = &response_str[cert_start..cert_end + 25];

                        // Extract base64 content
                        let lines: Vec<&str> = cert_pem
                            .lines()
                            .filter(|line| !line.contains("-----"))
                            .collect();
                        let base64_content = lines.join("");

                        // Decode from base64
                        if let Ok(cert_der) =
                            base64::engine::general_purpose::STANDARD.decode(&base64_content)
                        {
                            // Parse certificate
                            match Certificate::from_der(&cert_der) {
                                Ok(cert) => {
                                    println!("\nCertificate Details:");
                                    println!("  Subject: {}", cert.tbs_certificate.subject);
                                    println!("  Issuer: {}", cert.tbs_certificate.issuer);

                                    // Extract SKI from certificate
                                    let mut found_ski = None;
                                    if let Some(extensions) = &cert.tbs_certificate.extensions {
                                        for ext in extensions.iter() {
                                            // Subject Key Identifier OID is 2.5.29.14
                                            if ext.extn_id.to_string() == "2.5.29.14" {
                                                let ski_bytes = ext.extn_value.as_bytes();
                                                // SKI is OCTET STRING, skip tag and length
                                                if ski_bytes.len() > 2 && ski_bytes[0] == 0x04 {
                                                    let ski_hex = hex::encode(&ski_bytes[2..]);
                                                    found_ski = Some(ski_hex);
                                                }
                                            }
                                        }
                                    }

                                    if let Some(cert_ski) = found_ski {
                                        println!("  SKI in certificate: {cert_ski}");

                                        if cert_ski == ski_from_signature {
                                            println!("\n✅ SUCCESS! The SKI matches!");
                                            println!(
                                                "This confirms that the SKI can be used to fetch the signer's certificate!"
                                            );

                                            // Extract public key info
                                            let spki =
                                                &cert.tbs_certificate.subject_public_key_info;
                                            println!("\n📌 Public Key Information:");
                                            println!("  Algorithm: {}", spki.algorithm.oid);
                                            println!(
                                                "  Key size: {} bytes",
                                                spki.subject_public_key.raw_bytes().len()
                                            );

                                            // This is the public key we can use for signature verification!
                                            println!(
                                                "\n🔑 We can now extract the public key for signature verification!"
                                            );
                                        } else {
                                            println!("\n❌ SKI mismatch!");
                                            println!("  Expected: {ski_from_signature}");
                                            println!("  Found: {cert_ski}");
                                        }
                                    } else {
                                        println!("\n⚠️  No SKI found in certificate extensions");
                                    }
                                }
                                Err(e) => {
                                    println!("✗ Failed to parse certificate: {e}");
                                }
                            }
                        }
                    }
                }
            } else {
                println!("✗ Response is not a certificate");
            }
        }
        Err(e) => {
            println!("✗ Request failed: {e}");
        }
    }

    // Also check why there was a checksum mismatch
    println!("\n=== Investigating Checksum Mismatch ===");

    match client.request(&endpoint).await {
        Ok(_) => {
            println!("✓ Normal request succeeded (checksum validated)");
        }
        Err(e) => {
            println!("✗ Normal request failed: {e}");
            println!(
                "This might be due to the certificate response format differing from expected"
            );
        }
    }

    Ok(())
}

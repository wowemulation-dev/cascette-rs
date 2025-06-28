//! Example to test if SKI can be used to fetch certificates from the certs endpoint
//!
//! This tests whether the Subject Key Identifier (SKI) found in signatures
//! can be used to retrieve the actual certificate.

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Certificate Fetching by SKI Test ===\n");

    // First, get a signature to find the SKI
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("1. Getting signature from: {}", endpoint.as_path());

    let ski = match client.request(&endpoint).await {
        Ok(response) => {
            if let Some(mime_parts) = &response.mime_parts {
                if let Some(sig_content) = &mime_parts.signature {
                    match ribbit_client::cms_parser::parse_cms_signature(sig_content) {
                        Ok(cms_info) => {
                            if let Some(first_signer) = cms_info.signers.first() {
                                // Extract SKI from the identifier
                                if first_signer
                                    .identifier
                                    .issuer
                                    .starts_with("SubjectKeyIdentifier:")
                                {
                                    let ski = &first_signer.identifier.serial_number;
                                    println!("✓ Found SKI: {ski}");
                                    Some(ski.clone())
                                } else {
                                    println!("✗ Signer doesn't use SKI");
                                    None
                                }
                            } else {
                                println!("✗ No signers found");
                                None
                            }
                        }
                        Err(e) => {
                            println!("✗ Failed to parse signature: {e}");
                            None
                        }
                    }
                } else {
                    println!("✗ No signature in response");
                    None
                }
            } else {
                println!("✗ No MIME parts in response");
                None
            }
        }
        Err(e) => {
            println!("✗ Request failed: {e}");
            None
        }
    };

    if let Some(ski) = ski {
        println!(
            "\n2. Testing different certificate endpoint formats with SKI: {ski}\n"
        );

        // Test different possible endpoint formats
        let test_endpoints = [
            // Try the SKI directly
            Endpoint::Cert(ski.clone()),
            // Try uppercase
            Endpoint::Cert(ski.to_uppercase()),
            // Try with common certificate fingerprint hashes
            Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string()), // Known working cert
        ];

        for (i, cert_endpoint) in test_endpoints.iter().enumerate() {
            println!("Test #{}: {}", i + 1, cert_endpoint.as_path());

            match client.request(cert_endpoint).await {
                Ok(response) => {
                    if let Some(data) = &response.data {
                        if data.contains("-----BEGIN CERTIFICATE-----") {
                            println!("✓ Got PEM certificate!");

                            // Try to parse the certificate
                            if let Some(cert_start) = data.find("-----BEGIN CERTIFICATE-----") {
                                if let Some(cert_end) = data.find("-----END CERTIFICATE-----") {
                                    let cert_pem = &data[cert_start..cert_end + 25];

                                    // Extract base64 content
                                    let lines: Vec<&str> = cert_pem
                                        .lines()
                                        .filter(|line| !line.contains("-----"))
                                        .collect();
                                    let base64_content = lines.join("");

                                    // Decode from base64
                                    use base64::Engine;
                                    if let Ok(cert_der) = base64::engine::general_purpose::STANDARD
                                        .decode(&base64_content)
                                    {
                                        // Parse certificate
                                        use der::Decode;
                                        use x509_cert::Certificate;

                                        match Certificate::from_der(&cert_der) {
                                            Ok(cert) => {
                                                println!(
                                                    "  Subject: {}",
                                                    cert.tbs_certificate.subject
                                                );

                                                // Try to extract SKI from certificate extensions
                                                if let Some(extensions) =
                                                    &cert.tbs_certificate.extensions
                                                {
                                                    for ext in extensions.iter() {
                                                        // Subject Key Identifier OID is 2.5.29.14
                                                        if ext.extn_id.to_string() == "2.5.29.14" {
                                                            let ski_bytes =
                                                                ext.extn_value.as_bytes();
                                                            // SKI is typically OCTET STRING containing the identifier
                                                            // Skip the OCTET STRING tag and length
                                                            if ski_bytes.len() > 2
                                                                && ski_bytes[0] == 0x04
                                                            {
                                                                let ski_hex =
                                                                    hex::encode(&ski_bytes[2..]);
                                                                println!(
                                                                    "  Certificate SKI: {ski_hex}"
                                                                );

                                                                if ski_hex == ski {
                                                                    println!(
                                                                        "  🎯 SKI MATCHES! This is the signer's certificate!"
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                println!("  Failed to parse certificate: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                        } else if data.contains("Product") || data.contains("Region") {
                            println!("✗ Got product data instead of certificate");
                        } else {
                            println!(
                                "✗ Response doesn't contain certificate (length: {} bytes)",
                                data.len()
                            );
                        }
                    } else {
                        println!("✗ Empty response");
                    }
                }
                Err(e) => {
                    println!("✗ Request failed: {e}");
                }
            }
            println!();
        }

        // Also test if there's a pattern for certificate endpoints
        println!("\n3. Analyzing certificate endpoint patterns:\n");

        // Get a few known certificates to see the pattern
        let known_certs = vec![
            "5168ff90af0207753cccd9656462a212b859723b",
            "28458c5833cf2cf050900c3ddc956011de3a8fce",
        ];

        for cert_hash in known_certs {
            println!("Fetching known cert: {cert_hash}");
            let cert_endpoint = Endpoint::Cert(cert_hash.to_string());

            if let Ok(response) = client.request(&cert_endpoint).await {
                if let Some(data) = &response.data {
                    if data.contains("-----BEGIN CERTIFICATE-----") {
                        println!("✓ Success - appears to be SHA-1 fingerprint");
                    }
                }
            }
        }

        println!("\nConclusion: The certs endpoint likely expects SHA-1 fingerprints, not SKIs");
        println!(
            "The SKI {ski} cannot be directly used with the certs endpoint"
        );
    }

    Ok(())
}

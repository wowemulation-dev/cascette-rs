//! Debug what data is actually signed in Ribbit responses
//!
//! This helps us understand the exact format of signed data.

use ribbit_client::{
    Endpoint, ProtocolVersion, Region, RibbitClient, cms_parser::parse_cms_signature,
};
use sha2::{Digest, Sha256};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Debugging Ribbit Signature Data ===\n");

    // Fetch raw response
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    println!("Fetching raw response from: {}", endpoint.as_path());

    let raw_bytes = client.request_raw(&endpoint).await?;
    println!("Total response size: {} bytes\n", raw_bytes.len());

    // Find the MIME boundary
    let response_str = String::from_utf8_lossy(&raw_bytes);
    if let Some(boundary_start) = response_str.find("boundary=\"") {
        let boundary_end = response_str[boundary_start + 10..].find('"').unwrap_or(0);
        let boundary = &response_str[boundary_start + 10..boundary_start + 10 + boundary_end];
        println!("MIME boundary: {boundary}");

        // Find the data part
        let data_marker = format!("--{boundary}\r\nContent-Type: text/plain");
        if let Some(data_start) = response_str.find(&data_marker) {
            // Find where data content starts (after headers)
            if let Some(content_start) = response_str[data_start..].find("\r\n\r\n") {
                let content_start_pos = data_start + content_start + 4;

                // Find where data content ends
                let next_boundary = format!("\r\n--{boundary}");
                if let Some(content_end) = response_str[content_start_pos..].find(&next_boundary) {
                    let data_content =
                        &response_str[content_start_pos..content_start_pos + content_end];

                    println!("\nData content ({} bytes):", data_content.len());
                    println!(
                        "First 200 chars: {}",
                        &data_content[..200.min(data_content.len())]
                    );

                    // Compute SHA-256 of just the data content
                    let data_hash = Sha256::digest(data_content.as_bytes());
                    println!("\nSHA-256 of data content: {data_hash:x}");

                    // Now let's check what the signature actually contains
                    let response = client.request(&endpoint).await?;
                    if let Some(mime_parts) = &response.mime_parts {
                        if let Some(sig_bytes) = &mime_parts.signature {
                            let cms_info = parse_cms_signature(sig_bytes)?;
                            if let Some(signer) = cms_info.signers.first() {
                                println!("\nSignature info:");
                                println!("  Algorithm: {}", signer.digest_algorithm);
                                println!("  Signature size: {} bytes", signer.signature.len());

                                // The signature might be over different data formats:
                                // 1. Just the PSV content
                                // 2. The entire MIME part (with headers)
                                // 3. The entire message (minus checksum)

                                // Test different possibilities
                                println!("\nTesting different signed data possibilities:");

                                // Test 1: Just the PSV content
                                let test1_hash = Sha256::digest(data_content.as_bytes());
                                println!("1. PSV content only: {test1_hash:x}");

                                // Test 2: The entire data MIME part
                                if let Some(part_end) =
                                    response_str[data_start..].find(&next_boundary)
                                {
                                    let mime_part =
                                        &response_str[data_start..data_start + part_end];
                                    let test2_hash = Sha256::digest(mime_part.as_bytes());
                                    println!("2. Entire MIME part: {test2_hash:x}");
                                }

                                // Test 3: Everything before the checksum
                                if let Some(checksum_pos) = response_str.rfind("\nChecksum:") {
                                    let before_checksum = &raw_bytes[..checksum_pos];
                                    let test3_hash = Sha256::digest(before_checksum);
                                    println!("3. Everything before checksum: {test3_hash:x}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

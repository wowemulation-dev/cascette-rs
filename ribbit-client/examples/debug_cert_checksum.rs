use ribbit_client::{Endpoint, Region, RibbitClient};
use sha2::{Sha256, Digest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut client = RibbitClient::new(Region::US);
    client.set_protocol_version(ribbit_client::ProtocolVersion::V1);

    println!("=== Debugging Certificate Checksum Issue ===\n");

    let ski = "782a8a710b950421127250a3e91b751ca356e202";
    println!("Requesting certificate for SKI: {}", ski);

    // Make raw request to see actual data
    match client.request_raw(&Endpoint::Cert(ski.to_string())).await {
        Ok(raw_data) => {
            println!("\nReceived {} bytes", raw_data.len());
            
            // Try to find MIME boundaries and structure
            let data_str = String::from_utf8_lossy(&raw_data);
            
            // Look for the checksum in epilogue
            let mut message_without_checksum = &raw_data[..];
            if let Some(checksum_start) = data_str.find("Checksum: ") {
                let checksum_line = &data_str[checksum_start..];
                if let Some(end) = checksum_line.find('\n') {
                    let checksum = checksum_line[10..end].trim();
                    println!("Expected checksum from epilogue: {}", checksum);
                    
                    // Get the byte position of the checksum line start
                    let checksum_byte_pos = checksum_start;
                    message_without_checksum = &raw_data[..checksum_byte_pos];
                    
                    // Compute checksum of message without the checksum line
                    let mut hasher = Sha256::new();
                    hasher.update(message_without_checksum);
                    let computed = format!("{:x}", hasher.finalize());
                    println!("Computed checksum of message without checksum line: {}", computed);
                    
                    if computed == checksum {
                        println!("✓ Checksum matches!");
                    } else {
                        println!("✗ Checksum mismatch!");
                    }
                }
            }
            
            // Find the certificate content specifically
            let cert_start_marker = "-----BEGIN CERTIFICATE-----";
            let cert_end_marker = "-----END CERTIFICATE-----";
            
            if let Some(cert_start) = data_str.find(cert_start_marker) {
                if let Some(cert_end_pos) = data_str[cert_start..].find(cert_end_marker) {
                    let cert_end = cert_start + cert_end_pos + cert_end_marker.len();
                    let cert_content = &data_str[cert_start..cert_end];
                    
                    println!("\nFound certificate content ({} bytes)", cert_content.len());
                    
                    // Method 1: Checksum of certificate content including headers
                    let mut hasher = Sha256::new();
                    hasher.update(cert_content.as_bytes());
                    let checksum1 = format!("{:x}", hasher.finalize());
                    println!("Checksum of certificate with headers: {}", checksum1);
                    
                    // Method 2: Checksum of certificate content with trailing newline
                    let mut hasher = Sha256::new();
                    hasher.update(cert_content.as_bytes());
                    hasher.update(b"\n");
                    let checksum2 = format!("{:x}", hasher.finalize());
                    println!("Checksum of certificate with trailing newline: {}", checksum2);
                    
                    // Method 3: Checksum of entire MIME part body (from Content-Disposition to boundary)
                    if let Some(content_disp) = data_str.find("Content-Disposition: cert") {
                        if let Some(body_start) = data_str[content_disp..].find("\r\n\r\n") {
                            let body_start_pos = content_disp + body_start + 4;
                            if let Some(boundary_pos) = data_str[body_start_pos..].find("\r\n--") {
                                let body_end = body_start_pos + boundary_pos;
                                let body = &data_str[body_start_pos..body_end];
                                
                                let mut hasher = Sha256::new();
                                hasher.update(body.as_bytes());
                                let checksum3 = format!("{:x}", hasher.finalize());
                                println!("Checksum of MIME part body: {}", checksum3);
                            }
                        }
                    }
                    
                    // Print first few lines of certificate
                    println!("\nCertificate preview:");
                    for (i, line) in cert_content.lines().take(5).enumerate() {
                        println!("  {}: {}", i, line);
                    }
                } else {
                    println!("Could not find certificate end marker");
                }
            } else {
                println!("Could not find certificate start marker");
            }
            
            // Save raw response for analysis
            std::fs::write("debug_cert_response.txt", &raw_data)?;
            println!("\nRaw response saved to debug_cert_response.txt");
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }

    Ok(())
}
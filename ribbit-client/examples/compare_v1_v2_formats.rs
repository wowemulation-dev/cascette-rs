//! Compare V1 (MIME) and V2 (raw) response formats for the same endpoint

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Comparing V1 vs V2 Response Formats ===\n");

    let endpoint = Endpoint::ProductVersions("wow".to_string());

    // Test V1 (MIME with signature)
    println!("🔒 V1 Protocol (MIME + Signature):");
    let client_v1 = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    match client_v1.request(&endpoint).await {
        Ok(response) => {
            println!("   ✅ Success");
            if let Some(mime_parts) = &response.mime_parts {
                println!("   📦 MIME Structure:");
                if let Some(data) = &response.data {
                    println!("      📄 Data: {} bytes", data.len());
                    // Check if the MIME data content is also BPSV
                    let is_bpsv = data.contains('|') && data.contains("!STRING");
                    println!(
                        "      📊 Content Format: {}",
                        if is_bpsv { "BPSV" } else { "Other" }
                    );

                    if is_bpsv {
                        let lines: Vec<&str> = data.lines().take(3).collect();
                        println!("      📋 Sample lines:");
                        for (i, line) in lines.iter().enumerate() {
                            println!("         {}: {}", i + 1, line);
                        }
                    }
                }
                if let Some(sig) = &mime_parts.signature {
                    println!("      🔐 Signature: {} bytes", sig.len());
                }
                if let Some(checksum) = &mime_parts.checksum {
                    println!("      ✅ Checksum: {checksum}");
                }
            }
        }
        Err(e) => println!("   ❌ Error: {e}"),
    }

    println!();

    // Test V2 (Raw PSV)
    println!("📄 V2 Protocol (Raw BPSV):");
    let client_v2 = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    match client_v2.request(&endpoint).await {
        Ok(response) => {
            println!("   ✅ Success");
            if let Some(data) = &response.data {
                println!("   📄 Data: {} bytes", data.len());
                let is_bpsv = data.contains('|') && data.contains("!STRING");
                println!("   📊 Format: {}", if is_bpsv { "BPSV" } else { "Other" });

                if is_bpsv {
                    let lines: Vec<&str> = data.lines().take(3).collect();
                    println!("   📋 Sample lines:");
                    for (i, line) in lines.iter().enumerate() {
                        println!("      {}: {}", i + 1, line);
                    }
                }
            }
            // V2 has no signature or checksum
            println!("   🔐 Signature: None (V2 doesn't include signatures)");
            println!("   ✅ Checksum: None (V2 doesn't include checksums)");
        }
        Err(e) => println!("   ❌ Error: {e}"),
    }

    println!("\n📋 Key Findings:");
    println!("   • Both V1 and V2 contain the same BPSV data content");
    println!("   • V1 wraps BPSV in MIME with signature and checksum verification");
    println!("   • V2 provides raw BPSV data without cryptographic verification");
    println!("   • BPSV parsing is essential for both protocols");
    println!("   • All TACT/CDN endpoints use BPSV format");

    Ok(())
}

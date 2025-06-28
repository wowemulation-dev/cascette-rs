//! Debug BPSV format to understand the field length issue

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    println!("Fetching raw BPSV data...\n");

    let response = client
        .request(&Endpoint::ProductVersions("wow".to_string()))
        .await?;

    if let Some(data) = &response.data {
        // Show first few lines
        let lines: Vec<&str> = data.lines().take(5).collect();
        for (i, line) in lines.iter().enumerate() {
            println!("Line {i}: {line}");

            // For the first data line after header and seqn, show field lengths
            if i == 2 && !line.starts_with("#") {
                let fields: Vec<&str> = line.split('|').collect();
                println!("\nField lengths:");
                for (j, field) in fields.iter().enumerate() {
                    println!("  Field {}: {} chars ({})", j, field.len(), field);
                }
            }
        }
    }

    Ok(())
}

//! Example of basic TACT client usage

use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create a client for V1 protocol (original TCP-based)
    let client_v1 = HttpClient::new(Region::US, ProtocolVersion::V1)?;
    println!("V1 Base URL: {}", client_v1.base_url());

    // Fetch versions for World of Warcraft
    println!("\nFetching WoW versions...");
    let response = client_v1.get_versions("wow").await?;
    if response.status().is_success() {
        let text = response.text().await?;
        println!(
            "First 500 chars of versions response:\n{}",
            &text[..text.len().min(500)]
        );
    }

    // Create a client for V2 protocol (HTTPS-based)
    let client_v2 = HttpClient::new(Region::US, ProtocolVersion::V2)?;
    println!("\nV2 Base URL: {}", client_v2.base_url());

    // Fetch product summary
    println!("\nFetching product summary...");
    let response = client_v2.get_summary().await?;
    if response.status().is_success() {
        println!("Summary request successful!");
    }

    // Example of switching regions
    let mut client = HttpClient::new(Region::US, ProtocolVersion::V1)?;
    println!("\nOriginal region: {}", client.region());

    client.set_region(Region::EU);
    println!("New region: {}", client.region());
    println!("New base URL: {}", client.base_url());

    Ok(())
}

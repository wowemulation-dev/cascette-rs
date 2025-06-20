//! Test different products with TACT client

use tact_client::{HttpClient, ProtocolVersion, Region};

async fn test_product(client: &HttpClient, product: &str) {
    println!("\n{:=^60}", format!(" Testing {} ", product));

    // Test versions
    let response = client.get(&format!("/{}/versions", product)).await.unwrap();
    println!("/{}/versions - Status: {}", product, response.status());
    if response.status().is_success() {
        let text = response.text().await.unwrap();
        let lines: Vec<&str> = text.lines().take(3).collect();
        println!("  First 3 lines:\n    {}", lines.join("\n    "));
    }

    // Test cdns
    let response = client.get(&format!("/{}/cdns", product)).await.unwrap();
    println!("/{}/cdns - Status: {}", product, response.status());

    // Test bgdl
    let response = client.get(&format!("/{}/bgdl", product)).await.unwrap();
    println!("/{}/bgdl - Status: {}", product, response.status());

    // Test blobs
    let response = client.get(&format!("/{}/blobs", product)).await.unwrap();
    println!("/{}/blobs - Status: {}", product, response.status());

    // Test cert endpoint with product
    let response = client.get(&format!("/{}/cert", product)).await.unwrap();
    println!("/{}/cert - Status: {}", product, response.status());

    let response = client.get(&format!("/{}/certs", product)).await.unwrap();
    println!("/{}/certs - Status: {}", product, response.status());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;

    // Test different known products
    let products = vec![
        "wow",         // World of Warcraft
        "wow_classic", // WoW Classic
        "wowt",        // WoW Test
        "wow_beta",    // WoW Beta
        "agent",       // Battle.net Agent
        "bna",         // Battle.net App
        "pro",         // Overwatch
        "s2",          // StarCraft II
        "d3",          // Diablo 3
        "hero",        // Heroes of the Storm
        "hsb",         // Hearthstone
        "w3",          // Warcraft III
    ];

    for product in products {
        test_product(&client, product).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Test certs endpoint directly
    println!("\n\n{:=^60}", " Testing Direct Endpoints ");

    // Test the direct certs endpoint (no product)
    let response = client.get("/certs").await?;
    println!("/certs - Status: {}", response.status());
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response:\n{}", text);
    }

    Ok(())
}

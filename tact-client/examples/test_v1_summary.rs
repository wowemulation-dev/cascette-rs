//! Test v1 summary endpoint

use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Test v1 summary (no product)
    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;

    println!("Testing V1 summary endpoint...\n");

    // Try direct summary endpoint
    let response = client.get("/summary").await?;
    println!("GET /summary - Status: {}", response.status());
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response:\n{}", text);
    }

    // Try v1/summary
    let response = client.get("/v1/summary").await?;
    println!("\nGET /v1/summary - Status: {}", response.status());
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response:\n{}", text);
    }

    // Try without leading slash
    let response = client.get("summary").await?;
    println!("\nGET summary - Status: {}", response.status());
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response:\n{}", text);
    }

    // Test blob endpoints with query params
    println!("\n\nTesting blob endpoints with query params...");

    // Try blobs with region
    let response = client.get("/wow/blobs?region=us").await?;
    println!("\nGET /wow/blobs?region=us - Status: {}", response.status());
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response preview:\n{}", &text[..text.len().min(500)]);
    }

    // Try blob/game with region
    let response = client.get("/wow/blob/game?region=us").await?;
    println!(
        "\nGET /wow/blob/game?region=us - Status: {}",
        response.status()
    );
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response preview:\n{}", &text[..text.len().min(500)]);
    }

    // Test certs endpoint
    let response = client.get("/certs").await?;
    println!("\nGET /certs - Status: {}", response.status());
    if response.status().is_success() {
        let text = response.text().await?;
        println!("Response preview:\n{}", &text[..text.len().min(500)]);
    }

    Ok(())
}

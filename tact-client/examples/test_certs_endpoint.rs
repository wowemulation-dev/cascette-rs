//! Test certs endpoint variations

use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;

    println!("Testing certs endpoint variations...\n");

    // Try different cert endpoint patterns
    let endpoints = vec![
        "/certs",
        "/cert",
        "/certificates",
        "/pki",
        "/ca",
        "/wow/cert",
        "/wow/certs",
        "/ribbit/certs",
        "/ribbit/cert",
        "/v1/certs",
        "/v1/cert",
        "/certs/download",
        "/certs/list",
        "/certs/all",
    ];

    for endpoint in endpoints {
        let response = client.get(endpoint).await?;
        println!("{} - Status: {}", endpoint, response.status());

        if response.status().is_success() {
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown");
            println!("  Content-Type: {}", content_type);

            let text = response.text().await?;
            if text.len() > 200 {
                println!("  Response preview: {}...", &text[..200]);
            } else {
                println!("  Response: {}", text);
            }
        }
    }

    // Try certs with specific product query params
    println!("\n\nTesting certs with query parameters...\n");

    let query_endpoints = vec![
        "/certs?product=wow",
        "/cert?product=wow",
        "/certs?app=wow",
        "/cert?app=wow",
    ];

    for endpoint in query_endpoints {
        let response = client.get(endpoint).await?;
        println!("{} - Status: {}", endpoint, response.status());
    }

    // Check if certs is actually just a download command
    println!("\n\nTesting as download command...\n");

    let response = client.get("/certs download").await?;
    println!("/certs download - Status: {}", response.status());

    // Also test on ngdp-cache Ribbit client we saw earlier
    println!("\n\nNote: Based on ngdp-cache code, certs might be a Ribbit command like:");
    println!("  v1/certs/ribbit/list");
    println!("  v1/ribbit/certs");
    println!("But these would be TCP commands, not HTTP endpoints");

    Ok(())
}

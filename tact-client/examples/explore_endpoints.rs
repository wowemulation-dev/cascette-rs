//! Explore all TACT protocol endpoints

use tact_client::{HttpClient, ProtocolVersion, Region};
use tracing::{error, info, warn};

#[derive(Debug)]
struct EndpointResult {
    endpoint: String,
    status: u16,
    content_type: Option<String>,
    content_length: Option<u64>,
    first_bytes: String,
}

async fn test_endpoint(client: &HttpClient, path: &str) -> EndpointResult {
    match client.get(path).await {
        Ok(response) => {
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let content_length = response.content_length();

            let first_bytes = match response.text().await {
                Ok(text) => {
                    if text.len() > 500 {
                        format!("{}...", &text[..500])
                    } else {
                        text
                    }
                }
                Err(e) => format!("Error reading body: {}", e),
            };

            EndpointResult {
                endpoint: path.to_string(),
                status,
                content_type,
                content_length,
                first_bytes,
            }
        }
        Err(e) => EndpointResult {
            endpoint: path.to_string(),
            status: 0,
            content_type: None,
            content_length: None,
            first_bytes: format!("Request failed: {}", e),
        },
    }
}

async fn explore_v1_endpoints(product: &str) {
    info!("=== Exploring TACT v1 endpoints for {} ===", product);

    let client = match HttpClient::new(Region::US, ProtocolVersion::V1) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create v1 client: {}", e);
            return;
        }
    };

    // Known v1 endpoints
    let endpoints = vec![
        format!("/{}/versions", product),
        format!("/{}/cdns", product),
        format!("/{}/bgdl", product),
    ];

    let mut results = Vec::new();

    for endpoint in endpoints {
        info!("Testing endpoint: {}", endpoint);
        let result = test_endpoint(&client, &endpoint).await;

        if result.status == 200 {
            info!("✓ {} - Status: {}", endpoint, result.status);
        } else if result.status == 404 {
            warn!("✗ {} - Not found", endpoint);
        } else if result.status == 0 {
            error!("✗ {} - Failed to connect", endpoint);
        } else {
            warn!("? {} - Status: {}", endpoint, result.status);
        }

        results.push(result);
    }

    // Print summary
    println!("\n=== V1 Results Summary ===");
    for result in results {
        println!("\nEndpoint: {}", result.endpoint);
        println!("Status: {}", result.status);
        if let Some(ct) = &result.content_type {
            println!("Content-Type: {}", ct);
        }
        if let Some(cl) = result.content_length {
            println!("Content-Length: {}", cl);
        }
        if result.status == 200 {
            println!("Response preview:\n{}", result.first_bytes);
        }
        println!("{}", "-".repeat(60));
    }
}

async fn explore_v2_endpoints(product: &str) {
    info!("=== Exploring TACT v2 endpoints for {} ===", product);

    let client = match HttpClient::new(Region::US, ProtocolVersion::V2) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create v2 client: {}", e);
            return;
        }
    };

    // v2 base is https://{region}.version.battle.net/v2/products
    let endpoints = vec![
        format!("/{}/versions", product),
        format!("/{}/cdns", product),
        format!("/{}/bgdl", product),
    ];

    let mut results = Vec::new();

    for endpoint in endpoints {
        info!("Testing endpoint: {}", endpoint);
        let result = test_endpoint(&client, &endpoint).await;

        if result.status == 200 {
            info!("✓ {} - Status: {}", endpoint, result.status);
        } else if result.status == 404 {
            warn!("✗ {} - Not found", endpoint);
        } else if result.status == 0 {
            error!("✗ {} - Failed to connect", endpoint);
        } else {
            warn!("? {} - Status: {}", endpoint, result.status);
        }

        results.push(result);
    }

    // Print summary
    println!("\n=== V2 Results Summary ===");
    for result in results {
        println!("\nEndpoint: {}", result.endpoint);
        println!("Status: {}", result.status);
        if let Some(ct) = &result.content_type {
            println!("Content-Type: {}", ct);
        }
        if let Some(cl) = result.content_length {
            println!("Content-Length: {}", cl);
        }
        if result.status == 200 {
            println!("Response preview:\n{}", result.first_bytes);
        }
        println!("{}", "-".repeat(60));
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Test a single product first
    println!("Testing World of Warcraft endpoints...\n");
    explore_v1_endpoints("wow").await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    explore_v2_endpoints("wow").await;
}

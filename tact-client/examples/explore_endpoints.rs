//! Comprehensive TACT protocol endpoint exploration
//!
//! This example demonstrates:
//! - V1 and V2 protocol endpoint testing
//! - Multi-product endpoint discovery
//! - Certificate endpoint variations
//! - Summary and blob endpoint testing
//! - Different query parameter combinations
//!
//! Run with: `cargo run --example explore_endpoints`

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
                Err(e) => format!("Error reading body: {e}"),
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
            first_bytes: format!("Request failed: {e}"),
        },
    }
}

async fn explore_single_product_endpoints(product: &str) {
    info!("=== Exploring endpoints for {} ===", product);

    // Test both V1 and V2
    explore_v1_endpoints(product).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    explore_v2_endpoints(product).await;
}

async fn explore_v1_endpoints(product: &str) {
    info!("=== TACT v1 endpoints for {} ===", product);

    let client = match HttpClient::new(Region::US, ProtocolVersion::V1) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create v1 client: {}", e);
            return;
        }
    };

    // Standard product endpoints
    let endpoints = vec![
        format!("/{}/versions", product),
        format!("/{}/cdns", product),
        format!("/{}/bgdl", product),
        format!("/{}/blobs", product),
        format!("/{}/cert", product),
        format!("/{}/certs", product),
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
    println!("\n=== V1 Results Summary for {product} ===");
    for result in results {
        println!("\nEndpoint: {}", result.endpoint);
        println!("Status: {}", result.status);
        if let Some(ct) = &result.content_type {
            println!("Content-Type: {ct}");
        }
        if let Some(cl) = result.content_length {
            println!("Content-Length: {cl}");
        }
        if result.status == 200 {
            let preview = if result.first_bytes.len() > 300 {
                format!("{}...", &result.first_bytes[..300])
            } else {
                result.first_bytes.clone()
            };
            println!("Response preview:\n{preview}");
        }
        println!("{}", "-".repeat(60));
    }
}

async fn explore_v2_endpoints(product: &str) {
    info!("=== TACT v2 endpoints for {} ===", product);

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
    println!("\n=== V2 Results Summary for {product} ===");
    for result in results {
        println!("\nEndpoint: {}", result.endpoint);
        println!("Status: {}", result.status);
        if let Some(ct) = &result.content_type {
            println!("Content-Type: {ct}");
        }
        if let Some(cl) = result.content_length {
            println!("Content-Length: {cl}");
        }
        if result.status == 200 {
            let preview = if result.first_bytes.len() > 300 {
                format!("{}...", &result.first_bytes[..300])
            } else {
                result.first_bytes.clone()
            };
            println!("Response preview:\n{preview}");
        }
        println!("{}", "-".repeat(60));
    }
}

async fn explore_certificate_endpoints() {
    info!("=== Exploring certificate endpoint variations ===");

    let client = match HttpClient::new(Region::US, ProtocolVersion::V1) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create v1 client: {}", e);
            return;
        }
    };

    println!("\n=== Certificate Endpoint Variations ===");

    // Certificate endpoint patterns
    let cert_endpoints = vec![
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

    for endpoint in cert_endpoints {
        let result = test_endpoint(&client, endpoint).await;
        print_result_summary(&result);
    }

    // Certificate endpoints with query parameters
    println!("\n=== Certificate Endpoints with Query Parameters ===");
    let query_endpoints = vec![
        "/certs?product=wow",
        "/cert?product=wow",
        "/certs?app=wow",
        "/cert?app=wow",
    ];

    for endpoint in query_endpoints {
        let result = test_endpoint(&client, endpoint).await;
        print_result_summary(&result);
    }

    // Certificate download command test
    println!("\n=== Certificate Download Command Test ===");
    let result = test_endpoint(&client, "/certs download").await;
    print_result_summary(&result);

    println!("\nNote: Based on documentation, certs might be a Ribbit TCP command:");
    println!("  v1/certs/ribbit/list");
    println!("  v1/ribbit/certs");
    println!("These would be TCP commands, not HTTP endpoints");
}

async fn explore_summary_and_blob_endpoints() {
    info!("=== Exploring summary and blob endpoints ===");

    let client = match HttpClient::new(Region::US, ProtocolVersion::V1) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create v1 client: {}", e);
            return;
        }
    };

    println!("\n=== Summary Endpoint Variations ===");

    // Summary endpoint variations
    let summary_endpoints = vec![
        "/summary",
        "/v1/summary",
        "summary", // without leading slash
    ];

    for endpoint in summary_endpoints {
        let result = test_endpoint(&client, endpoint).await;
        print_result_summary(&result);
    }

    println!("\n=== Blob Endpoints with Query Parameters ===");

    // Blob endpoints with query parameters
    let blob_endpoints = vec!["/wow/blobs?region=us", "/wow/blob/game?region=us"];

    for endpoint in blob_endpoints {
        let result = test_endpoint(&client, endpoint).await;
        print_result_summary(&result);
    }
}

async fn explore_multiple_products() {
    info!("=== Testing multiple Blizzard products ===");

    let client = match HttpClient::new(Region::US, ProtocolVersion::V1) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create v1 client: {}", e);
            return;
        }
    };

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

    println!("\n=== Multi-Product Endpoint Testing ===");

    for product in products {
        println!("\n{:=^60}", format!(" Testing {} ", product));

        // Test core endpoints for each product
        let endpoints = vec![
            format!("/{}/versions", product),
            format!("/{}/cdns", product),
            format!("/{}/bgdl", product),
        ];

        for endpoint in endpoints {
            let result = test_endpoint(&client, &endpoint).await;
            println!("{} - Status: {}", endpoint, result.status);

            if result.status == 200 && endpoint.contains("versions") {
                // Show first few lines for versions endpoint
                let lines: Vec<&str> = result.first_bytes.lines().take(3).collect();
                if !lines.is_empty() {
                    println!("  First 3 lines:\n    {}", lines.join("\n    "));
                }
            }
        }

        // Small delay between products
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}

fn print_result_summary(result: &EndpointResult) {
    println!("{} - Status: {}", result.endpoint, result.status);

    if result.status == 200 {
        if let Some(ct) = &result.content_type {
            println!("  Content-Type: {ct}");
        }

        let preview = if result.first_bytes.len() > 200 {
            format!("{}...", &result.first_bytes[..200])
        } else {
            result.first_bytes.clone()
        };

        if !preview.trim().is_empty() {
            println!("  Response preview: {preview}");
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Comprehensive TACT Protocol Endpoint Exploration ===");
    println!("This explores V1/V2 protocols, multiple products, and various endpoint patterns\n");

    // 1. Test single product in depth
    println!("\n🎯 Single Product Deep Dive (WoW):");
    explore_single_product_endpoints("wow").await;

    // 2. Test certificate endpoints
    println!("\n🔐 Certificate Endpoint Discovery:");
    explore_certificate_endpoints().await;

    // 3. Test summary and blob endpoints
    println!("\n📋 Summary and Blob Endpoints:");
    explore_summary_and_blob_endpoints().await;

    // 4. Test multiple products
    println!("\n🎮 Multi-Product Testing:");
    explore_multiple_products().await;

    println!("\n=== Exploration Complete ===");
    println!("This comprehensive test covers:");
    println!("  ✓ V1 and V2 protocol differences");
    println!("  ✓ Standard product endpoints (versions, cdns, bgdl)");
    println!("  ✓ Certificate endpoint variations");
    println!("  ✓ Summary and blob endpoints");
    println!("  ✓ Query parameter combinations");
    println!("  ✓ Multiple Blizzard product testing");
}

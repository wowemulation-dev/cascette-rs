//! Example demonstrating the products cdns command with region filtering

use ngdp_client::{OutputFormat, ProductsCommands, handle_products};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== CDNs Region Filtering Demo ===\n");

    // Test 1: CDN config for specific region
    println!("1. CDN configuration for wow in US region:");
    println!("   Command: ngdp products cdns wow --region us\n");

    match handle_products(
        ProductsCommands::Cdns {
            product: "wow".to_string(),
            region: "us".to_string(),
        },
        OutputFormat::Text,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success"),
        Err(e) => eprintln!("\n✗ Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(60));

    // Test 2: CDN config for EU region
    println!("2. CDN configuration for wow in EU region:");
    println!("   Command: ngdp products cdns wow --region eu\n");

    match handle_products(
        ProductsCommands::Cdns {
            product: "wow".to_string(),
            region: "eu".to_string(),
        },
        OutputFormat::Text,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success"),
        Err(e) => eprintln!("\n✗ Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(60));

    // Test 3: JSON output for CN region
    println!("3. CDN configuration for wow in CN region (JSON):");
    println!("   Command: ngdp products cdns wow --region cn -o json-pretty\n");

    match handle_products(
        ProductsCommands::Cdns {
            product: "wow".to_string(),
            region: "cn".to_string(),
        },
        OutputFormat::JsonPretty,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success"),
        Err(e) => eprintln!("\n✗ Error: {}", e),
    }

    Ok(())
}

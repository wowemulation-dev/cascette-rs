//! Example demonstrating the products info command behavior with regions

use ngdp_client::{OutputFormat, ProductsCommands, handle_products};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Products Info Demo ===\n");

    // Test 1: Info for specific region
    println!("1. Product info for wow_classic_era in EU region:");
    println!("   Command: ngdp products info wow_classic_era --region eu\n");

    match handle_products(
        ProductsCommands::Info {
            product: "wow_classic_era".to_string(),
            region: Some("eu".to_string()),
        },
        OutputFormat::Text,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success"),
        Err(e) => eprintln!("\n✗ Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(60));

    // Test 2: Info for all regions (no region specified)
    println!("2. Product info for wow_classic_era (all regions):");
    println!("   Command: ngdp products info wow_classic_era\n");

    match handle_products(
        ProductsCommands::Info {
            product: "wow_classic_era".to_string(),
            region: None,
        },
        OutputFormat::Text,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success"),
        Err(e) => eprintln!("\n✗ Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(60));

    // Test 3: JSON output for specific region
    println!("3. Product info for wow in US region (JSON format):");
    println!("   Command: ngdp products info wow --region us -o json-pretty\n");

    match handle_products(
        ProductsCommands::Info {
            product: "wow".to_string(),
            region: Some("us".to_string()),
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

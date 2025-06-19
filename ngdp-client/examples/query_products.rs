//! Example of using ngdp-client as a library to query product information

use ngdp_client::{OutputFormat, ProductsCommands, handle_products};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debugging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Example 1: List all WoW-related products
    println!("=== Listing WoW products ===");
    let cmd = ProductsCommands::List {
        filter: Some("wow".to_string()),
        region: "us".to_string(),
    };

    handle_products(cmd, OutputFormat::Text).await?;

    println!("\n=== Same query in JSON format ===");
    let cmd = ProductsCommands::List {
        filter: Some("wow".to_string()),
        region: "us".to_string(),
    };

    handle_products(cmd, OutputFormat::Json).await?;

    Ok(())
}

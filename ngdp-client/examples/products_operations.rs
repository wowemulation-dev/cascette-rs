//! Comprehensive products operations example
//!
//! This example demonstrates all product-related commands:
//! - Listing products with filters
//! - Getting product information for specific/all regions
//! - Retrieving CDN configurations across regions
//! - Using different output formats (Text/JSON)
//!
//! Run with: `cargo run --example products_operations`

use ngdp_client::{OutputFormat, ProductsCommands, handle_products};

/// Demonstrate product listing with filters
async fn product_listing_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Product Listing Operations ===\n");

    // Example 1: List all WoW-related products
    println!("1. Listing WoW products (Text format):");
    println!("   Command: ngdp products list --filter wow --region us\n");

    let cmd = ProductsCommands::List {
        filter: Some("wow".to_string()),
        region: "us".to_string(),
    };
    handle_products(cmd, OutputFormat::Text).await?;

    // Example 2: Same query in JSON format
    println!("\n2. Same query in JSON format:");
    println!("   Command: ngdp products list --filter wow --region us --output json\n");

    let cmd = ProductsCommands::List {
        filter: Some("wow".to_string()),
        region: "us".to_string(),
    };
    handle_products(cmd, OutputFormat::Json).await?;

    // Example 3: List all products (no filter)
    println!("\n3. Listing all products:");
    println!("   Command: ngdp products list --region us\n");

    let cmd = ProductsCommands::List {
        filter: None,
        region: "us".to_string(),
    };
    match handle_products(cmd, OutputFormat::Text).await {
        Ok(_) => println!("\n✓ Success"),
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    Ok(())
}

/// Demonstrate product information queries
async fn product_info_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Product Information Operations ===\n");

    // Example 1: Info for specific region
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
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    // Example 2: Info for all regions (no region specified)
    println!("\n2. Product info for wow_classic_era (all regions):");
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
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    // Example 3: JSON output for specific region
    println!("\n3. Product info in JSON format:");
    println!("   Command: ngdp products info wow --region us --output json\n");

    match handle_products(
        ProductsCommands::Info {
            product: "wow".to_string(),
            region: Some("us".to_string()),
        },
        OutputFormat::Json,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success (JSON output shown above)"),
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    Ok(())
}

/// Demonstrate CDN configuration queries
async fn cdn_configuration_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CDN Configuration Operations ===\n");

    // Example 1: CDN config for US region
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
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    // Example 2: CDN config for EU region
    println!("\n2. CDN configuration for wow in EU region:");
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
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    // Example 3: CDN config for CN region in JSON format
    println!("\n3. CDN configuration for wow in CN region (JSON):");
    println!("   Command: ngdp products cdns wow --region cn --output json\n");

    match handle_products(
        ProductsCommands::Cdns {
            product: "wow".to_string(),
            region: "cn".to_string(),
        },
        OutputFormat::Json,
    )
    .await
    {
        Ok(_) => println!("\n✓ Success (JSON output shown above)"),
        Err(e) => eprintln!("\n✗ Error: {e}"),
    }

    Ok(())
}

/// Demonstrate cross-region comparisons
async fn cross_region_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cross-Region Comparison ===\n");

    let regions = ["us", "eu", "kr"];
    let product = "wow";

    println!("Comparing CDN configurations across regions for {product}:");

    for (i, region) in regions.iter().enumerate() {
        println!("\n{}. {} region:", i + 1, region.to_uppercase());
        println!("   Command: ngdp products cdns {product} --region {region}");

        match handle_products(
            ProductsCommands::Cdns {
                product: product.to_string(),
                region: region.to_string(),
            },
            OutputFormat::Text,
        )
        .await
        {
            Ok(_) => println!("   ✓ Success"),
            Err(e) => eprintln!("   ✗ Error: {e}"),
        }
    }

    Ok(())
}

/// Demonstrate output format variations
async fn output_format_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Output Format Variations ===\n");

    let product = "wow";
    let region = "us";

    // Text format
    println!("1. Text format output:");
    println!("   Command: ngdp products info {product} --region {region}");
    match handle_products(
        ProductsCommands::Info {
            product: product.to_string(),
            region: Some(region.to_string()),
        },
        OutputFormat::Text,
    )
    .await
    {
        Ok(_) => println!("   ✓ Text format success"),
        Err(e) => eprintln!("   ✗ Text format error: {e}"),
    }

    // JSON format
    println!("\n2. JSON format output:");
    println!("   Command: ngdp products info {product} --region {region} --output json");
    match handle_products(
        ProductsCommands::Info {
            product: product.to_string(),
            region: Some(region.to_string()),
        },
        OutputFormat::Json,
    )
    .await
    {
        Ok(_) => println!("   ✓ JSON format success"),
        Err(e) => eprintln!("   ✗ JSON format error: {e}"),
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Comprehensive Products Operations Example ===\n");

    // Run all demonstrations
    product_listing_demo().await?;
    println!("\n{}\n", "=".repeat(60));

    product_info_demo().await?;
    println!("\n{}\n", "=".repeat(60));

    cdn_configuration_demo().await?;
    println!("\n{}\n", "=".repeat(60));

    cross_region_demo().await?;
    println!("\n{}\n", "=".repeat(60));

    output_format_demo().await?;

    println!("\n=== Example Complete ===");
    println!("This example demonstrated:");
    println!("  ✓ Product listing with filtering");
    println!("  ✓ Product information queries (single/all regions)");
    println!("  ✓ CDN configuration retrieval");
    println!("  ✓ Cross-region comparisons");
    println!("  ✓ Multiple output formats (Text/JSON)");
    println!("  ✓ Error handling for various scenarios");

    Ok(())
}

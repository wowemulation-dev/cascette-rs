//! Simple example matching Ribbit.NET's test program
//!
//! This demonstrates the simplicity of the typed API

use ribbit_client::{Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client for US region (just like Ribbit.NET)
    let client = RibbitClient::new(Region::US);

    // Request WoW versions and print them
    let versions = client.get_product_versions("wow").await?;
    println!(
        "{}",
        versions
            .entries
            .iter()
            .map(|e| format!("{}: {} (build {})", e.region, e.versions_name, e.build_id))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Request Overwatch versions
    let versions = client.get_product_versions("pro").await?;
    println!("\nOverwatch versions:");
    println!(
        "{}",
        versions
            .entries
            .iter()
            .map(|e| format!("{}: {} (build {})", e.region, e.versions_name, e.build_id))
            .collect::<Vec<_>>()
            .join("\n")
    );

    Ok(())
}

//! Debug with trace logging
//!
//! Run with: `RUST_LOG=ribbit_client=trace cargo run --example trace_debug`

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable trace logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    let client = RibbitClient::new(Region::US);

    println!("Trace Debug - Signature Parsing");
    println!("{:=<60}\n", "");

    // Get product versions which should have a signature
    let endpoint = Endpoint::ProductVersions("wow".to_string());

    match client.request(&endpoint).await {
        Ok(response) => {
            println!("Response received");

            if let Some(mime_parts) = response.mime_parts {
                println!("\nMIME parts found:");
                println!("  Data: {} bytes", mime_parts.data.len());
                println!(
                    "  Signature: {}",
                    if let Some(sig) = &mime_parts.signature {
                        format!("{} bytes", sig.len())
                    } else {
                        "None".to_string()
                    }
                );
                println!(
                    "  Checksum: {}",
                    mime_parts.checksum.as_deref().unwrap_or("None")
                );

                if let Some(sig_info) = &mime_parts.signature_info {
                    println!("\nSignature info: {sig_info:?}");
                }
            }
        }
        Err(e) => println!("Error: {e}"),
    }

    Ok(())
}

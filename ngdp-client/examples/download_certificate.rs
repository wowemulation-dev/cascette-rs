//! Example of using ngdp-client to download certificates
//!
//! This example demonstrates:
//! - Downloading certificates by SKI/hash
//! - Saving certificates in different formats (PEM/DER)
//! - Extracting certificate information
//!
//! Run with: `cargo run --example download_certificate`

use ngdp_client::{CertFormat, CertsCommands, OutputFormat, commands};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Certificate Download Example ===\n");

    // Known certificate hash (DigiCert SHA2 High Assurance Server CA)
    let cert_hash = "5168ff90af0207753cccd9656462a212b859723b";

    // Example 1: Download and display certificate details
    println!("1. Downloading certificate and showing details:");
    let cmd = CertsCommands::Download {
        ski: cert_hash.to_string(),
        output: None,
        region: "us".to_string(),
        cert_format: CertFormat::Pem,
        details: true,
    };

    commands::certs::handle(cmd, OutputFormat::Text).await?;

    // Example 2: Save certificate to PEM file
    println!("\n2. Saving certificate to PEM file:");
    let pem_path = Path::new("example-cert.pem");
    let cmd = CertsCommands::Download {
        ski: cert_hash.to_string(),
        output: Some(pem_path.to_path_buf()),
        region: "us".to_string(),
        cert_format: CertFormat::Pem,
        details: false,
    };

    commands::certs::handle(cmd, OutputFormat::Text).await?;
    println!("Certificate saved to: {}", pem_path.display());

    // Example 3: Save certificate to DER file
    println!("\n3. Saving certificate to DER file:");
    let der_path = Path::new("example-cert.der");
    let cmd = CertsCommands::Download {
        ski: cert_hash.to_string(),
        output: Some(der_path.to_path_buf()),
        region: "us".to_string(),
        cert_format: CertFormat::Der,
        details: false,
    };

    commands::certs::handle(cmd, OutputFormat::Text).await?;
    println!("Certificate saved to: {}", der_path.display());

    // Example 4: Get certificate details as JSON
    println!("\n4. Getting certificate details as JSON:");
    let _cmd = CertsCommands::Download {
        ski: cert_hash.to_string(),
        output: None,
        region: "us".to_string(),
        cert_format: CertFormat::Pem,
        details: true,
    };

    // In a real application, you would call:
    // commands::certs::handle(cmd, OutputFormat::Json).await?;
    println!("(JSON output would appear here in normal usage)");

    // Clean up example files
    if pem_path.exists() {
        std::fs::remove_file(pem_path)?;
        println!("\nCleaned up: {}", pem_path.display());
    }
    if der_path.exists() {
        std::fs::remove_file(der_path)?;
        println!("Cleaned up: {}", der_path.display());
    }

    println!("\n=== Example Complete ===");

    Ok(())
}

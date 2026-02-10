//! Simple Ribbit server example.
//!
//! This example demonstrates minimal setup for a Ribbit server with a small test database.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example simple_server
//! ```
//!
//! Then test with:
//! ```bash
//! # HTTP endpoints
//! curl http://localhost:8080/wow/versions
//! curl http://localhost:8080/wow/cdns
//! curl http://localhost:8080/wow/bgdl
//!
//! # TCP v2 protocol
//! echo "v2/products/wow/versions" | nc localhost 1119
//!
//! # TCP v1 protocol (MIME-wrapped)
//! echo "v1/products/wow/versions" | nc localhost 1119
//! echo "v1/summary" | nc localhost 1119
//! ```

#![allow(clippy::expect_used)]

use anyhow::Result;
use cascette_ribbit::{Server, ServerConfig};
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create a temporary database file with sample builds
    let mut db_file = NamedTempFile::new()?;
    let sample_builds = r#"[
        {
            "id": 1,
            "product": "wow",
            "version": "1.14.2.42597",
            "build": "42597",
            "build_config": "0123456789abcdef0123456789abcdef",
            "cdn_config": "fedcba9876543210fedcba9876543210",
            "product_config": null,
            "build_time": "2024-01-01T00:00:00+00:00",
            "encoding_ekey": "aaaabbbbccccddddeeeeffffaaaaffff",
            "root_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
            "install_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
            "download_ekey": "ddddeeeeffffaaaabbbbccccddddeeee"
        },
        {
            "id": 2,
            "product": "wowt",
            "version": "11.0.7.58187",
            "build": "58187",
            "build_config": "1234567890abcdef1234567890abcdef",
            "cdn_config": "edcba9876543210fedcba9876543210f",
            "product_config": null,
            "build_time": "2024-06-01T00:00:00+00:00",
            "encoding_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
            "root_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
            "install_ekey": "ddddeeeeffffaaaabbbbccccddddeeee",
            "download_ekey": "eeeeffffaaaabbbbccccddddeeeeaaaa"
        }
    ]"#;
    db_file.write_all(sample_builds.as_bytes())?;
    db_file.flush()?;

    // Create server configuration
    let config = ServerConfig {
        http_bind: "127.0.0.1:8080".parse()?,
        tcp_bind: "127.0.0.1:1119".parse()?,
        builds: db_file.path().to_path_buf(),
        cdn_hosts: "cdn.arctium.tools".to_string(),
        cdn_path: "tpr/wow".to_string(),
        tls_cert: None,
        tls_key: None,
    };

    // Validate configuration
    config.validate()?;

    // Create and run server
    let server = Server::new(config)?;

    tracing::info!("Starting simple Ribbit server");
    tracing::info!("HTTP endpoints: http://127.0.0.1:8080/{{product}}/{{endpoint}}");
    tracing::info!("TCP port: 127.0.0.1:1119");
    tracing::info!("Press Ctrl+C to stop");

    server.run().await?;

    Ok(())
}

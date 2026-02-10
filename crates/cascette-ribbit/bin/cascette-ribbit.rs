//! Cascette Ribbit Server binary entry point.
//!
//! This is a thin wrapper around the cascette-ribbit library that:
//! 1. Parses command-line arguments
//! 2. Initializes logging
//! 3. Loads configuration
//! 4. Starts the server
//!
//! For library usage, see the cascette-ribbit crate documentation.

use anyhow::Result;
use cascette_ribbit::{Server, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Cascette Ribbit Server starting...");

    // Parse configuration from CLI args
    let config = ServerConfig::from_args();

    tracing::info!(
        "Configuration loaded: HTTP={}, TCP={}, builds={:?}",
        config.http_bind,
        config.tcp_bind,
        config.builds
    );

    // Validate configuration
    config.validate()?;

    // Create and run server
    let server = Server::new(config)?;
    server.run().await?;

    Ok(())
}

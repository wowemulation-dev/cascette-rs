//! Ribbit server implementation.
//!
//! This crate provides a replacement for Blizzard's Ribbit service,
//! supporting all protocol variants:
//! - TCP Ribbit v1 (MIME-wrapped with SHA-256 checksums)
//! - TCP Ribbit v2 (raw BPSV)
//! - HTTP/HTTPS TACT v2 (raw BPSV)
//!
//! # Architecture
//!
//! The server uses a library-first design with the following components:
//! - `server`: Main server orchestration (HTTP + TCP listeners)
//! - `config`: Configuration loading and validation
//! - `database`: JSON database loading and indexing
//! - `http`: HTTP server and handlers
//! - `tcp`: TCP server and handlers
//! - `responses`: BPSV/MIME generation and checksums
//!
//! # Example
//!
//! ```no_run
//! use cascette_ribbit::{Server, ServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize logging
//!     tracing_subscriber::fmt::init();
//!
//!     // Load configuration from CLI args and environment
//!     let config = ServerConfig::from_args();
//!     config.validate()?;
//!
//!     // Create and run server
//!     let server = Server::new(config)?;
//!     server.run().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Multi-Protocol**: HTTP/HTTPS and TCP v1/v2 protocols
//! - **Multi-Region**: Automatic 5-region support (us, eu, kr, tw, cn)
//! - **Performance**: O(1) product lookups, async I/O
//! - **Standards Compliant**: RFC 2046 MIME, SHA-256 checksums

#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

// Module declarations
pub mod config;
pub mod database;
pub mod error;
pub mod http;
pub mod responses;
pub mod server;
pub mod tcp;

// Re-exports for public API
pub use config::{CdnConfig, ServerConfig};
pub use database::{BuildDatabase, BuildRecord};
pub use error::{ConfigError, DatabaseError, ProtocolError, ServerError};
pub use responses::BpsvResponse;
pub use server::{AppState, Server};

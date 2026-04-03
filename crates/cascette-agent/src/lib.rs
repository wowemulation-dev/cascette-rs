//! Local HTTP agent service compatible with Blizzard Agent.exe (version 3.13.3).
//!
//! This crate implements a REST API on port 1120 that manages game product
//! installations, updates, repairs, and verification. The API surface matches
//! the real Blizzard agent endpoints to enable drop-in replacement with the
//! Battle.net launcher.
//!
//! # Architecture
//!
//! - `config`: CLI flags and configuration (matches real agent parameters)
//! - `error`: Error types (thiserror for library, anyhow in binary)
//! - `models`: Domain models (Operation, Product, Progress state machines)
//! - `state`: SQLite persistence via turso (products, operations, queue)
//! - `server`: Axum HTTP server with real agent endpoint paths
//! - `executor`: Background operation runner with pipeline integration
//! - `process_detection`: Cross-platform game process detection
//! - `observability`: Tracing and Prometheus metrics
//!
//! # Example
//!
//! ```no_run
//! use cascette_agent::config::AgentConfig;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = AgentConfig::from_args();
//!     // Server startup handled by the binary entry point
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod config;
pub mod error;
pub mod executor;
pub mod models;
pub mod observability;
pub mod process_detection;
pub mod server;
pub mod session;
pub mod state;

// Re-exports for public API
pub use config::AgentConfig;
pub use error::{AgentError, AgentResult};
pub use observability::Metrics;

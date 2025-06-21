//! NGDP CDN client for downloading game content
//!
//! This crate provides an async HTTP client specifically designed for downloading
//! content from Blizzard's CDN servers. It includes:
//!
//! - Connection pooling for efficient multiple downloads
//! - Automatic retry with exponential backoff
//! - Support for gzip/deflate compression
//! - Configurable timeouts and retry policies
//!
//! # Example
//!
//! ```no_run
//! use ngdp_cdn::CdnClient;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a CDN client with default configuration
//! let client = CdnClient::new()?;
//!
//! // Download a file by hash
//! let response = client.download(
//!     "blzddist1-a.akamaihd.net",
//!     "tpr/wow",
//!     "2e9c1e3b5f5a0c9d9e8f1234567890ab",
//! ).await?;
//!
//! let content = response.bytes().await?;
//! println!("Downloaded {} bytes", content.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Advanced Configuration
//!
//! ```no_run
//! use ngdp_cdn::CdnClient;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client with custom retry configuration
//! let client = CdnClient::builder()
//!     .max_retries(5)
//!     .initial_backoff_ms(200)
//!     .max_backoff_ms(30_000)
//!     .connect_timeout(60)
//!     .pool_max_idle_per_host(50)
//!     .build()?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod client;
mod error;

pub use client::{CdnClient, CdnClientBuilder};
pub use error::{Error, Result};

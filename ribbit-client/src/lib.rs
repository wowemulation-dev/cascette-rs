//! Ribbit protocol client for Cascette
//!
//! This crate provides an async TCP client for Blizzard's Ribbit protocol,
//! which is used to retrieve version information, CDN configurations, and
//! other metadata for Blizzard games.
//!
//! # Example
//!
//! ```no_run
//! use ribbit_client::{RibbitClient, Region, Endpoint};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client for the US region
//!     let client = RibbitClient::new(Region::US);
//!
//!     // Request WoW version information
//!     let endpoint = Endpoint::ProductVersions("wow".to_string());
//!     let response = client.request(&endpoint).await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod certificate_fetcher;
pub mod client;
pub mod cms_parser;
pub mod error;
pub mod response_types;
pub mod signature;
pub mod signature_verify;
pub mod types;

pub use client::{Response, RibbitClient};
pub use error::{Error, Result};
pub use response_types::{
    BgdlEntry, CdnEntry, ProductBgdlResponse, ProductCdnsResponse, ProductSummary,
    ProductVersionsResponse, SummaryResponse, TypedResponse, VersionEntry,
};
pub use types::{Endpoint, ProtocolVersion, Region};

//! Response generation for Ribbit protocol.
//!
//! This module handles generation of responses in Blizzard's BPSV (Blizzard Pipe-Separated Values)
//! format, which is used across all Ribbit protocol variants.
//!
//! # BPSV Format
//!
//! BPSV responses consist of:
//! - Header line defining column names and types (e.g., `Region!STRING:0|BuildId!DEC:4`)
//! - Data rows with pipe-separated values
//! - Sequence number footer (`## seqn = {timestamp}`)
//!
//! # Example
//!
//! ```
//! use cascette_ribbit::BpsvResponse;
//!
//! // Generate a CDN response
//! let cdn_config = cascette_ribbit::CdnConfig {
//!     hosts: "cdn.arctium.tools".to_string(),
//!     path: "tpr/wow".to_string(),
//!     servers: "https://cdn.arctium.tools".to_string(),
//!     config_path: "tpr/wow".to_string(),
//! };
//!
//! let response = BpsvResponse::cdns(&cdn_config, 1730534400);
//! let text = response.to_string();
//!
//! assert!(text.contains("Name!STRING:0"));
//! assert!(text.contains("## seqn = 1730534400"));
//! ```

pub mod bpsv;

pub use bpsv::BpsvResponse;

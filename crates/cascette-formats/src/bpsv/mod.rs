//! BPSV (Blizzard Pipe-Separated Values) format support
//!
//! BPSV is a text-based data serialization format used by Blizzard's NGDP system
//! for transmitting version and configuration information via the Ribbit protocol.
//!
//! # Format Overview
//!
//! BPSV files consist of:
//! - A header line with field definitions (name!type:size)
//! - An optional sequence number line (## seqn = N)
//! - Data rows with pipe-separated values
//!
//! # Example
//!
//! ```
//! use cascette_formats::bpsv::{parse, BpsvBuilder, BpsvField, BpsvType, BpsvValue};
//!
//! // Parse BPSV data
//! let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678";
//! let document = parse(content).expect("Test operation should succeed");
//! assert_eq!(document.sequence_number(), Some(12345));
//! assert_eq!(document.row_count(), 2);
//!
//! // Build BPSV data
//! let mut builder = BpsvBuilder::new();
//! builder
//!     .add_field(BpsvField::new("Region", BpsvType::String(0)))
//!     .add_field(BpsvField::new("BuildId", BpsvType::Dec(4)))
//!     .set_sequence(99999);
//!
//! builder.add_row(vec![
//!     BpsvValue::String("us".to_string()),
//!     BpsvValue::Dec(1234),
//! ]).expect("Test operation should succeed");
//!
//! let doc = builder.build();
//! let output = cascette_formats::bpsv::format(&doc);
//! assert!(output.contains("Region!STRING:0|BuildId!DEC:4"));
//! assert!(output.contains("## seqn = 99999"));
//! ```

mod document;
mod reader;
mod row;
mod schema;
mod types;
mod writer;

// #[cfg(feature = "serde")]
// mod serde_impl;

// Re-export main types
pub use document::BpsvDocument;
pub use reader::{BpsvReader, parse, parse_schema};
pub use row::BpsvRow;
pub use schema::BpsvSchema;
pub use types::{BpsvError, BpsvField, BpsvType, BpsvValue};
pub use writer::{BpsvBuilder, BpsvWriter, format, write_to_file};

// #[cfg(feature = "serde")]
// pub use serde_impl::{deserialize, serialize};

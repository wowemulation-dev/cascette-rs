//! # ngdp-bpsv
//!
//! A comprehensive parser and writer for BPSV (Blizzard Pipe-Separated Values) format,
//! used throughout Blizzard's NGDP (Next Generation Distribution Pipeline) system.
//!
//! BPSV is a structured data format with typed columns, sequence numbers, and pipe-separated values.
//!
//! ## Format Structure
//!
//! ```text
//! FieldName!TYPE:length|AnotherField!TYPE:length
//! ## seqn = 12345
//! value1|value2
//! value3|value4
//! ```
//!
//! ## Quick Start
//!
//! ### Parsing BPSV Data
//!
//! ```rust
//! use ngdp_bpsv::BpsvDocument;
//!
//! let data = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678";
//!
//! let doc = BpsvDocument::parse(data)?;
//! println!("Sequence: {:?}", doc.sequence_number());
//! println!("Rows: {}", doc.rows().len());
//! # Ok::<(), ngdp_bpsv::Error>(())
//! ```
//!
//! ### Building BPSV Data
//!
//! ```rust
//! use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};
//!
//! let mut builder = BpsvBuilder::new();
//! builder.add_field("Region", BpsvFieldType::String(0))?;
//! builder.add_field("BuildId", BpsvFieldType::Decimal(4))?;
//! builder.set_sequence_number(12345);
//!
//! builder.add_row(vec![
//!     BpsvValue::String("us".to_string()),
//!     BpsvValue::Decimal(1234),
//! ])?;
//!
//! let bpsv_output = builder.build()?;
//! # Ok::<(), ngdp_bpsv::Error>(())
//! ```

pub mod builder;
pub mod document;
pub mod error;
pub mod field_type;
pub mod parser;
pub mod schema;
pub mod value;

pub use builder::BpsvBuilder;
pub use document::BpsvDocument;
pub use error::{Error, Result};
pub use field_type::BpsvFieldType;
pub use parser::BpsvParser;
pub use schema::{BpsvField, BpsvSchema};
pub use value::BpsvValue;

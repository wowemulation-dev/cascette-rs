//! `ESpec` (Encoding Specification) format support
//!
//! `ESpec` is a domain-specific language used in CASC to define how content
//! should be encoded, compressed, and optionally encrypted. It appears in
//! encoding files and patch manifests to specify data transformation pipelines.
//!
//! # Format
//!
//! `ESpec` strings define compression pipelines:
//! - `n` - No compression
//! - `z[:level[,bits]]` - `ZLib` compression
//! - `e:{key,iv,spec}` - Encryption wrapper
//! - `b:{chunks}` - Block table with per-block specs
//! - `c:{version}` - `BCPack` compression
//! - `g:{level}` - `GDeflate` compression
//!
//! # Examples
//!
//! ## Basic Compression
//!
//! ```
//! use cascette_formats::espec::{ESpec, ZLibBits};
//!
//! // No compression
//! let spec = ESpec::parse("n").expect("Test operation should succeed");
//! assert!(!spec.is_compressed());
//!
//! // ZLib with default settings
//! let spec = ESpec::parse("z").expect("Test operation should succeed");
//! assert!(spec.is_compressed());
//!
//! // ZLib with specific level and window bits
//! let spec = ESpec::parse("z:{9,15}").expect("Test operation should succeed");
//! assert_eq!(spec.to_string(), "z:{9,15}");
//!
//! // MPQ compatibility mode
//! let spec = ESpec::parse("z:{6,mpq}").expect("Test operation should succeed");
//! assert_eq!(spec.compression_type(), "zlib");
//! ```
//!
//! ## Block Tables
//!
//! Block tables allow different compression for different parts of the data:
//!
//! ```
//! use cascette_formats::espec::ESpec;
//!
//! // Streaming optimization: first 1MB uncompressed, rest compressed
//! let spec = ESpec::parse("b:{256K*4=n,*=z:9}").expect("Test operation should succeed");
//!
//! // Mixed compression strategies
//! let spec = ESpec::parse("b:{1768=z,66443=n}").expect("Test operation should succeed");
//!
//! // Complex multi-block specification
//! let spec = ESpec::parse("b:{256K=n,512K*2=z:6,*=z:9}").expect("Test operation should succeed");
//! assert_eq!(spec.to_string(), "b:{256K=n,512K*2=z:6,*=z:9}");
//! ```
//!
//! ## Building Programmatically
//!
//! ```
//! use cascette_formats::espec::{ESpec, ZLibBits, BlockChunk, BlockSizeSpec};
//!
//! // Build a complex block table
//! let spec = ESpec::BlockTable {
//!     chunks: vec![
//!         // First 256KB uncompressed for quick access
//!         BlockChunk {
//!             size_spec: Some(BlockSizeSpec {
//!                 size: 256 * 1024,
//!                 count: None,
//!             }),
//!             spec: ESpec::None,
//!         },
//!         // Remainder with maximum compression
//!         BlockChunk {
//!             size_spec: None,
//!             spec: ESpec::ZLib {
//!                 level: Some(9),
//!                 bits: Some(ZLibBits::Bits(15)),
//!             },
//!         },
//!     ],
//! };
//!
//! // Convert to string and verify round-trip
//! let spec_str = spec.to_string();
//! let parsed = ESpec::parse(&spec_str).expect("Test operation should succeed");
//! assert_eq!(spec, parsed);
//! ```
//!
//! # Real-World Examples
//!
//! These patterns are found in production World of Warcraft CASC archives:
//!
//! - **Streaming**: `b:{256K*4=n,*=z:9}` - First 1MB uncompressed for instant playback
//! - **Patch files**: `b:{64K=n,64K*10=z:6,*=z:9}` - Header uncompressed, patches moderate, bulk high
//! - **Small files**: `z:9` - Simple maximum compression
//! - **Large assets**: `b:{22=n,31943=z,211_232=n,*=z}` - Mixed strategies for different sections
//! - **MPQ compat**: `b:{16K*=z:{6,mpq}}` - Backward compatibility with older tools

mod parser;
mod types;

pub use parser::Parser;
pub use types::{BlockChunk, BlockSizeSpec, ESpec, ZLibBits};

// Re-export the main parse function
pub use types::parse;

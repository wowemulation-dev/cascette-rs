//! Size manifest format (`DS` magic) for estimated file sizes
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::use_self)]
//!
//! The Size manifest maps encoding keys to estimated file sizes (eSize). It is
//! used when compressed size (cSize) is unavailable, enabling disk space
//! estimation and download progress reporting.
//!
//! # Format Overview
//!
//! - Magic: `DS` (0x44, 0x53)
//! - Versions: 1 (variable esize width) and 2 (fixed 4-byte esize)
//! - All multi-byte integers: big-endian
//! - V1 header: 19 bytes, V2 header: 15 bytes
//! - Entries: encoding key + 16-bit key hash + variable-width esize
//!
//! # Usage
//!
//! ```rust,no_run
//! use cascette_formats::size::{SizeManifest, SizeManifestBuilder};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Build a size manifest
//! let manifest = SizeManifestBuilder::new()
//!     .version(2)
//!     .key_size_bits(128)
//!     .add_entry(vec![0xAA; 16], 0x1234, 1024)
//!     .add_entry(vec![0xBB; 16], 0x5678, 2048)
//!     .build()?;
//!
//! // Serialize to bytes
//! let data = manifest.build()?;
//!
//! // Parse from bytes
//! let parsed = SizeManifest::parse(&data)?;
//! assert_eq!(parsed.entries.len(), 2);
//! # Ok(())
//! # }
//! ```

pub mod builder;
pub mod entry;
pub mod error;
pub mod header;
pub mod manifest;

// Re-export main types
pub use builder::SizeManifestBuilder;
pub use entry::SizeEntry;
pub use error::{Result, SizeError};
pub use header::{SizeHeader, SizeHeaderV1, SizeHeaderV2};
pub use manifest::SizeManifest;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_re_exports_accessible() {
        // Verify all public types are accessible through the module re-exports
        let _ = SizeManifestBuilder::new();
        let header = SizeHeader::new_v2(0, 0, 128, 0);
        assert_eq!(header.version(), 2);
        let entry = SizeEntry::new(vec![0x00; 16], 0x1234, 100);
        assert_eq!(entry.esize, 100);
    }

    #[test]
    fn test_basic_workflow() {
        // Build -> serialize -> parse round-trip
        let manifest = SizeManifestBuilder::new()
            .version(1)
            .key_size_bits(128)
            .add_entry(vec![0xAA; 16], 0x1111, 500)
            .add_entry(vec![0xBB; 16], 0x2222, 700)
            .build()
            .expect("Should build manifest");

        assert_eq!(manifest.header.version(), 1);
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.header.total_size(), 1200);

        let data = manifest.build().expect("Should serialize");
        let parsed = SizeManifest::parse(&data).expect("Should parse");

        assert_eq!(manifest, parsed);
    }
}

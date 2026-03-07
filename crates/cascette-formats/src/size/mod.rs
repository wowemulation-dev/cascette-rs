//! Size manifest format (`DS` magic)
//!
//! Source: <https://wowdev.wiki/TACT#Size_manifest>
//!
//! The Size manifest was introduced in build 27547. It maps partial encoding
//! keys to estimated file sizes. Files are sorted descending by `esize`.
//!
//! # Format
//!
//! - Magic: `DS` (0x44, 0x53)
//! - Version: 1 (only known version)
//! - Header: 15 bytes (see [`SizeHeader`][crate::size::SizeHeader])
//! - Tags: same structure as install/download manifest tags
//! - Entries: `ekey[ekey_size] + esize[4 BE u32]` per file
//!
//! # Usage
//!
//! ```rust,no_run
//! use cascette_formats::size::SizeManifest;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse from BLTE-decompressed bytes
//! let data: Vec<u8> = vec![]; // placeholder
//! let manifest = SizeManifest::parse(&data)?;
//! for entry in &manifest.entries {
//!     println!("{} -> {} bytes", hex::encode(&entry.key), entry.esize);
//! }
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
pub use header::SizeHeader;
pub use manifest::SizeManifest;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_re_exports_accessible() {
        let _ = SizeManifestBuilder::new();
        let header = SizeHeader::new(1, 9, 0, 0, 0);
        assert_eq!(header.version, 1);
        let entry = SizeEntry::new(vec![0x00; 9], 100);
        assert_eq!(entry.esize, 100);
    }

    #[test]
    fn test_basic_workflow() {
        let manifest = SizeManifestBuilder::new()
            .add_entry(vec![0xAAu8; 9], 500)
            .add_entry(vec![0xBBu8; 9], 700)
            .build()
            .expect("Should build manifest");

        assert_eq!(manifest.header.version, 1);
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.header.total_size, 1200);

        let data = manifest.build().expect("Should serialize");
        let parsed = SizeManifest::parse(&data).expect("Should parse");
        assert_eq!(manifest, parsed);
    }
}

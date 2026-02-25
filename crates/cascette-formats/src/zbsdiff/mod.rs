//! ZBSDIFF1 (Zlib-compressed Binary Differential) format implementation
//!
//! ZBSDIFF1 is a binary differential patch format used by NGDP/TACT for efficient
//! file updates. It's based on the bsdiff algorithm by Colin Percival, with zlib
//! compression applied to the control, diff, and extra data blocks.
//!
//! # Format Structure
//!
//! ```text
//! ZBSDIFF1 File:
//! ├── Header (32 bytes)
//! │   ├── Signature: "ZBSDIFF1" (8 bytes, little-endian)
//! │   ├── Control Block Size (8 bytes, little-endian)
//! │   ├── Diff Block Size (8 bytes, little-endian)
//! │   └── Output File Size (8 bytes, little-endian)
//! ├── Control Block (zlib-compressed)
//! │   └── Triple sequences: (diff_size, extra_size, seek_offset)
//! ├── Diff Block (zlib-compressed)
//! │   └── Byte differences to apply to old data
//! └── Extra Block (zlib-compressed)
//!     └── New data to insert
//! ```
//!
//! # Key Characteristics
//!
//! - **Little-Endian Header**: All header fields use little-endian byte order
//! - **Zlib Compression**: All data blocks are zlib-compressed
//! - **Three-Block Structure**: Control, diff, and extra data blocks
//! - **Signed Integers**: Uses signed 64-bit integers for sizes and offsets
//! - **Streaming Application**: Can be applied without loading entire files
//!
//! # Usage Examples
//!
//! ## Simple Patch Application
//!
//! ```rust
//! use cascette_formats::zbsdiff;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Load files
//! let old_data = std::fs::read("old_file.bin")?;
//! let patch_data = std::fs::read("file.zbsdiff")?;
//!
//! // Apply patch
//! let new_data = zbsdiff::apply_patch_memory(&old_data, &patch_data)?;
//!
//! // Save result
//! std::fs::write("new_file.bin", &new_data)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Creating Patches
//!
//! ```rust
//! use cascette_formats::zbsdiff::ZbsdiffBuilder;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let old_data = b"Hello, World!".to_vec();
//! let new_data = b"Hello, Rust!".to_vec();
//!
//! // Create patch using bsdiff algorithm
//! let builder = ZbsdiffBuilder::new(old_data, new_data);
//! let patch = builder.build()?;
//!
//! // Apply patch to verify
//! let result = cascette_formats::zbsdiff::apply_patch_memory(b"Hello, World!", &patch)?;
//! assert_eq!(result, b"Hello, Rust!");
//! # Ok(())
//! # }
//! ```
//!
//! # Implementation Status
//!
//! - ✅ Header parsing with binrw
//! - ✅ Control block decompression and parsing
//! - ✅ Zlib compression/decompression
//! - ✅ Memory-based patch application
//! - ✅ Streaming patch application for large files
//! - ✅ Suffix array-based patch creation (bsdiff algorithm)
//! - ✅ Basic patch creation (simple and chunked, for testing)
//! - ✅ Error handling
//! - ✅ Round-trip validation

mod builder;
mod error;
mod header;
mod patcher;
mod suffix;
mod utils;

// Re-export public API
pub use builder::ZbsdiffBuilder;
pub use error::{ZbsdiffError, ZbsdiffResult};
pub use header::{ZBSDIFF1_SIGNATURE, ZbsdiffHeader};
pub use patcher::{ZbsdiffPatcher, apply_patch_memory};
pub use utils::{ControlBlock, ControlEntry, compress_zlib, decompress_zlib};

/// Main ZBSDIFF1 patch structure
#[derive(Debug, Clone)]
pub struct ZbsDiff {
    /// 32-byte header with format signature and block sizes
    pub header: ZbsdiffHeader,
    /// Compressed control block containing patch instructions
    pub control_data: Vec<u8>,
    /// Compressed diff data for byte-level differences
    pub diff_data: Vec<u8>,
    /// Compressed extra data for new content insertion
    pub extra_data: Vec<u8>,
}

impl ZbsDiff {
    /// Parse a complete ZBSDIFF1 patch from bytes
    pub fn parse(data: &[u8]) -> Result<Self, ZbsdiffError> {
        use binrw::BinRead;
        use std::io::{Cursor, Read};

        let mut cursor = Cursor::new(data);

        // Parse header
        let header = ZbsdiffHeader::read_options(&mut cursor, binrw::Endian::Little, ())?;
        header.validate()?;

        // Read compressed blocks based on header sizes
        let mut control_data = vec![0u8; header.control_size as usize];
        cursor.read_exact(&mut control_data)?;

        let mut diff_data = vec![0u8; header.diff_size as usize];
        cursor.read_exact(&mut diff_data)?;

        let mut extra_data = Vec::new();
        cursor.read_to_end(&mut extra_data)?;

        Ok(ZbsDiff {
            header,
            control_data,
            diff_data,
            extra_data,
        })
    }

    /// Build a ZBSDIFF1 patch to bytes
    pub fn build(&self) -> Result<Vec<u8>, ZbsdiffError> {
        use binrw::BinWrite;
        use std::io::{Cursor, Write};

        let mut patch = Vec::new();
        let mut cursor = Cursor::new(&mut patch);

        // Write header
        self.header
            .write_options(&mut cursor, binrw::Endian::Little, ())?;

        // Write compressed blocks
        cursor.write_all(&self.control_data)?;
        cursor.write_all(&self.diff_data)?;
        cursor.write_all(&self.extra_data)?;

        Ok(patch)
    }

    /// Apply this patch to old data, producing new data
    pub fn apply(&self, old_data: &[u8]) -> Result<Vec<u8>, ZbsdiffError> {
        let patch_data = self.build()?;
        apply_patch_memory(old_data, &patch_data)
    }

    /// Get the expected output size after applying this patch
    pub fn output_size(&self) -> usize {
        self.header.output_size as usize
    }

    /// Decompress and parse the control block
    pub fn control_block(&self) -> Result<ControlBlock, ZbsdiffError> {
        ControlBlock::from_compressed(&self.control_data)
    }

    /// Decompress the diff data
    pub fn diff_data(&self) -> Result<Vec<u8>, ZbsdiffError> {
        decompress_zlib(&self.diff_data)
    }

    /// Decompress the extra data
    pub fn extra_data(&self) -> Result<Vec<u8>, ZbsdiffError> {
        decompress_zlib(&self.extra_data)
    }
}

impl crate::CascFormat for ZbsDiff {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::parse(data)?)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(self.build()?)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::CascFormat;

    #[test]
    fn test_zbsdiff_parse_build_round_trip() {
        let old_data = b"Hello, World!";
        let new_data = b"Hello, Rust!";

        // Create a simple patch
        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Simple patch build should succeed");

        // Parse the patch
        let patch = ZbsDiff::parse(&patch_data).expect("Patch parsing should succeed");

        // Build it back
        let rebuilt = patch.build().expect("Patch build should succeed");

        // Verify round-trip
        assert_eq!(patch_data, rebuilt);

        // Verify application works
        let result = patch
            .apply(old_data)
            .expect("Patch application should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_zbsdiff_casc_format_trait() {
        let old_data = b"Test data";
        let new_data = b"Best data";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Simple patch build should succeed");

        // Test CascFormat trait implementation
        assert!(ZbsDiff::verify_round_trip(&patch_data).is_ok());
    }
}

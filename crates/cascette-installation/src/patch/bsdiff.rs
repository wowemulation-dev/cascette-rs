//! BsDiff full-file patching
//!
//! Wraps `zbsdiff` with BLTE decoding for CDN-sourced patch data.
//! The patch data from CDN is BLTE-encoded; this module decodes it
//! and applies the underlying ZBSDIFF1 patch.

use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;
use cascette_formats::zbsdiff;

use super::error::PatchError;

/// Apply a BsDiff patch to base content
///
/// # Arguments
///
/// * `base_data` - Decoded base file content (raw bytes)
/// * `patch_blte_data` - BLTE-encoded ZBSDIFF1 patch data from CDN
///
/// # Returns
///
/// The patched output bytes.
pub fn apply_bsdiff_patch(base_data: &[u8], patch_blte_data: &[u8]) -> Result<Vec<u8>, PatchError> {
    // Decode BLTE container to get raw ZBSDIFF1 data
    let blte = BlteFile::parse(patch_blte_data)?;
    let raw_patch = blte.decompress()?;

    // Apply the ZBSDIFF1 patch
    let result = zbsdiff::apply_patch_memory(base_data, &raw_patch)?;

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_formats::blte::{BlteBuilder, CompressionMode};
    use cascette_formats::zbsdiff::ZbsdiffBuilder;

    #[test]
    fn test_apply_bsdiff_patch() {
        let old_data = b"Hello, World! This is test data.";
        let new_data = b"Hello, Rust! This is patched data.";

        // Create a ZBSDIFF1 patch
        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let raw_patch = builder
            .build_simple_patch()
            .expect("patch build should succeed");

        // Wrap in BLTE (no compression for simplicity)
        let blte = BlteBuilder::new()
            .add_data(&raw_patch)
            .expect("add data should succeed")
            .build()
            .expect("BLTE build should succeed");
        let blte_data: Vec<u8> =
            CascFormat::build(&blte).expect("BLTE serialization should succeed");

        // Apply
        let result = apply_bsdiff_patch(old_data, &blte_data).expect("patch apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_apply_bsdiff_patch_compressed() {
        let old_data = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let new_data = b"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let raw_patch = builder
            .build_simple_patch()
            .expect("patch build should succeed");

        // Wrap in BLTE with zlib compression
        let blte = BlteFile::compress(&raw_patch, raw_patch.len(), CompressionMode::ZLib)
            .expect("compress should succeed");
        let blte_data: Vec<u8> =
            CascFormat::build(&blte).expect("BLTE serialization should succeed");

        let result = apply_bsdiff_patch(old_data, &blte_data).expect("patch apply should succeed");
        assert_eq!(result, new_data);
    }
}

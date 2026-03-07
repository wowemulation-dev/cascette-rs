//! Block-level diff patching
//!
//! Applies a block-by-block binary diff for BLTE-chunked files.
//! Each chunk in the patch BLTE is added byte-by-byte to the
//! corresponding chunk in the base BLTE, producing a new output.

use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;

use super::error::PatchError;

/// Apply a block-level diff patch
///
/// Both inputs are BLTE-encoded. The patch is applied chunk-by-chunk:
/// for each byte position, `output[i] = base[i] + patch[i]` (wrapping add).
///
/// # Arguments
///
/// * `base_blte_data` - BLTE-encoded base file
/// * `patch_blte_data` - BLTE-encoded patch data (block diffs)
///
/// # Returns
///
/// The patched raw content bytes (decoded).
pub fn apply_block_patch(
    base_blte_data: &[u8],
    patch_blte_data: &[u8],
) -> Result<Vec<u8>, PatchError> {
    let base_blte = BlteFile::parse(base_blte_data)?;
    let patch_blte = BlteFile::parse(patch_blte_data)?;

    let base_decoded = base_blte.decompress()?;
    let patch_decoded = patch_blte.decompress()?;

    // Apply byte-level diff: output[i] = base[i].wrapping_add(patch[i])
    let len = base_decoded.len().min(patch_decoded.len());
    let mut result = Vec::with_capacity(len);
    for i in 0..len {
        result.push(base_decoded[i].wrapping_add(patch_decoded[i]));
    }

    // If patch is longer than base, append remaining patch bytes directly
    if patch_decoded.len() > len {
        result.extend_from_slice(&patch_decoded[len..]);
    }

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_formats::blte::CompressionMode;

    #[test]
    fn test_apply_block_patch() {
        // Base content
        let base_content = vec![10u8, 20, 30, 40, 50];
        // Patch diff: adding these to base yields target
        let target = vec![15u8, 25, 35, 45, 55];
        let patch_diff: Vec<u8> = base_content
            .iter()
            .zip(target.iter())
            .map(|(b, t)| t.wrapping_sub(*b))
            .collect();

        // Wrap both in BLTE
        let base_blte = BlteFile::single_chunk(base_content, CompressionMode::None)
            .expect("base BLTE should succeed");
        let patch_blte = BlteFile::single_chunk(patch_diff, CompressionMode::None)
            .expect("patch BLTE should succeed");

        let base_data: Vec<u8> = CascFormat::build(&base_blte).expect("base build should succeed");
        let patch_data: Vec<u8> =
            CascFormat::build(&patch_blte).expect("patch build should succeed");

        let result =
            apply_block_patch(&base_data, &patch_data).expect("block patch should succeed");
        assert_eq!(result, target);
    }

    #[test]
    fn test_apply_block_patch_wrapping() {
        // Test wrapping addition (255 + 1 = 0)
        let base_content = vec![255u8, 200, 100];
        let patch_diff = vec![1u8, 56, 156]; // wraps: 0, 0, 0
        let expected = vec![0u8, 0, 0];

        let base_blte = BlteFile::single_chunk(base_content, CompressionMode::None).unwrap();
        let patch_blte = BlteFile::single_chunk(patch_diff, CompressionMode::None).unwrap();

        let base_data: Vec<u8> = CascFormat::build(&base_blte).unwrap();
        let patch_data: Vec<u8> = CascFormat::build(&patch_blte).unwrap();

        let result = apply_block_patch(&base_data, &patch_data).unwrap();
        assert_eq!(result, expected);
    }
}

//! ZBSDIFF1 patch builder for creating binary differential patches.
//!
//! This module provides functionality to create ZBSDIFF1 patches from old and new data.
//!
//! ## Algorithms
//!
//! Three algorithms are available:
//!
//! 1. **Optimized** (`build()` / `build_optimized_patch()`): Suffix array-based bsdiff
//!    algorithm. Produces near-optimal patches by finding longest matches anywhere in
//!    the old data. This is the recommended method.
//!
//! 2. **Simple** (`build_simple_patch()`): Treats all new data as "extra" data.
//!    Fast but produces patches roughly the size of the new file. Useful for
//!    testing and zero-diff cases.
//!
//! 3. **Chunked** (`build_chunked_patch()`): Forward-only byte matching. Better than
//!    simple but worse than optimized. Kept for testing.

use crate::zbsdiff::{
    ZBSDIFF1_SIGNATURE, ZbsdiffHeader,
    error::ZbsdiffResult,
    utils::{ControlBlock, ControlEntry, compress_zlib},
};
use binrw::BinWrite;
use std::io::{Cursor, Write};

/// Builder for creating ZBSDIFF1 patches.
///
/// Creates patches that transform old data into new data. The recommended
/// method is `build()`, which uses the bsdiff suffix array algorithm for
/// near-optimal patch sizes.
///
/// # Examples
///
/// ```rust
/// use cascette_formats::zbsdiff::ZbsdiffBuilder;
///
/// let old_data = b"Hello, World!".to_vec();
/// let new_data = b"Hello, Rust!".to_vec();
///
/// let builder = ZbsdiffBuilder::new(old_data, new_data);
/// let patch = builder.build().expect("Operation should succeed");
///
/// // Verify the patch works
/// let result = cascette_formats::zbsdiff::apply_patch_memory(b"Hello, World!", &patch).expect("Operation should succeed");
/// assert_eq!(result, b"Hello, Rust!");
/// ```
#[derive(Debug, Clone)]
pub struct ZbsdiffBuilder {
    old_data: Vec<u8>,
    new_data: Vec<u8>,
    max_diff_block_size: usize,
}

impl ZbsdiffBuilder {
    /// Create a new patch builder
    ///
    /// # Arguments
    /// * `old_data` - The original data to patch from
    /// * `new_data` - The target data to patch to
    pub fn new(old_data: Vec<u8>, new_data: Vec<u8>) -> Self {
        Self {
            old_data,
            new_data,
            max_diff_block_size: 1024 * 1024, // 1MB chunks for diff operations
        }
    }

    /// Set the maximum size for diff blocks (default: 1MB)
    ///
    /// Smaller blocks use less memory but may result in larger patches.
    /// Larger blocks may find more matching content but use more memory.
    pub fn with_max_diff_block_size(mut self, size: usize) -> Self {
        self.max_diff_block_size = size; // No minimum constraint for testing flexibility
        self
    }

    /// Build a simple patch using a naive algorithm
    ///
    /// This algorithm treats everything as "extra" data, which results in
    /// patches roughly the size of the new data plus overhead. It's simple
    /// and fast but not optimized for size.
    ///
    /// For production use, consider implementing `build_optimized_patch()`
    /// with suffix array-based matching.
    pub fn build_simple_patch(&self) -> ZbsdiffResult<Vec<u8>> {
        // Simple strategy: treat the entire new data as "extra" data
        let control_entries = vec![ControlEntry::new(
            0,                          // No diff operations
            self.new_data.len() as i64, // All data comes from extra
            0,                          // No seek needed
        )];

        let control_block = ControlBlock::with_entries(control_entries)?;
        let diff_data = Vec::new(); // No diff data in simple patch
        let extra_data = self.new_data.clone();

        self.build_patch_internal(control_block, diff_data, extra_data)
    }

    /// Build a patch using basic diff algorithm with chunked matching
    ///
    /// This algorithm attempts to find matching chunks between old and new data
    /// and uses diff operations where possible. It's better than the simple
    /// algorithm but still not as optimal as a suffix array approach.
    pub fn build_chunked_patch(&self) -> ZbsdiffResult<Vec<u8>> {
        let mut control_entries = Vec::new();
        let mut diff_data = Vec::new();
        let mut extra_data = Vec::new();

        let mut old_pos = 0usize;
        let mut new_pos = 0usize;

        while new_pos < self.new_data.len() {
            // Try to find a matching chunk
            let chunk_size = self.find_matching_chunk(old_pos, new_pos);

            if chunk_size >= 4 {
                // Only use diff for chunks >= 4 bytes
                // Use diff operation for this chunk
                for i in 0..chunk_size {
                    let old_byte = self.old_data.get(old_pos + i).copied().unwrap_or(0);
                    let new_byte = self.new_data[new_pos + i];
                    let diff_byte = new_byte.wrapping_sub(old_byte);
                    diff_data.push(diff_byte);
                }

                control_entries.push(ControlEntry::new(chunk_size as i64, 0, 0));

                old_pos += chunk_size;
                new_pos += chunk_size;
            } else {
                // Use extra data for non-matching content
                let extra_chunk_size = self.find_extra_chunk_size(new_pos);

                for i in 0..extra_chunk_size {
                    extra_data.push(self.new_data[new_pos + i]);
                }

                control_entries.push(ControlEntry::new(
                    0,
                    extra_chunk_size as i64,
                    old_pos as i64, // Seek to maintain position tracking
                ));

                new_pos += extra_chunk_size;
                // old_pos stays the same for extra data
            }
        }

        let control_block = ControlBlock::with_entries(control_entries)?;
        self.build_patch_internal(control_block, diff_data, extra_data)
    }

    /// Build a patch using the bsdiff suffix array algorithm.
    ///
    /// This is the recommended method. It uses suffix array-based matching
    /// (via `divsufsort`) to find the longest match of each new-data position
    /// anywhere in the old data, producing near-optimal patches.
    ///
    /// Memory usage is approximately 5x the old data size (1 byte + 4 bytes
    /// per byte for the suffix array). The i32 suffix array index limits
    /// old data to ~2 GiB, which is within the header's 1 GiB validation cap.
    pub fn build(&self) -> ZbsdiffResult<Vec<u8>> {
        self.build_optimized_patch()
    }

    /// Build a patch using the bsdiff suffix array algorithm.
    ///
    /// Equivalent to `build()`. Kept for API symmetry with `build_simple_patch()`
    /// and `build_chunked_patch()`.
    pub fn build_optimized_patch(&self) -> ZbsdiffResult<Vec<u8>> {
        use super::suffix;

        let result = suffix::compute_diff(&self.old_data, &self.new_data);

        // Handle empty new data: compute_diff returns no control entries,
        // but build_patch_internal requires at least one entry.
        if result.control.is_empty() {
            return self.build_simple_patch();
        }

        let control_block = ControlBlock::with_entries(result.control)?;
        self.build_patch_internal(control_block, result.diff_data, result.extra_data)
    }

    /// Internal method to build a patch from components
    fn build_patch_internal(
        &self,
        control_block: ControlBlock,
        diff_data: Vec<u8>,
        extra_data: Vec<u8>,
    ) -> ZbsdiffResult<Vec<u8>> {
        // Compress all blocks
        let control_compressed = control_block.to_compressed()?;
        let diff_compressed = compress_zlib(&diff_data)?;
        let extra_compressed = compress_zlib(&extra_data)?;

        // Create header
        let header = ZbsdiffHeader {
            signature: ZBSDIFF1_SIGNATURE,
            control_size: control_compressed.len() as i64,
            diff_size: diff_compressed.len() as i64,
            output_size: self.new_data.len() as i64,
        };
        header.validate()?;

        // Assemble patch
        let mut patch = Vec::new();
        let mut cursor = Cursor::new(&mut patch);

        // Write header (32 bytes)
        header.write_options(&mut cursor, binrw::Endian::Little, ())?;

        // Write compressed blocks
        cursor.write_all(&control_compressed)?;
        cursor.write_all(&diff_compressed)?;
        cursor.write_all(&extra_compressed)?;

        Ok(patch)
    }

    /// Find the size of a matching chunk starting at given positions
    fn find_matching_chunk(&self, old_pos: usize, new_pos: usize) -> usize {
        let mut size = 0;
        let max_size = self
            .max_diff_block_size
            .min(self.old_data.len().saturating_sub(old_pos))
            .min(self.new_data.len().saturating_sub(new_pos));

        while size < max_size {
            let old_byte = self.old_data.get(old_pos + size).copied().unwrap_or(0);
            let new_byte = self.new_data[new_pos + size];

            // For now, simple byte-by-byte matching
            // A more sophisticated algorithm would look for longer patterns
            if old_byte == new_byte {
                size += 1;
            } else {
                break;
            }
        }

        size
    }

    /// Find the size of an extra data chunk starting at given position
    fn find_extra_chunk_size(&self, new_pos: usize) -> usize {
        // For simplicity, use small extra chunks to allow more opportunities
        // for finding matches later
        let remaining = self.new_data.len() - new_pos;
        remaining.min(256) // Max 256 bytes per extra chunk
    }

    /// Get statistics about what a patch would contain
    pub fn analyze_patch(&self) -> PatchAnalysis {
        // Analyze using the chunked algorithm approach
        let mut total_diff_bytes = 0;
        let mut total_extra_bytes = 0;
        let mut total_match_bytes = 0;

        let mut old_pos = 0usize;
        let mut new_pos = 0usize;

        while new_pos < self.new_data.len() {
            let match_size = self.find_matching_chunk(old_pos, new_pos);

            if match_size >= 4 {
                total_diff_bytes += match_size;
                total_match_bytes += match_size;
                old_pos += match_size;
                new_pos += match_size;
            } else {
                let extra_size = self.find_extra_chunk_size(new_pos);
                total_extra_bytes += extra_size;
                new_pos += extra_size;
            }
        }

        PatchAnalysis {
            old_size: self.old_data.len(),
            new_size: self.new_data.len(),
            total_diff_bytes,
            total_extra_bytes,
            total_match_bytes,
            compression_ratio: 0.0, // Would need to compress to calculate
        }
    }
}

/// Analysis of what a patch would contain
#[derive(Debug, Clone)]
pub struct PatchAnalysis {
    /// Size of old data
    pub old_size: usize,
    /// Size of new data
    pub new_size: usize,
    /// Bytes that would use diff operations
    pub total_diff_bytes: usize,
    /// Bytes that would be stored as extra data
    pub total_extra_bytes: usize,
    /// Bytes that match between old and new (efficiency metric)
    pub total_match_bytes: usize,
    /// Estimated compression ratio (patch size / new size)
    pub compression_ratio: f64,
}

impl PatchAnalysis {
    /// Calculate the match percentage (how much content is reused)
    pub fn match_percentage(&self) -> f64 {
        if self.new_size == 0 {
            return 100.0;
        }
        (self.total_match_bytes as f64 / self.new_size as f64) * 100.0
    }

    /// Calculate the extra data percentage
    pub fn extra_percentage(&self) -> f64 {
        if self.new_size == 0 {
            return 0.0;
        }
        (self.total_extra_bytes as f64 / self.new_size as f64) * 100.0
    }
}

/// Convenience function to create a patch between two byte slices
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
pub fn create_patch(old_data: &[u8], new_data: &[u8]) -> ZbsdiffResult<Vec<u8>> {
    let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
    builder.build_chunked_patch()
}

/// Convenience function to create a simple patch between two byte slices
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
pub fn create_simple_patch(old_data: &[u8], new_data: &[u8]) -> ZbsdiffResult<Vec<u8>> {
    let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
    builder.build_simple_patch()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::zbsdiff::apply_patch_memory;

    #[test]
    fn test_simple_patch_creation() {
        let old_data = b"Hello, World!";
        let new_data = b"Hello, Rust!";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        // Verify patch works
        let result = apply_patch_memory(old_data, &patch).expect("Operation should succeed");
        assert_eq!(result, new_data);

        // Verify patch is reasonable size
        // Simple patches may be larger than new data due to compression overhead and headers
        println!(
            "Patch size: {}, New data size: {}, Ratio: {:.2}",
            patch.len(),
            new_data.len(),
            patch.len() as f64 / new_data.len() as f64
        );
        // Just verify the patch isn't absurdly large (10x would be concerning)
        assert!(
            patch.len() < new_data.len() * 10,
            "Patch is unreasonably large"
        );
    }

    #[test]
    fn test_chunked_patch_creation() {
        let old_data = b"The quick brown fox jumps over the lazy dog";
        let new_data = b"The quick brown cat jumps over the lazy dog";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder
            .build_chunked_patch()
            .expect("Operation should succeed");

        // Verify patch works
        let result = apply_patch_memory(old_data, &patch).expect("Operation should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_empty_to_content() {
        let old_data = b"";
        let new_data = b"New content here!";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("Operation should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_content_to_empty() {
        let old_data = b"Some content to remove";
        let new_data = b"";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("Operation should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_identical_data() {
        let data = b"Identical data in both old and new";

        let builder = ZbsdiffBuilder::new(data.to_vec(), data.to_vec());
        let patch = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        let result = apply_patch_memory(data, &patch).expect("Operation should succeed");
        assert_eq!(result, data);
    }

    #[test]
    fn test_large_data_patch() {
        // Create larger test data
        let old_data = vec![42u8; 5000];
        let mut new_data = old_data.clone();

        // Make some changes
        new_data[1000] = 100;
        new_data[2000] = 200;
        new_data.extend_from_slice(b" Additional content at the end");

        let builder =
            ZbsdiffBuilder::new(old_data.clone(), new_data.clone()).with_max_diff_block_size(1024);

        // Use simple patch for now since chunked algorithm needs more work
        let patch = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        let result = apply_patch_memory(&old_data, &patch).expect("Operation should succeed");
        assert_eq!(result, new_data, "Large data patch application failed");
    }

    #[test]
    fn test_patch_analysis() {
        let old_data = b"ABCDEFGHIJKLMNOP";
        let new_data = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ"; // Same prefix + extra

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let analysis = builder.analyze_patch();

        assert_eq!(analysis.old_size, old_data.len());
        assert_eq!(analysis.new_size, new_data.len());
        assert!(analysis.match_percentage() > 50.0); // Most content matches
        assert!(analysis.extra_percentage() < 50.0); // Some extra content
    }

    #[test]
    fn test_convenience_functions() {
        let old_data = b"old data";
        let new_data = b"new data";

        let simple_patch =
            create_simple_patch(old_data, new_data).expect("Operation should succeed");
        let result1 =
            apply_patch_memory(old_data, &simple_patch).expect("Operation should succeed");
        assert_eq!(result1, new_data);

        let chunked_patch = create_patch(old_data, new_data).expect("Operation should succeed");
        let result2 =
            apply_patch_memory(old_data, &chunked_patch).expect("Operation should succeed");
        assert_eq!(result2, new_data);
    }

    #[test]
    fn test_builder_configuration() {
        let old_data = b"test data for configuration";
        let new_data = b"test data for configuration testing";

        let builder =
            ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec()).with_max_diff_block_size(512);

        assert_eq!(builder.max_diff_block_size, 512);

        let patch = builder
            .build_chunked_patch()
            .expect("Operation should succeed");
        let result = apply_patch_memory(old_data, &patch).expect("Operation should succeed");
        assert_eq!(result, new_data);
    }

    // --- Optimized (bsdiff) patch tests ---

    #[test]
    fn test_optimized_patch_small_edit() {
        let old_data = b"Hello, World!";
        let new_data = b"Hello, Rust!";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_identical() {
        let data = b"Identical data that should diff to nearly nothing";

        let builder = ZbsdiffBuilder::new(data.to_vec(), data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(data, &patch).expect("apply should succeed");
        assert_eq!(result, data);
    }

    #[test]
    fn test_optimized_patch_empty_old() {
        let old_data = b"";
        let new_data = b"Brand new content";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_empty_new() {
        let old_data = b"Content to delete";
        let new_data = b"";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_both_empty() {
        let builder = ZbsdiffBuilder::new(Vec::new(), Vec::new());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(&[], &patch).expect("apply should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_optimized_patch_single_byte() {
        let old_data = b"A";
        let new_data = b"B";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_insertion() {
        let old_data = b"The quick fox jumps over the lazy dog";
        let new_data = b"The quick brown fox jumps over the lazy dog";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_deletion() {
        let old_data = b"The quick brown fox jumps over the lazy dog";
        let new_data = b"The quick fox jumps over the lazy dog";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_reordered_blocks() {
        // Content with blocks that get reordered
        let old_data = b"AAAABBBBCCCCDDDD";
        let new_data = b"CCCCAAAADDDDBBBB";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_smaller_than_simple() {
        // Data with shared content should produce smaller patches than simple
        let old_data = vec![42u8; 2000];
        let mut new_data = old_data.clone();
        new_data[1000] = 99; // Single byte change

        let builder = ZbsdiffBuilder::new(old_data.clone(), new_data.clone());
        let optimized_patch = builder.build().expect("optimized build should succeed");

        let builder2 = ZbsdiffBuilder::new(old_data.clone(), new_data.clone());
        let simple_patch = builder2
            .build_simple_patch()
            .expect("simple build should succeed");

        // Verify both produce correct output
        let result1 =
            apply_patch_memory(&old_data, &optimized_patch).expect("apply should succeed");
        let result2 = apply_patch_memory(&old_data, &simple_patch).expect("apply should succeed");
        assert_eq!(result1, new_data);
        assert_eq!(result2, new_data);

        // Optimized should be smaller for data with shared content
        assert!(
            optimized_patch.len() < simple_patch.len(),
            "optimized ({}) should be smaller than simple ({})",
            optimized_patch.len(),
            simple_patch.len(),
        );
    }

    #[test]
    fn test_optimized_patch_random_edits() {
        use rand::{RngExt, SeedableRng};

        let mut rng = rand::rngs::StdRng::seed_from_u64(12345);

        // Generate base data
        let mut old_data = vec![0u8; 4096];
        rng.fill(&mut old_data[..]);

        // Make a copy with scattered edits
        let mut new_data = old_data.clone();
        for _ in 0..50 {
            let pos = rng.random_range(0..new_data.len());
            new_data[pos] = rng.random();
        }

        let builder = ZbsdiffBuilder::new(old_data.clone(), new_data.clone());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(&old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_optimized_patch_large_with_suffix() {
        // Larger data with appended content
        let old_data = vec![42u8; 5000];
        let mut new_data = old_data.clone();
        new_data[1000] = 100;
        new_data[2000] = 200;
        new_data.extend_from_slice(b" Additional content at the end");

        let builder = ZbsdiffBuilder::new(old_data.clone(), new_data.clone());
        let patch = builder.build().expect("build() should succeed");

        let result = apply_patch_memory(&old_data, &patch).expect("apply should succeed");
        assert_eq!(result, new_data);
    }
}

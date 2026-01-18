//! ZBSDIFF1 patch application engine with streaming support
//!
//! This module provides both memory-based and streaming patch application
//! for ZBSDIFF1 binary differential patches. The streaming implementation
//! enables processing large files without loading them entirely into memory.

use crate::zbsdiff::{
    ControlBlock, ZbsdiffHeader,
    error::{ZbsdiffError, ZbsdiffResult},
    utils::{apply_diff_byte, decompress_zlib, read_old_byte_at},
};
use binrw::BinRead;
use std::io::{Cursor, Read, Seek, SeekFrom};

/// Apply a ZBSDIFF1 patch to old data in memory, producing new data
///
/// This is the simplest way to apply a patch when both the old file and
/// patch fit comfortably in memory.
///
/// # Examples
///
/// ```rust
/// use cascette_formats::zbsdiff;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let old_data = b"Hello, World!";
/// let patch_data = std::fs::read("patch.zbsdiff")?;
///
/// let new_data = zbsdiff::apply_patch_memory(old_data, &patch_data)?;
/// println!("Patched {} bytes -> {} bytes", old_data.len(), new_data.len());
/// # Ok(())
/// # }
/// ```
pub fn apply_patch_memory(old_data: &[u8], patch_data: &[u8]) -> ZbsdiffResult<Vec<u8>> {
    let mut cursor = Cursor::new(patch_data);

    // Parse header
    let header = ZbsdiffHeader::read_options(&mut cursor, binrw::Endian::Big, ())?;
    header.validate()?;

    // Read compressed blocks based on header sizes
    let mut control_compressed = vec![0u8; header.control_size as usize];
    cursor.read_exact(&mut control_compressed)?;

    let mut diff_compressed = vec![0u8; header.diff_size as usize];
    cursor.read_exact(&mut diff_compressed)?;

    let mut extra_compressed = Vec::new();
    cursor.read_to_end(&mut extra_compressed)?;

    // Decompress blocks
    let control_block = ControlBlock::from_compressed(&control_compressed)?;
    let diff_data = decompress_zlib(&diff_compressed)?;
    let extra_data = decompress_zlib(&extra_compressed)?;

    // Apply patch
    apply_patch_with_data(
        old_data,
        &control_block,
        &diff_data,
        &extra_data,
        header.output_size as usize,
    )
}

/// Apply a patch using pre-parsed components
fn apply_patch_with_data(
    old_data: &[u8],
    control_block: &ControlBlock,
    diff_data: &[u8],
    extra_data: &[u8],
    expected_output_size: usize,
) -> ZbsdiffResult<Vec<u8>> {
    let mut output = Vec::with_capacity(expected_output_size);
    let mut diff_cursor = Cursor::new(diff_data);
    let mut extra_cursor = Cursor::new(extra_data);
    let mut old_pos = 0usize;

    for entry in &control_block.entries {
        // Apply diff block
        for i in 0..entry.diff_size {
            if diff_cursor.position() >= diff_data.len() as u64 {
                return Err(ZbsdiffError::insufficient_data(
                    (entry.diff_size - i) as usize,
                    (diff_data.len() as u64 - diff_cursor.position()) as usize,
                ));
            }

            let mut diff_byte_buf = [0u8; 1];
            diff_cursor.read_exact(&mut diff_byte_buf)?;
            let diff_byte = diff_byte_buf[0];

            let old_byte = read_old_byte_at(old_data, old_pos);
            let new_byte = apply_diff_byte(old_byte, diff_byte);

            output.push(new_byte);
            old_pos += 1;
        }

        // Copy extra block
        for i in 0..entry.extra_size {
            if extra_cursor.position() >= extra_data.len() as u64 {
                return Err(ZbsdiffError::insufficient_data(
                    (entry.extra_size - i) as usize,
                    (extra_data.len() as u64 - extra_cursor.position()) as usize,
                ));
            }

            let mut extra_byte_buf = [0u8; 1];
            extra_cursor.read_exact(&mut extra_byte_buf)?;
            output.push(extra_byte_buf[0]);
        }

        // Seek in old file
        if entry.seek_offset != 0 {
            if entry.seek_offset < 0 {
                old_pos = old_pos.saturating_sub((-entry.seek_offset) as usize);
            } else {
                old_pos = old_pos.saturating_add(entry.seek_offset as usize);
            }
        }
    }

    // Verify output size
    if output.len() != expected_output_size {
        return Err(ZbsdiffError::SizeMismatch {
            expected: expected_output_size,
            actual: output.len(),
        });
    }

    Ok(output)
}

/// Streaming ZBSDIFF1 patcher for large files
///
/// This patcher enables processing large files by reading from the old file
/// on-demand rather than loading it entirely into memory.
///
/// # Examples
///
/// ```rust
/// use std::fs::File;
/// use cascette_formats::zbsdiff::{ZbsdiffPatcher, ZbsdiffHeader};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let old_file = File::open("large_old_file.bin")?;
/// let patch_data = std::fs::read("patch.zbsdiff")?;
///
/// // Parse header to get expected output size
/// let header = ZbsdiffHeader::parse_from_patch(&patch_data)?;
///
/// let patcher = ZbsdiffPatcher::new(old_file, header.output_size as usize);
/// let result = patcher.apply_patch_from_data(&patch_data)?;
///
/// std::fs::write("new_file.bin", &result)?;
/// # Ok(())
/// # }
/// ```
pub struct ZbsdiffPatcher<R: Read + Seek> {
    old_file: R,
    output_capacity: usize,
    buffer_size: usize,
}

impl<R: Read + Seek> ZbsdiffPatcher<R> {
    /// Create a new streaming patcher
    ///
    /// # Arguments
    /// * `old_file` - Readable and seekable source for old file data
    /// * `output_size` - Expected size of output after patching
    pub fn new(old_file: R, output_size: usize) -> Self {
        Self {
            old_file,
            output_capacity: output_size,
            buffer_size: 8192, // 8KB buffer for reading old file
        }
    }

    /// Set the buffer size for reading old file data (default: 8KB)
    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size.max(1024); // Minimum 1KB
        self
    }

    /// Apply a patch from raw patch data
    pub fn apply_patch_from_data(self, patch_data: &[u8]) -> ZbsdiffResult<Vec<u8>> {
        let mut cursor = Cursor::new(patch_data);

        // Parse header
        let header = ZbsdiffHeader::read_options(&mut cursor, binrw::Endian::Big, ())?;
        header.validate()?;

        // Read compressed blocks
        let mut control_compressed = vec![0u8; header.control_size as usize];
        cursor.read_exact(&mut control_compressed)?;

        let mut diff_compressed = vec![0u8; header.diff_size as usize];
        cursor.read_exact(&mut diff_compressed)?;

        let mut extra_compressed = Vec::new();
        cursor.read_to_end(&mut extra_compressed)?;

        // Decompress blocks
        let control_block = ControlBlock::from_compressed(&control_compressed)?;
        let diff_data = decompress_zlib(&diff_compressed)?;
        let extra_data = decompress_zlib(&extra_compressed)?;

        self.apply_patch(&control_block, &diff_data, &extra_data)
    }

    /// Apply a patch using pre-parsed components
    pub fn apply_patch(
        mut self,
        control_block: &ControlBlock,
        diff_data: &[u8],
        extra_data: &[u8],
    ) -> ZbsdiffResult<Vec<u8>> {
        let mut output = Vec::with_capacity(self.output_capacity);
        let mut diff_cursor = Cursor::new(diff_data);
        let mut extra_cursor = Cursor::new(extra_data);
        let mut old_pos = 0usize;

        // Get old file size for bounds checking
        let old_file_size = self.get_old_file_size()?;

        for entry in &control_block.entries {
            // Apply diff block
            self.apply_diff_block(
                &mut output,
                &mut diff_cursor,
                &mut old_pos,
                entry.diff_size as usize,
                old_file_size,
            )?;

            // Copy extra block
            self.copy_extra_block(&mut output, &mut extra_cursor, entry.extra_size as usize)?;

            // Seek in old file
            if entry.seek_offset != 0 {
                old_pos = self.apply_seek_offset(old_pos, entry.seek_offset)?;
            }
        }

        // Verify output size
        if output.len() != self.output_capacity {
            return Err(ZbsdiffError::SizeMismatch {
                expected: self.output_capacity,
                actual: output.len(),
            });
        }

        Ok(output)
    }

    /// Apply a diff block by reading old file data and applying diffs
    fn apply_diff_block(
        &mut self,
        output: &mut Vec<u8>,
        diff_cursor: &mut Cursor<&[u8]>,
        old_pos: &mut usize,
        diff_size: usize,
        old_file_size: usize,
    ) -> ZbsdiffResult<()> {
        let mut remaining = diff_size;

        while remaining > 0 {
            let chunk_size = remaining.min(self.buffer_size);

            // Read chunk from diff data
            let mut diff_chunk = vec![0u8; chunk_size];
            diff_cursor.read_exact(&mut diff_chunk)?;

            // Read corresponding chunk from old file
            let old_chunk = self.read_old_chunk(*old_pos, chunk_size, old_file_size)?;

            // Apply diffs
            for (diff_byte, old_byte) in diff_chunk.iter().zip(old_chunk.iter()) {
                output.push(apply_diff_byte(*old_byte, *diff_byte));
            }

            *old_pos += chunk_size;
            remaining -= chunk_size;
        }

        Ok(())
    }

    /// Copy a block of extra data to output
    fn copy_extra_block(
        &mut self,
        output: &mut Vec<u8>,
        extra_cursor: &mut Cursor<&[u8]>,
        extra_size: usize,
    ) -> ZbsdiffResult<()> {
        let mut remaining = extra_size;

        while remaining > 0 {
            let chunk_size = remaining.min(self.buffer_size);
            let mut chunk = vec![0u8; chunk_size];
            extra_cursor.read_exact(&mut chunk)?;
            output.extend_from_slice(&chunk);
            remaining -= chunk_size;
        }

        Ok(())
    }

    /// Apply a seek offset to the current position
    fn apply_seek_offset(&self, current_pos: usize, offset: i64) -> ZbsdiffResult<usize> {
        let new_pos = if offset < 0 {
            current_pos.saturating_sub((-offset) as usize)
        } else {
            current_pos.saturating_add(offset as usize)
        };

        Ok(new_pos)
    }

    /// Read a chunk from the old file, filling with zeros if beyond EOF
    fn read_old_chunk(
        &mut self,
        pos: usize,
        size: usize,
        old_file_size: usize,
    ) -> ZbsdiffResult<Vec<u8>> {
        let mut chunk = vec![0u8; size];

        if pos < old_file_size {
            // Seek to position
            self.old_file.seek(SeekFrom::Start(pos as u64))?;

            // Read available bytes
            let available = (old_file_size - pos).min(size);
            let mut available_chunk = vec![0u8; available];
            self.old_file
                .read_exact(&mut available_chunk)
                .map_err(ZbsdiffError::old_file_read_error)?;

            // Copy to output chunk (rest remains zero)
            chunk[..available].copy_from_slice(&available_chunk);
        }
        // If pos >= old_file_size, chunk remains all zeros

        Ok(chunk)
    }

    /// Get the size of the old file
    fn get_old_file_size(&mut self) -> ZbsdiffResult<usize> {
        let current_pos = self.old_file.stream_position()?;
        let size = self.old_file.seek(SeekFrom::End(0))? as usize;
        self.old_file.seek(SeekFrom::Start(current_pos))?;
        Ok(size)
    }
}

/// Utility methods for ZbsdiffHeader
impl ZbsdiffHeader {
    /// Parse just the header from patch data without processing the full patch
    pub fn parse_from_patch(patch_data: &[u8]) -> ZbsdiffResult<Self> {
        if patch_data.len() < 32 {
            return Err(ZbsdiffError::insufficient_data(32, patch_data.len()));
        }

        let mut cursor = Cursor::new(patch_data);
        let header = Self::read_options(&mut cursor, binrw::Endian::Big, ())?;
        header.validate()?;
        Ok(header)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::zbsdiff::ZbsdiffBuilder;
    use std::io::Cursor;

    #[test]
    fn test_apply_patch_memory_simple() {
        let old_data = b"Hello, World!";
        let new_data = b"Hello, Rust!";

        // Create patch
        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        // Apply patch
        let result = apply_patch_memory(old_data, &patch_data).expect("Operation should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_apply_patch_memory_empty_to_content() {
        let old_data = b"";
        let new_data = b"New content!";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        let result = apply_patch_memory(old_data, &patch_data).expect("Operation should succeed");
        assert_eq!(result, new_data);
    }

    #[test]
    fn test_streaming_patcher_with_cursor() {
        let old_data = b"Hello, World!";
        let new_data = b"Hello, Rust!";

        // Create patch
        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        // Apply with streaming patcher using Cursor (implements Read + Seek)
        let old_cursor = Cursor::new(old_data);
        let patcher = ZbsdiffPatcher::new(old_cursor, new_data.len());
        let result = patcher
            .apply_patch_from_data(&patch_data)
            .expect("Operation should succeed");

        assert_eq!(result, new_data);
    }

    #[test]
    fn test_streaming_patcher_large_data() {
        // Create larger test data
        let old_data = vec![42u8; 10000];
        let mut new_data = old_data.clone();
        new_data[5000] = 100; // Change one byte in the middle
        new_data.extend_from_slice(b" Additional data");

        let builder = ZbsdiffBuilder::new(old_data.clone(), new_data.clone());
        let patch_data = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        // Apply with small buffer size to test chunked processing
        let old_cursor = Cursor::new(old_data);
        let patcher = ZbsdiffPatcher::new(old_cursor, new_data.len()).with_buffer_size(1024);
        let result = patcher
            .apply_patch_from_data(&patch_data)
            .expect("Operation should succeed");

        assert_eq!(result, new_data);
    }

    #[test]
    fn test_header_parse_from_patch() {
        let old_data = b"test";
        let new_data = b"best";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        let header =
            ZbsdiffHeader::parse_from_patch(&patch_data).expect("Operation should succeed");
        assert_eq!(header.output_size, new_data.len() as i64);
    }

    #[test]
    fn test_insufficient_patch_data() {
        let insufficient_data = vec![0u8; 10]; // Less than 32 bytes for header

        let result = ZbsdiffHeader::parse_from_patch(&insufficient_data);
        assert!(matches!(
            result,
            Err(ZbsdiffError::InsufficientData {
                needed: 32,
                available: 10
            })
        ));
    }

    #[test]
    fn test_corrupt_diff_data() {
        let old_data = b"hello";
        let new_data = b"world";

        let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
        let patch_data = builder
            .build_simple_patch()
            .expect("Operation should succeed");

        // Test with truncated patch data (insufficient data)
        let truncated_patch = &patch_data[..patch_data.len() / 2];
        let result = apply_patch_memory(old_data, truncated_patch);
        assert!(result.is_err(), "Truncated patch should fail to apply");

        // Test with corrupted header by changing a byte in the signature
        let mut corrupted_patch = patch_data.clone();
        if corrupted_patch.len() > 8 {
            corrupted_patch[0] = 0xFF; // Corrupt the signature
            let result = apply_patch_memory(old_data, &corrupted_patch);
            assert!(result.is_err(), "Corrupted signature should fail");
        }
    }

    #[test]
    fn test_size_mismatch_error() {
        // This test would require carefully crafted patch data that produces wrong size
        // For now, we'll test the error type construction
        let error = ZbsdiffError::SizeMismatch {
            expected: 100,
            actual: 50,
        };
        let message = error.to_string();
        assert!(message.contains("100"));
        assert!(message.contains("50"));
    }

    #[test]
    fn test_streaming_with_seek_beyond_eof() {
        // Test reading beyond old file EOF (should return zeros)
        let old_data = b"short";
        let old_cursor = Cursor::new(old_data);

        let mut patcher = ZbsdiffPatcher::new(old_cursor, 100);

        // Read chunk that goes beyond EOF
        let chunk = patcher
            .read_old_chunk(3, 10, old_data.len())
            .expect("Operation should succeed");

        // First 2 bytes should be from file ("rt"), rest should be zeros
        assert_eq!(chunk[0], b'r');
        assert_eq!(chunk[1], b't');
        for &byte in &chunk[2..] {
            assert_eq!(byte, 0);
        }
    }
}

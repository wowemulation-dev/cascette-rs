//! Utility functions and structures for ZBSDIFF1 format operations
//!
//! This module provides compression utilities, control block structures,
//! and various helper functions needed for ZBSDIFF1 patch operations.

use crate::zbsdiff::error::{ZbsdiffError, ZbsdiffResult};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use std::io::{Cursor, Read, Write};

/// Decode a sign-magnitude encoded 64-bit integer (bsdiff `offtin`).
///
/// The bsdiff format stores signed 64-bit values using sign-magnitude
/// encoding: bit 63 is the sign bit, bits 0-62 hold the absolute value,
/// stored in little-endian byte order. This differs from two's complement
/// which Rust's `i64::from_le_bytes` assumes.
fn offtin(buf: [u8; 8]) -> i64 {
    let magnitude = i64::from_le_bytes([
        buf[0],
        buf[1],
        buf[2],
        buf[3],
        buf[4],
        buf[5],
        buf[6],
        buf[7] & 0x7F,
    ]);
    if buf[7] & 0x80 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

/// Encode a signed 64-bit integer using sign-magnitude (bsdiff `offtout`).
fn offtout(value: i64) -> [u8; 8] {
    let (magnitude, negative) = if value < 0 {
        (-value, true)
    } else {
        (value, false)
    };
    let mut buf = magnitude.to_le_bytes();
    if negative {
        buf[7] |= 0x80;
    }
    buf
}

/// Compress data using zlib compression
pub fn compress_zlib(data: &[u8]) -> ZbsdiffResult<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

/// Decompress zlib-compressed data
pub fn decompress_zlib(data: &[u8]) -> ZbsdiffResult<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();

    decoder
        .read_to_end(&mut decompressed)
        .map_err(ZbsdiffError::decompression_error)?;

    Ok(decompressed)
}

/// A single control block entry containing patch instructions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlEntry {
    /// Number of bytes to apply diff operation
    pub diff_size: i64,
    /// Number of bytes to copy from extra data
    pub extra_size: i64,
    /// Signed offset to seek in old file (can be negative)
    pub seek_offset: i64,
}

impl ControlEntry {
    /// Create a new control entry
    pub fn new(diff_size: i64, extra_size: i64, seek_offset: i64) -> Self {
        Self {
            diff_size,
            extra_size,
            seek_offset,
        }
    }

    /// Validate this control entry for reasonable values
    pub fn validate(&self) -> ZbsdiffResult<()> {
        // Reasonable size limits (10MB per operation)
        const MAX_OP_SIZE: i64 = 10_000_000;

        if self.diff_size < 0 {
            return Err(ZbsdiffError::application_failed(format!(
                "Negative diff_size: {}",
                self.diff_size
            )));
        }

        if self.extra_size < 0 {
            return Err(ZbsdiffError::application_failed(format!(
                "Negative extra_size: {}",
                self.extra_size
            )));
        }

        if self.diff_size > MAX_OP_SIZE {
            return Err(ZbsdiffError::application_failed(format!(
                "diff_size too large: {}",
                self.diff_size
            )));
        }

        if self.extra_size > MAX_OP_SIZE {
            return Err(ZbsdiffError::application_failed(format!(
                "extra_size too large: {}",
                self.extra_size
            )));
        }

        Ok(())
    }

    /// Calculate total bytes this entry will produce
    pub fn output_bytes(&self) -> i64 {
        self.diff_size + self.extra_size
    }
}

/// Control block containing all patch instructions
#[derive(Debug, Clone)]
pub struct ControlBlock {
    /// List of control entries
    pub entries: Vec<ControlEntry>,
}

impl ControlBlock {
    /// Create a new empty control block
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create a control block with the given entries
    pub fn with_entries(entries: Vec<ControlEntry>) -> ZbsdiffResult<Self> {
        let block = Self { entries };
        block.validate()?;
        Ok(block)
    }

    /// Parse a control block from compressed data
    pub fn from_compressed(data: &[u8]) -> ZbsdiffResult<Self> {
        let decompressed = decompress_zlib(data)?;
        let mut cursor = Cursor::new(&decompressed);
        let mut entries = Vec::new();

        while cursor.position() < decompressed.len() as u64 {
            // Check if we have enough bytes for a complete entry (24 bytes)
            let remaining = decompressed.len() as u64 - cursor.position();
            if remaining < 24 {
                if remaining > 0 {
                    return Err(ZbsdiffError::corrupt_patch(format!(
                        "Incomplete control entry: {} bytes remaining",
                        remaining
                    )));
                }
                break;
            }

            // Read 8 bytes for each value using bsdiff sign-magnitude encoding.
            // The bsdiff format uses sign-magnitude (bit 63 = sign, bits 0-62 =
            // magnitude) rather than two's complement for all three fields.
            let mut buf = [0u8; 8];
            cursor.read_exact(&mut buf)?;
            let diff_size = offtin(buf);

            cursor.read_exact(&mut buf)?;
            let extra_size = offtin(buf);

            cursor.read_exact(&mut buf)?;
            let seek_offset = offtin(buf);

            let entry = ControlEntry::new(diff_size, extra_size, seek_offset);
            entry.validate()?;

            entries.push(entry);
        }

        if entries.is_empty() {
            return Err(ZbsdiffError::EmptyControlBlock);
        }

        Ok(ControlBlock { entries })
    }

    /// Compress the control block to bytes
    pub fn to_compressed(&self) -> ZbsdiffResult<Vec<u8>> {
        let mut uncompressed = Vec::new();
        let mut cursor = Cursor::new(&mut uncompressed);

        for entry in &self.entries {
            cursor.write_all(&offtout(entry.diff_size))?;
            cursor.write_all(&offtout(entry.extra_size))?;
            cursor.write_all(&offtout(entry.seek_offset))?;
        }

        compress_zlib(&uncompressed)
    }

    /// Add a control entry to this block
    pub fn add_entry(&mut self, entry: ControlEntry) -> ZbsdiffResult<()> {
        entry.validate()?;
        self.entries.push(entry);
        Ok(())
    }

    /// Validate all entries in this control block
    pub fn validate(&self) -> ZbsdiffResult<()> {
        if self.entries.is_empty() {
            return Err(ZbsdiffError::EmptyControlBlock);
        }

        for (i, entry) in self.entries.iter().enumerate() {
            entry
                .validate()
                .map_err(|e| ZbsdiffError::invalid_control_entry(i, e.to_string()))?;
        }

        Ok(())
    }

    /// Calculate total expected output size from all entries
    pub fn total_output_size(&self) -> i64 {
        self.entries.iter().map(|e| e.output_bytes()).sum()
    }

    /// Get number of entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Check if this control block is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ControlBlock {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate that a seek offset is reasonable
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
pub fn validate_seek_offset(
    offset: i64,
    current_pos: usize,
    _file_size: usize,
) -> ZbsdiffResult<usize> {
    let new_pos = if offset < 0 {
        current_pos.saturating_sub((-offset) as usize)
    } else {
        current_pos.saturating_add(offset as usize)
    };

    // Allow seeking beyond EOF - will read as zeros
    Ok(new_pos)
}

/// Read a single byte from old data at position, returning 0 if beyond EOF
pub fn read_old_byte_at(old_data: &[u8], pos: usize) -> u8 {
    old_data.get(pos).copied().unwrap_or(0)
}

/// Apply a diff byte to an old byte
pub fn apply_diff_byte(old_byte: u8, diff_byte: u8) -> u8 {
    old_byte.wrapping_add(diff_byte)
}

/// Calculate statistics for a control block
#[derive(Debug, Clone)]
pub struct ControlBlockStats {
    /// Number of entries
    pub entry_count: usize,
    /// Total diff bytes
    pub total_diff_bytes: i64,
    /// Total extra bytes
    pub total_extra_bytes: i64,
    /// Total output bytes
    pub total_output_bytes: i64,
    /// Average seek distance
    pub avg_seek_distance: f64,
}

impl ControlBlock {
    /// Calculate statistics for this control block
    pub fn stats(&self) -> ControlBlockStats {
        if self.entries.is_empty() {
            return ControlBlockStats {
                entry_count: 0,
                total_diff_bytes: 0,
                total_extra_bytes: 0,
                total_output_bytes: 0,
                avg_seek_distance: 0.0,
            };
        }

        let total_diff_bytes: i64 = self.entries.iter().map(|e| e.diff_size).sum();
        let total_extra_bytes: i64 = self.entries.iter().map(|e| e.extra_size).sum();
        let total_output_bytes = total_diff_bytes + total_extra_bytes;

        let total_seek_distance: i64 = self.entries.iter().map(|e| e.seek_offset.abs()).sum();
        let avg_seek_distance = total_seek_distance as f64 / self.entries.len() as f64;

        ControlBlockStats {
            entry_count: self.entries.len(),
            total_diff_bytes,
            total_extra_bytes,
            total_output_bytes,
            avg_seek_distance,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_zlib_compression_round_trip() {
        let original = b"This is test data for compression round-trip testing. It contains repeating patterns patterns patterns.";

        let compressed = compress_zlib(original).expect("Operation should succeed");
        let decompressed = decompress_zlib(&compressed).expect("Operation should succeed");

        assert_eq!(original, decompressed.as_slice());
        assert!(compressed.len() < original.len()); // Should compress well
    }

    #[test]
    fn test_control_entry_validation() {
        // Valid entry
        let valid = ControlEntry::new(10, 5, -3);
        assert!(valid.validate().is_ok());

        // Invalid entries
        let negative_diff = ControlEntry::new(-1, 5, 0);
        assert!(negative_diff.validate().is_err());

        let negative_extra = ControlEntry::new(10, -1, 0);
        assert!(negative_extra.validate().is_err());

        let too_large_diff = ControlEntry::new(20_000_000, 0, 0);
        assert!(too_large_diff.validate().is_err());
    }

    #[test]
    fn test_control_entry_output_bytes() {
        let entry = ControlEntry::new(10, 5, 100);
        assert_eq!(entry.output_bytes(), 15);
    }

    #[test]
    fn test_control_block_creation() {
        let entries = vec![ControlEntry::new(10, 5, -3), ControlEntry::new(0, 20, 100)];

        let block = ControlBlock::with_entries(entries.clone()).expect("Operation should succeed");
        assert_eq!(block.entries.len(), 2);
        assert_eq!(block.total_output_size(), 35); // 10+5 + 0+20
        assert!(!block.is_empty());
    }

    #[test]
    fn test_control_block_compression_round_trip() {
        let entries = vec![
            ControlEntry::new(10, 5, -3),
            ControlEntry::new(0, 20, 100),
            ControlEntry::new(15, 0, -50),
        ];

        let original_block =
            ControlBlock::with_entries(entries.clone()).expect("Operation should succeed");
        let compressed = original_block
            .to_compressed()
            .expect("Operation should succeed");
        let decoded_block =
            ControlBlock::from_compressed(&compressed).expect("Operation should succeed");

        assert_eq!(original_block.entries.len(), decoded_block.entries.len());
        for (orig, decoded) in original_block
            .entries
            .iter()
            .zip(decoded_block.entries.iter())
        {
            assert_eq!(orig.diff_size, decoded.diff_size);
            assert_eq!(orig.extra_size, decoded.extra_size);
            assert_eq!(orig.seek_offset, decoded.seek_offset);
        }
    }

    #[test]
    fn test_control_block_empty() {
        let empty_block = ControlBlock::new();
        assert!(empty_block.is_empty());
        assert!(empty_block.validate().is_err());

        // Empty compressed data should fail
        let empty_compressed = compress_zlib(&[]).expect("Operation should succeed");
        assert!(ControlBlock::from_compressed(&empty_compressed).is_err());
    }

    #[test]
    fn test_control_block_corrupt_data() {
        // Incomplete entry (only 8 bytes instead of 24)
        let incomplete_data = vec![0u8; 8];
        let compressed = compress_zlib(&incomplete_data).expect("Operation should succeed");

        assert!(ControlBlock::from_compressed(&compressed).is_err());
    }

    #[test]
    fn test_utility_functions() {
        // Test seek offset validation
        let new_pos = validate_seek_offset(10, 50, 100).expect("Operation should succeed");
        assert_eq!(new_pos, 60);

        let new_pos = validate_seek_offset(-20, 50, 100).expect("Operation should succeed");
        assert_eq!(new_pos, 30);

        let new_pos = validate_seek_offset(-100, 50, 100).expect("Operation should succeed");
        assert_eq!(new_pos, 0); // Saturated

        // Test old byte reading
        let data = b"hello";
        assert_eq!(read_old_byte_at(data, 0), b'h');
        assert_eq!(read_old_byte_at(data, 4), b'o');
        assert_eq!(read_old_byte_at(data, 10), 0); // Beyond EOF

        // Test diff application
        assert_eq!(apply_diff_byte(100, 5), 105);
        assert_eq!(apply_diff_byte(250, 10), 4); // Wrapping
    }

    #[test]
    fn test_control_block_stats() {
        let entries = vec![
            ControlEntry::new(10, 5, -3),  // 15 output, seek 3
            ControlEntry::new(0, 20, 100), // 20 output, seek 100
            ControlEntry::new(15, 0, -50), // 15 output, seek 50
        ];

        let block = ControlBlock::with_entries(entries).expect("Operation should succeed");
        let stats = block.stats();

        assert_eq!(stats.entry_count, 3);
        assert_eq!(stats.total_diff_bytes, 25); // 10 + 0 + 15
        assert_eq!(stats.total_extra_bytes, 25); // 5 + 20 + 0
        assert_eq!(stats.total_output_bytes, 50); // 25 + 25
        assert_eq!(stats.avg_seek_distance, 51.0); // (3 + 100 + 50) / 3
    }

    #[test]
    fn test_control_block_empty_stats() {
        let empty = ControlBlock::new();
        let stats = empty.stats();

        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.total_diff_bytes, 0);
        assert_eq!(stats.total_extra_bytes, 0);
        assert_eq!(stats.total_output_bytes, 0);
        assert_eq!(stats.avg_seek_distance, 0.0);
    }
}

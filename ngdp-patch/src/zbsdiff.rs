//! ZBSDIFF patch format implementation
//!
//! Based on bsdiff by Colin Percival with zlib compression

use crate::error::{PatchError, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use std::io::{Cursor, Read, Write};
use tracing::{debug, trace};

/// ZBSDIFF1 magic signature
const ZBSDIFF1_SIGNATURE: u64 = 0x314646494453425A; // "ZBSDIFF1"

/// Patch format identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchFormat {
    /// ZBSDIFF version 1
    ZBSDiff1,
}

impl PatchFormat {
    /// Get the signature for this format
    pub fn signature(&self) -> u64 {
        match self {
            PatchFormat::ZBSDiff1 => ZBSDIFF1_SIGNATURE,
        }
    }

    /// Parse format from signature
    pub fn from_signature(sig: u64) -> Option<Self> {
        match sig {
            ZBSDIFF1_SIGNATURE => Some(PatchFormat::ZBSDiff1),
            _ => None,
        }
    }
}

/// ZBSDIFF patch header
#[derive(Debug, Clone)]
pub struct ZBSDiffHeader {
    /// Format signature
    pub signature: u64,
    /// Control block size (compressed)
    pub control_size: i64,
    /// Diff block size (compressed)
    pub diff_size: i64,
    /// Output file size
    pub output_size: i64,
}

impl ZBSDiffHeader {
    /// Read header from stream
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let signature = reader.read_u64::<BigEndian>()?;
        let control_size = reader.read_i64::<BigEndian>()?;
        let diff_size = reader.read_i64::<BigEndian>()?;
        let output_size = reader.read_i64::<BigEndian>()?;

        // Validate
        if PatchFormat::from_signature(signature).is_none() {
            return Err(PatchError::InvalidSignature {
                expected: ZBSDIFF1_SIGNATURE,
                actual: signature,
            });
        }

        if control_size < 0 || diff_size < 0 || output_size < 0 {
            return Err(PatchError::CorruptPatch(
                "Invalid sizes in patch header".to_string(),
            ));
        }

        Ok(Self {
            signature,
            control_size,
            diff_size,
            output_size,
        })
    }

    /// Write header to stream
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u64::<BigEndian>(self.signature)?;
        writer.write_i64::<BigEndian>(self.control_size)?;
        writer.write_i64::<BigEndian>(self.diff_size)?;
        writer.write_i64::<BigEndian>(self.output_size)?;
        Ok(())
    }
}

/// Apply a ZBSDIFF patch to original data
pub fn apply_patch(original: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    debug!(
        "Applying ZBSDIFF patch: original={} bytes, patch={} bytes",
        original.len(),
        patch.len()
    );

    let mut cursor = Cursor::new(patch);

    // Read header
    let header = ZBSDiffHeader::read(&mut cursor)?;
    trace!("Patch header: {:?}", header);

    // Read compressed blocks
    let control_block = read_compressed_block(&mut cursor, header.control_size as usize)?;
    let diff_block = read_compressed_block(&mut cursor, header.diff_size as usize)?;

    // Remaining data is the extra block
    let mut extra_compressed = Vec::new();
    cursor.read_to_end(&mut extra_compressed)?;
    let extra_block = decompress_block(&extra_compressed)?;

    // Apply patch
    let result = apply_patch_blocks(
        original,
        &control_block,
        &diff_block,
        &extra_block,
        header.output_size as usize,
    )?;

    debug!("Patch applied successfully: output={} bytes", result.len());
    Ok(result)
}

/// Read and decompress a block from the patch
fn read_compressed_block<R: Read>(reader: &mut R, size: usize) -> Result<Vec<u8>> {
    let mut compressed = vec![0u8; size];
    reader.read_exact(&mut compressed)?;
    decompress_block(&compressed)
}

/// Decompress a zlib block (skipping first 2 bytes which are zlib header)
fn decompress_block(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 2 {
        return Err(PatchError::CorruptPatch(
            "Compressed block too small".to_string(),
        ));
    }

    // Skip zlib header (2 bytes)
    let mut decoder = DeflateDecoder::new(&data[2..]);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| PatchError::DecompressionError(e.to_string()))?;

    Ok(decompressed)
}

/// Apply patch using control, diff, and extra blocks
fn apply_patch_blocks(
    original: &[u8],
    control: &[u8],
    diff: &[u8],
    extra: &[u8],
    output_size: usize,
) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(output_size);
    let mut control_cursor = Cursor::new(control);
    let mut diff_cursor = Cursor::new(diff);
    let mut extra_cursor = Cursor::new(extra);
    let mut old_pos = 0usize;

    while output.len() < output_size {
        // Read control data
        let diff_size = control_cursor.read_i64::<BigEndian>()? as usize;
        let extra_size = control_cursor.read_i64::<BigEndian>()? as usize;
        let seek_amount = control_cursor.read_i64::<BigEndian>()? as isize;

        // Sanity check
        if output.len() + diff_size + extra_size > output_size {
            return Err(PatchError::CorruptPatch(
                "Patch would exceed output size".to_string(),
            ));
        }

        // Apply diff block
        for _ in 0..diff_size {
            let diff_byte = diff_cursor.read_u8()?;
            let old_byte = if old_pos < original.len() {
                original[old_pos]
            } else {
                0
            };
            output.push(old_byte.wrapping_add(diff_byte));
            old_pos += 1;
        }

        // Copy extra block
        let mut extra_data = vec![0u8; extra_size];
        extra_cursor.read_exact(&mut extra_data)?;
        output.extend_from_slice(&extra_data);

        // Seek in old file
        if seek_amount < 0 {
            old_pos = old_pos.saturating_sub((-seek_amount) as usize);
        } else {
            old_pos = old_pos.saturating_add(seek_amount as usize);
        }
    }

    if output.len() != output_size {
        return Err(PatchError::SizeMismatch {
            expected: output_size,
            actual: output.len(),
        });
    }

    Ok(output)
}

/// Create a ZBSDIFF patch between original and modified data
///
/// Note: This is a simplified implementation for testing.
/// A full implementation would include suffix array construction
/// and optimal diff/extra block generation.
pub fn create_patch(original: &[u8], modified: &[u8]) -> Result<Vec<u8>> {
    debug!(
        "Creating ZBSDIFF patch: original={} bytes, modified={} bytes",
        original.len(),
        modified.len()
    );

    // For now, create a simple patch that stores the full modified file
    // This is valid but not optimal - a real implementation would
    // generate minimal diffs using suffix arrays

    let mut control = Vec::new();
    let diff = Vec::new();
    let mut extra = Vec::new();

    // Simple strategy: treat everything as extra data
    // Control: 0 diff bytes, all extra bytes, no seek
    control.write_i64::<BigEndian>(0)?; // diff_size
    control.write_i64::<BigEndian>(modified.len() as i64)?; // extra_size
    control.write_i64::<BigEndian>(0)?; // seek_amount

    // No diff data needed

    // All data goes to extra
    extra.extend_from_slice(modified);

    // Compress blocks
    let control_compressed = compress_block(&control)?;
    let diff_compressed = compress_block(&diff)?;
    let extra_compressed = compress_block(&extra)?;

    // Build patch file
    let mut patch = Vec::new();

    let header = ZBSDiffHeader {
        signature: ZBSDIFF1_SIGNATURE,
        control_size: control_compressed.len() as i64,
        diff_size: diff_compressed.len() as i64,
        output_size: modified.len() as i64,
    };

    header.write(&mut patch)?;
    patch.extend_from_slice(&control_compressed);
    patch.extend_from_slice(&diff_compressed);
    patch.extend_from_slice(&extra_compressed);

    debug!("Patch created: {} bytes", patch.len());
    Ok(patch)
}

/// Compress a block using deflate with zlib header
fn compress_block(data: &[u8]) -> Result<Vec<u8>> {
    let mut compressed = Vec::new();

    // Write zlib header (0x78, 0x9C for best compression)
    compressed.write_all(&[0x78, 0x9C])?;

    // Compress data
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    let deflated = encoder.finish()?;
    compressed.extend_from_slice(&deflated);

    Ok(compressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_format() {
        assert_eq!(PatchFormat::ZBSDiff1.signature(), ZBSDIFF1_SIGNATURE);
        assert_eq!(
            PatchFormat::from_signature(ZBSDIFF1_SIGNATURE),
            Some(PatchFormat::ZBSDiff1)
        );
        assert_eq!(PatchFormat::from_signature(0x1234567890ABCDEF), None);
    }

    #[test]
    fn test_simple_patch() {
        let original = b"Hello, World!";
        let modified = b"Hello, Rust!";

        // Create patch
        let patch = create_patch(original, modified).unwrap();

        // Apply patch
        let result = apply_patch(original, &patch).unwrap();

        assert_eq!(result, modified);
    }

    #[test]
    fn test_empty_to_data_patch() {
        let original = b"";
        let modified = b"New content here";

        let patch = create_patch(original, modified).unwrap();
        let result = apply_patch(original, &patch).unwrap();

        assert_eq!(result, modified);
    }

    #[test]
    fn test_data_to_empty_patch() {
        let original = b"Old content";
        let modified = b"";

        let patch = create_patch(original, modified).unwrap();
        let result = apply_patch(original, &patch).unwrap();

        assert_eq!(result, modified);
    }
}

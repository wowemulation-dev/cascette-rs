//! Compression support for Patch Archives using ESpec format
//!
//! Patch Archives use the ESpec compression specification language
//! to define how patch data is compressed. This module provides
//! utilities to work with ESpec in the context of patch archives.

use crate::espec::ESpec;
use crate::patch_archive::error::{PatchArchiveError, PatchArchiveResult};
use crate::patch_archive::utils::decompress_zlib;

/// Helper to parse compression specification from PA format
pub fn parse_compression_spec(info: &str) -> PatchArchiveResult<ESpec> {
    // PA files use a simplified block table format: b:{...}
    // Convert to standard ESpec format if needed
    let spec_str = if info.starts_with('{') && info.ends_with('}') {
        // Already in block table format, just add the 'b:' prefix
        format!("b:{}", info)
    } else if info.starts_with("b:") {
        // Already proper ESpec format
        info.to_string()
    } else {
        // Unknown format
        return Err(PatchArchiveError::InvalidCompressionSpec(info.to_string()));
    };

    ESpec::parse(&spec_str).map_err(|e| PatchArchiveError::InvalidCompressionSpec(e.to_string()))
}

/// Format ESpec for storage in PA file
pub fn format_compression_spec(spec: &ESpec) -> String {
    // PA files store the block table contents without the 'b:' prefix
    let full_spec = spec.to_string();

    if full_spec.starts_with("b:{") && full_spec.ends_with('}') {
        // Remove 'b:' prefix for PA storage format
        full_spec[2..].to_string()
    } else if let Some(compression) = full_spec.strip_prefix("b:") {
        // Handle simplified block tables like "b:z" -> "{*=z}"
        // This happens when ESpec optimizes single wildcard entries
        format!("{{*={}}}", compression)
    } else {
        // Keep as-is for non-block-table specs
        full_spec
    }
}

/// Decompress patch data according to ESpec specification
pub fn decompress_patch_data(data: &[u8], spec: &ESpec) -> PatchArchiveResult<Vec<u8>> {
    match spec {
        ESpec::None => Ok(data.to_vec()),

        ESpec::ZLib { .. } => {
            // Simple ZLib compression for entire data
            decompress_zlib(data)
        }

        ESpec::BlockTable { chunks } => {
            // Complex block-based compression
            let mut output = Vec::new();
            let mut offset = 0;

            for chunk in chunks {
                // Determine range for this chunk
                let chunk_end = if let Some(size_spec) = &chunk.size_spec {
                    let size = size_spec.size as usize;
                    let count = size_spec.count.unwrap_or(1) as usize;
                    (offset + size * count).min(data.len())
                } else {
                    // Final chunk with * specifier
                    data.len()
                };

                if offset >= data.len() {
                    break;
                }

                let chunk_data = &data[offset..chunk_end];

                // Decompress this chunk based on its spec
                let decompressed = decompress_chunk(chunk_data, &chunk.spec)?;
                output.extend_from_slice(&decompressed);

                offset = chunk_end;
            }

            Ok(output)
        }

        _ => {
            // Other ESpec types not typically used in PA files
            Err(PatchArchiveError::UnsupportedCompression('?'))
        }
    }
}

/// Decompress a single chunk based on its ESpec
fn decompress_chunk(data: &[u8], spec: &ESpec) -> PatchArchiveResult<Vec<u8>> {
    match spec {
        ESpec::None => Ok(data.to_vec()),
        ESpec::ZLib { .. } => decompress_zlib(data),
        _ => Err(PatchArchiveError::UnsupportedCompression('?')),
    }
}

/// Get compression method at a specific offset (for compatibility)
pub fn get_compression_at_offset(spec: &ESpec, offset: u64) -> &ESpec {
    match spec {
        ESpec::BlockTable { chunks } => {
            let mut current_offset = 0u64;

            for chunk in chunks {
                if let Some(size_spec) = &chunk.size_spec {
                    let size = size_spec.size;
                    let count = size_spec.count.unwrap_or(1) as u64;
                    let chunk_end = current_offset + size * count;

                    if offset >= current_offset && offset < chunk_end {
                        return &chunk.spec;
                    }

                    current_offset = chunk_end;
                } else {
                    // Final chunk with *
                    return &chunk.spec;
                }
            }

            // Default to last chunk if beyond all ranges
            chunks.last().map(|c| &c.spec).unwrap_or(spec)
        }
        _ => spec,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compression_spec_simple() {
        // PA format: just the block contents
        let spec = parse_compression_spec("{*=z}").expect("Operation should succeed");
        // ESpec optimizes {*=z} to just z when it's a single wildcard
        assert_eq!(spec.to_string(), "b:z");

        // Already ESpec format
        let spec = parse_compression_spec("b:{*=n}").expect("Operation should succeed");
        // ESpec optimizes {*=n} to just n when it's a single wildcard
        assert_eq!(spec.to_string(), "b:n");
    }

    #[test]
    fn test_parse_compression_spec_complex() {
        let spec = parse_compression_spec("{22=n,10044521=z,734880=n,*=z}")
            .expect("Operation should succeed");

        // Should parse as block table
        match spec {
            ESpec::BlockTable { chunks } => {
                // Note: chunks are parsed in order but may be reordered internally
                assert!(chunks.len() >= 3);
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }

    #[test]
    fn test_format_compression_spec() {
        // Create an ESpec and format for PA storage
        let spec = ESpec::parse("b:{256K=n,*=z:9}").expect("Operation should succeed");
        let formatted = format_compression_spec(&spec);

        // Should remove the 'b:' prefix for PA format
        assert!(formatted.starts_with('{'));
        assert!(formatted.ends_with('}'));
        assert!(formatted.contains("256K=n"));

        // Test simplified block table
        let spec = ESpec::parse("b:z").expect("Operation should succeed");
        let formatted = format_compression_spec(&spec);
        assert_eq!(formatted, "{*=z}");

        // Test with level
        let spec = ESpec::parse("b:z:9").expect("Operation should succeed");
        let formatted = format_compression_spec(&spec);
        assert_eq!(formatted, "{*=z:9}");
    }

    #[test]
    fn test_decompress_uncompressed() {
        let data = b"Hello, world!";
        let spec = ESpec::None;

        let result = decompress_patch_data(data, &spec).expect("Operation should succeed");
        assert_eq!(result, data);
    }

    #[test]
    fn test_get_compression_at_offset() {
        let spec = ESpec::parse("b:{100=n,200=z,*=n}").expect("Operation should succeed");

        // First 100 bytes are uncompressed
        let method = get_compression_at_offset(&spec, 50);
        assert_eq!(method.compression_type(), "none");

        // Next 200 bytes are compressed
        let method = get_compression_at_offset(&spec, 150);
        assert_eq!(method.compression_type(), "zlib");

        // Remainder is uncompressed
        let method = get_compression_at_offset(&spec, 500);
        assert_eq!(method.compression_type(), "none");
    }

    #[test]
    fn test_round_trip_format() {
        let original = "{22=n,734880=n,*=z}";
        let spec = parse_compression_spec(original).expect("Operation should succeed");
        let formatted = format_compression_spec(&spec);

        // Should be able to parse the formatted version
        let reparsed = parse_compression_spec(&formatted).expect("Operation should succeed");

        // Both should represent the same structure
        assert_eq!(spec.to_string(), reparsed.to_string());
    }
}

//! Re-encode patching
//!
//! Decodes a BLTE-encoded base file and re-encodes it with a different
//! encoding specification (ESpec). This is used when the file content
//! has not changed but the encoding parameters have (e.g., different
//! compression level or block layout).
//!
//! ## ESpec handling
//!
//! Walks all block chunks in the target ESpec and compresses each segment
//! independently.
//!
//! - Simple ESpecs (`n`, `z`, `z:9`, `g`, `c`, etc.): single-chunk BLTE using
//!   the compression mode from `ESpec::blte_compression_mode()`.
//! - `BlockTable` ESpecs (`b:{...}`): the raw content is split into segments
//!   matching the block sizes, each compressed per its chunk spec. The last
//!   chunk (size_spec = None, i.e. `*`) receives all remaining bytes.
//! - `Encrypted` ESpecs: not handled — re-encode patches are not issued for
//!   encrypted content in practice. Returns `PatchError::Format`.

use cascette_formats::CascFormat;
use cascette_formats::blte::{BlteBuilder, BlteFile, ChunkData, CompressionMode};
use cascette_formats::espec::{BlockChunk, ESpec};

use super::error::PatchError;

/// Re-encode a BLTE file with a new encoding specification.
///
/// The raw content is preserved; only the BLTE encoding changes.
///
/// # Arguments
///
/// * `base_blte_data` - BLTE-encoded base file
/// * `target_espec` - New encoding specification string
///
/// # Returns
///
/// The re-encoded BLTE data as bytes.
pub fn apply_reencode_patch(
    base_blte_data: &[u8],
    target_espec: &str,
) -> Result<Vec<u8>, PatchError> {
    // Decode the base BLTE to get raw content
    let base_blte = BlteFile::parse(base_blte_data)?;
    let raw_content = base_blte.decompress()?;

    // Parse the target ESpec
    let espec = ESpec::parse(target_espec)?;

    let new_blte = reencode_with_espec(&raw_content, &espec)?;
    let result: Vec<u8> = CascFormat::build(&new_blte)?;
    Ok(result)
}

/// Build a new BLTE from raw content using the full ESpec.
fn reencode_with_espec(raw_content: &[u8], espec: &ESpec) -> Result<BlteFile, PatchError> {
    match espec {
        ESpec::BlockTable { chunks } => reencode_block_table(raw_content, chunks),
        ESpec::Encrypted { .. } => Err(PatchError::Format(
            "re-encode of encrypted ESpec is not supported".to_string(),
        )),
        other => {
            // Simple ESpec: single-chunk BLTE
            let mode = other
                .blte_compression_mode()
                .unwrap_or(CompressionMode::ZLib);
            Ok(BlteBuilder::new()
                .with_compression(mode)
                .add_data(raw_content)?
                .build()?)
        }
    }
}

/// Build a multi-chunk BLTE from a BlockTable ESpec.
///
/// Each `BlockChunk` with a `size_spec` consumes that many bytes from the
/// raw content. The final chunk (`size_spec = None`, i.e. `*`) takes all
/// remaining bytes.
fn reencode_block_table(raw_content: &[u8], chunks: &[BlockChunk]) -> Result<BlteFile, PatchError> {
    let mut blte_chunks: Vec<ChunkData> = Vec::with_capacity(chunks.len());
    let mut offset = 0usize;

    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;

        let segment: &[u8] = if let Some(size_spec) = &chunk.size_spec {
            // Sized chunks: size × count bytes (count defaults to 1)
            let count = usize::try_from(size_spec.count.unwrap_or(1)).unwrap_or(1);
            let chunk_bytes = usize::try_from(size_spec.size)
                .unwrap_or(0)
                .saturating_mul(count);
            let end = (offset + chunk_bytes).min(raw_content.len());
            let seg = &raw_content[offset..end];
            offset = end;
            seg
        } else {
            // Wildcard (`*`): remainder of content
            &raw_content[offset..]
        };

        // Skip empty trailing segments (content exhausted before wildcard chunk)
        if segment.is_empty() && !is_last {
            continue;
        }

        let mode = chunk
            .spec
            .blte_compression_mode()
            .unwrap_or(CompressionMode::ZLib);

        blte_chunks.push(ChunkData::new(segment.to_vec(), mode)?);
    }

    if blte_chunks.is_empty() {
        // Degenerate case: build uncompressed single-chunk
        blte_chunks.push(ChunkData::new(raw_content.to_vec(), CompressionMode::None)?);
    }

    Ok(BlteFile::multi_chunk(blte_chunks)?)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_reencode_none_to_zlib() {
        let content = b"This is test content for re-encoding. Repeated data helps compression. Repeated data helps compression.";

        let base_blte = BlteFile::single_chunk(content.to_vec(), CompressionMode::None)
            .expect("base should succeed");
        let base_data: Vec<u8> = CascFormat::build(&base_blte).expect("build should succeed");

        let reencoded = apply_reencode_patch(&base_data, "z").expect("reencode should succeed");

        let result_blte = BlteFile::parse(&reencoded).expect("parse should succeed");
        let result_content = result_blte.decompress().expect("decompress should succeed");
        assert_eq!(result_content, content);
    }

    #[test]
    fn test_reencode_preserves_content() {
        let content = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let base_blte = BlteFile::single_chunk(content.clone(), CompressionMode::ZLib)
            .expect("base should succeed");
        let base_data: Vec<u8> = CascFormat::build(&base_blte).expect("build should succeed");

        let reencoded = apply_reencode_patch(&base_data, "n").expect("reencode should succeed");

        let result_blte = BlteFile::parse(&reencoded).expect("parse should succeed");
        let result_content = result_blte.decompress().expect("decompress should succeed");
        assert_eq!(result_content, content);
    }

    #[test]
    fn test_reencode_lz4() {
        let content = b"LZ4 compression test content. Repeated data helps compression. Repeated.";

        let base_blte = BlteFile::single_chunk(content.to_vec(), CompressionMode::None)
            .expect("base should succeed");
        let base_data: Vec<u8> = CascFormat::build(&base_blte).expect("build should succeed");

        // ESpec "z:{6,lz4hc}" maps to CompressionMode::LZ4 via blte_compression_mode()
        // Use a simple LZ4 re-encode path
        let espec = ESpec::parse("z:{6,lz4hc}").expect("parse lz4hc espec");
        let mode = espec.blte_compression_mode();
        assert_eq!(mode, Some(CompressionMode::LZ4));

        // apply via the re-encode path using the espec string that maps to LZ4
        let reencoded_blte = reencode_with_espec(content, &espec).expect("reencode");
        assert_eq!(reencoded_blte.chunks[0].mode, CompressionMode::LZ4);
        let decoded = reencoded_blte.decompress().expect("decompress");
        assert_eq!(decoded, content);
        drop(base_data);
    }

    #[test]
    fn test_reencode_block_table_two_chunks() {
        // 20 bytes of content: block table splits at 10 bytes
        let content: Vec<u8> = (0u8..20).collect();

        let base_blte = BlteFile::single_chunk(content.clone(), CompressionMode::None)
            .expect("base should succeed");
        let base_data: Vec<u8> = CascFormat::build(&base_blte).expect("build should succeed");

        // b:{10=n,*=z} — first 10 bytes uncompressed, remainder zlib
        let reencoded = apply_reencode_patch(&base_data, "b:{10=n,*=z}").expect("block reencode");

        let result_blte = BlteFile::parse(&reencoded).expect("parse");
        assert_eq!(result_blte.chunks.len(), 2);
        assert_eq!(result_blte.chunks[0].mode, CompressionMode::None);
        assert_eq!(result_blte.chunks[1].mode, CompressionMode::ZLib);

        let result_content = result_blte.decompress().expect("decompress");
        assert_eq!(result_content, content);
    }

    #[test]
    fn test_reencode_block_table_wildcard_only() {
        // b:{*=z} — single wildcard chunk, all content compressed
        let content: Vec<u8> = (0u8..50).collect();

        let base_blte =
            BlteFile::single_chunk(content.clone(), CompressionMode::None).expect("base");
        let base_data: Vec<u8> = CascFormat::build(&base_blte).expect("build");

        let reencoded = apply_reencode_patch(&base_data, "b:{*=z}").expect("block reencode");

        let result_blte = BlteFile::parse(&reencoded).expect("parse");
        assert_eq!(result_blte.chunks.len(), 1);
        assert_eq!(result_blte.chunks[0].mode, CompressionMode::ZLib);

        let result_content = result_blte.decompress().expect("decompress");
        assert_eq!(result_content, content);
    }

    #[test]
    fn test_reencode_encrypted_espec_rejected() {
        let content = b"some content";
        let blte = BlteFile::single_chunk(content.to_vec(), CompressionMode::None).expect("base");
        let base_data: Vec<u8> = CascFormat::build(&blte).expect("build");

        // e:{...} should return Format error
        let result = apply_reencode_patch(&base_data, "e:{0102030405060708,aabbccdd,z}");
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        assert!(matches!(err, PatchError::Format(_)));
    }
}

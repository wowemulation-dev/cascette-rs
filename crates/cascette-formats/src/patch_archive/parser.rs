//! Patch Archive parser for the block-based PA format
//!
//! Parses the real CDN patch manifest format:
//! - 10-byte header (big-endian)
//! - Optional extended header with encoding info (when flags & 0x02)
//! - Block table with per-block metadata
//! - Block data with file entries and patches

use crate::patch_archive::{
    PatchArchiveHeader,
    block::{
        FilePatch, PatchArchiveEncodingInfo, PatchBlock, PatchFileEntry, read_key, read_uint40_be,
    },
    error::{PatchArchiveError, PatchArchiveResult},
};
use binrw::BinRead;
use binrw::io::{Read, Seek, SeekFrom};

/// Parse the extended header (encoding info) from the reader
///
/// Layout: encoding_ckey + encoding_ekey + decoded_size(u32 BE) +
/// encoded_size(u32 BE) + espec_length(u8) + espec_string
pub fn parse_encoding_info<R: Read + Seek>(
    reader: &mut R,
    file_key_size: u8,
) -> PatchArchiveResult<PatchArchiveEncodingInfo> {
    let encoding_ckey = read_key(reader, file_key_size)?;
    let encoding_ekey = read_key(reader, file_key_size)?;

    let mut size_buf = [0u8; 4];
    reader.read_exact(&mut size_buf)?;
    let decoded_size = u32::from_be_bytes(size_buf);

    reader.read_exact(&mut size_buf)?;
    let encoded_size = u32::from_be_bytes(size_buf);

    let mut len_buf = [0u8; 1];
    reader.read_exact(&mut len_buf)?;
    let espec_length = len_buf[0] as usize;

    let mut espec_buf = vec![0u8; espec_length];
    reader.read_exact(&mut espec_buf)?;
    let espec =
        String::from_utf8(espec_buf).map_err(|e| PatchArchiveError::InvalidEntry(e.to_string()))?;

    Ok(PatchArchiveEncodingInfo {
        encoding_ckey,
        encoding_ekey,
        decoded_size,
        encoded_size,
        espec,
    })
}

/// Parse the block table entries (metadata only, not block data)
///
/// Each entry: last_file_ckey + block_md5(16) + block_offset(u32 BE)
pub fn parse_block_table<R: Read + Seek>(
    reader: &mut R,
    block_count: u16,
    file_key_size: u8,
) -> PatchArchiveResult<Vec<PatchBlock>> {
    let mut blocks = Vec::with_capacity(block_count as usize);

    for _ in 0..block_count {
        let last_file_ckey = read_key(reader, file_key_size)?;

        let mut block_md5 = [0u8; 16];
        reader.read_exact(&mut block_md5)?;

        let mut offset_buf = [0u8; 4];
        reader.read_exact(&mut offset_buf)?;
        let block_offset = u32::from_be_bytes(offset_buf);

        blocks.push(PatchBlock {
            last_file_ckey,
            block_md5,
            block_offset,
            file_entries: Vec::new(),
        });
    }

    Ok(blocks)
}

/// Parse file entries within a single block
///
/// File entries are terminated by a 0x00 sentinel byte (num_patches == 0).
/// Each entry: num_patches(u8) + target_ckey + decoded_size(uint40 BE)
///   + N × (source_ekey + source_decoded_size(uint40 BE) + patch_ekey
///     + patch_size(u32 BE) + patch_index(u8))
pub fn parse_block_data<R: Read + Seek>(
    reader: &mut R,
    header: &PatchArchiveHeader,
) -> PatchArchiveResult<Vec<PatchFileEntry>> {
    let mut entries = Vec::new();

    loop {
        let mut num_patches_buf = [0u8; 1];
        reader.read_exact(&mut num_patches_buf)?;
        let num_patches = num_patches_buf[0];

        if num_patches == 0 {
            break; // End of block sentinel
        }

        let target_ckey = read_key(reader, header.file_key_size)?;
        let decoded_size = read_uint40_be(reader)?;

        let mut patches = Vec::with_capacity(num_patches as usize);
        for _ in 0..num_patches {
            let source_ekey = read_key(reader, header.old_key_size)?;
            let source_decoded_size = read_uint40_be(reader)?;
            let patch_ekey = read_key(reader, header.patch_key_size)?;

            let mut size_buf = [0u8; 4];
            reader.read_exact(&mut size_buf)?;
            let patch_size = u32::from_be_bytes(size_buf);

            let mut idx_buf = [0u8; 1];
            reader.read_exact(&mut idx_buf)?;
            let patch_index = idx_buf[0];

            patches.push(FilePatch {
                source_ekey,
                source_decoded_size,
                patch_ekey,
                patch_size,
                patch_index,
            });
        }

        entries.push(PatchFileEntry {
            target_ckey,
            decoded_size,
            patches,
        });
    }

    Ok(entries)
}

/// Parse a complete Patch Archive from a reader
///
/// Reads header, optional encoding info, block table, and all block data.
pub fn parse_patch_archive<R: Read + Seek>(
    reader: &mut R,
) -> PatchArchiveResult<(
    PatchArchiveHeader,
    Option<PatchArchiveEncodingInfo>,
    Vec<PatchBlock>,
)> {
    // Read header (big-endian, 10 bytes)
    let header =
        PatchArchiveHeader::read_options(reader, binrw::Endian::Big, ()).map_err(|e| match e {
            binrw::Error::AssertFail { .. } => {
                PatchArchiveError::InvalidHeader("PA magic assertion failed".to_string())
            }
            other => PatchArchiveError::BinRw(other),
        })?;
    header.validate()?;

    // Parse extended header if present
    let encoding_info = if header.has_extended_header() {
        Some(parse_encoding_info(reader, header.file_key_size)?)
    } else {
        None
    };

    // Parse block table
    let mut blocks = parse_block_table(reader, header.block_count, header.file_key_size)?;

    // Parse block data for each block
    for block in &mut blocks {
        reader.seek(SeekFrom::Start(u64::from(block.block_offset)))?;
        block.file_entries = parse_block_data(reader, &header)?;
    }

    Ok((header, encoding_info, blocks))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::patch_archive::PatchArchiveBuilder;

    #[test]
    fn test_parse_built_archive() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![(
                [0x01; 16], // source ekey
                500,        // source decoded size
                [0x03; 16], // patch ekey
                200,        // patch size
                0,          // patch index
            )],
        );

        let data = builder.build().expect("build should succeed");
        let mut cursor = std::io::Cursor::new(&data);
        let (header, encoding_info, blocks) =
            parse_patch_archive(&mut cursor).expect("parse should succeed");

        assert_eq!(header.version, 2);
        assert_eq!(header.block_count, 1);
        assert!(encoding_info.is_none());
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].file_entries.len(), 1);
        assert_eq!(blocks[0].file_entries[0].target_ckey, [0x02; 16]);
        assert_eq!(blocks[0].file_entries[0].decoded_size, 1000);
        assert_eq!(blocks[0].file_entries[0].patches.len(), 1);
        assert_eq!(blocks[0].file_entries[0].patches[0].source_ekey, [0x01; 16]);
        assert_eq!(blocks[0].file_entries[0].patches[0].patch_size, 200);
    }
}

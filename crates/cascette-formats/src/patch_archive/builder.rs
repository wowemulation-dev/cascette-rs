//! Patch Archive builder for the block-based PA format
//!
//! Builds PA files matching the real CDN format:
//! - 10-byte header
//! - Optional encoding info (extended header)
//! - Block table with metadata
//! - Block data with file entries grouped by block size

use crate::patch_archive::{
    PatchArchive, PatchArchiveHeader,
    block::{FilePatch, PatchArchiveEncodingInfo, PatchFileEntry, write_key, write_uint40_be},
    error::PatchArchiveResult,
    header::{STANDARD_BLOCK_SIZE_BITS, STANDARD_KEY_SIZE},
};
use binrw::BinWrite;
use std::io::Write;

/// Builder for creating Patch Archives in the block-based format
pub struct PatchArchiveBuilder {
    /// Format version
    version: u8,
    /// Block size bits
    block_size_bits: u8,
    /// Optional encoding info for extended header
    encoding_info: Option<PatchArchiveEncodingInfo>,
    /// File entries to include (will be grouped into blocks)
    file_entries: Vec<PatchFileEntry>,
}

impl PatchArchiveBuilder {
    /// Create new builder with default settings
    pub fn new() -> Self {
        Self {
            version: 2,
            block_size_bits: STANDARD_BLOCK_SIZE_BITS,
            encoding_info: None,
            file_entries: Vec::new(),
        }
    }

    /// Set format version
    pub fn version(mut self, version: u8) -> Self {
        self.version = version;
        self
    }

    /// Set block size bits
    pub fn block_size_bits(mut self, bits: u8) -> Self {
        self.block_size_bits = bits;
        self
    }

    /// Set encoding info (extended header)
    pub fn encoding_info(mut self, info: PatchArchiveEncodingInfo) -> Self {
        self.encoding_info = Some(info);
        self
    }

    /// Add a file entry with multiple patches
    #[allow(clippy::type_complexity)]
    pub fn add_file_entry(
        &mut self,
        target_ckey: [u8; 16],
        decoded_size: u64,
        patches: Vec<([u8; 16], u64, [u8; 16], u32, u8)>,
    ) {
        self.file_entries.push(PatchFileEntry {
            target_ckey,
            decoded_size,
            patches: patches
                .into_iter()
                .map(
                    |(source_ekey, source_decoded_size, patch_ekey, patch_size, patch_index)| {
                        FilePatch {
                            source_ekey,
                            source_decoded_size,
                            patch_ekey,
                            patch_size,
                            patch_index,
                        }
                    },
                )
                .collect(),
        });
    }

    /// Add a pre-built file entry
    pub fn add_entry(&mut self, entry: PatchFileEntry) {
        self.file_entries.push(entry);
    }

    /// Get current number of file entries
    pub fn entry_count(&self) -> usize {
        self.file_entries.len()
    }

    /// Get reference to file entries
    pub fn entries(&self) -> &[PatchFileEntry] {
        &self.file_entries
    }

    /// Sort file entries by target CKey
    ///
    /// Agent.exe validates blocks are sorted by CKey. Sorting entries
    /// before building ensures blocks are sorted.
    pub fn sort_entries(&mut self) {
        self.file_entries
            .sort_by(|a, b| a.target_ckey.cmp(&b.target_ckey));
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.file_entries.clear();
    }

    /// Build the patch archive as binary data
    pub fn build(&self) -> PatchArchiveResult<Vec<u8>> {
        let mut output = Vec::new();
        self.write_to(&mut output)?;
        Ok(output)
    }

    /// Write the patch archive to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> PatchArchiveResult<()> {
        let file_key_size = STANDARD_KEY_SIZE;

        // Sort entries by target CKey for correct block ordering
        let mut sorted_entries = self.file_entries.clone();
        sorted_entries.sort_by(|a, b| a.target_ckey.cmp(&b.target_ckey));

        // Group entries into blocks by block size
        let block_size = 1usize << self.block_size_bits;
        let blocks = group_into_blocks(&sorted_entries, block_size, file_key_size);

        // Compute flags
        let flags = if self.encoding_info.is_some() {
            0x02
        } else {
            0x00
        };

        // Write header
        let header = PatchArchiveHeader {
            magic: *b"PA",
            version: self.version,
            file_key_size,
            old_key_size: STANDARD_KEY_SIZE,
            patch_key_size: STANDARD_KEY_SIZE,
            block_size_bits: self.block_size_bits,
            block_count: blocks.len() as u16,
            flags,
        };
        header.validate()?;

        let mut cursor = std::io::Cursor::new(Vec::new());
        header.write_options(&mut cursor, binrw::Endian::Big, ())?;
        writer.write_all(cursor.get_ref())?;

        // Write encoding info if present
        if let Some(ref info) = self.encoding_info {
            write_encoding_info(writer, info, file_key_size)?;
        }

        // Compute block table offsets
        // Block table size = block_count * (file_key_size + 16 + 4)
        let block_table_size = blocks.len() * (file_key_size as usize + 16 + 4);
        let block_data_start = cursor.get_ref().len()
            + encoding_info_size(self.encoding_info.as_ref(), file_key_size)
            + block_table_size;

        // Compute block offsets and serialize block data
        let mut block_data_parts: Vec<Vec<u8>> = Vec::new();
        let mut current_offset = block_data_start;

        for block in &blocks {
            // Serialize this block's file entries
            let block_data = serialize_block_data(block, &header)?;
            block_data_parts.push(block_data);
        }

        // Write block table with computed offsets
        for (i, block) in blocks.iter().enumerate() {
            let last_ckey = block.last().map(|e| e.target_ckey).unwrap_or([0u8; 16]);

            // Compute MD5 of block data
            let block_md5 = md5::compute(&block_data_parts[i]);

            write_key(writer, &last_ckey, file_key_size)?;
            writer.write_all(&block_md5.0)?;
            writer.write_all(&(current_offset as u32).to_be_bytes())?;

            current_offset += block_data_parts[i].len();
        }

        // Write block data
        for part in &block_data_parts {
            writer.write_all(part)?;
        }

        Ok(())
    }

    /// Build the patch archive object (parsed form)
    pub fn build_archive(&self) -> PatchArchiveResult<PatchArchive> {
        let data = self.build()?;
        let mut cursor = std::io::Cursor::new(&data);
        let (header, encoding_info, blocks) =
            crate::patch_archive::parser::parse_patch_archive(&mut cursor)?;
        Ok(PatchArchive {
            header,
            encoding_info,
            blocks,
        })
    }
}

impl Default for PatchArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Group file entries into blocks based on block size
fn group_into_blocks(
    entries: &[PatchFileEntry],
    block_size: usize,
    file_key_size: u8,
) -> Vec<Vec<PatchFileEntry>> {
    if entries.is_empty() {
        return vec![Vec::new()];
    }

    let mut blocks: Vec<Vec<PatchFileEntry>> = Vec::new();
    let mut current_block: Vec<PatchFileEntry> = Vec::new();
    let mut current_size = 0usize;

    for entry in entries {
        let entry_size = file_entry_size(entry, file_key_size);

        if !current_block.is_empty() && current_size + entry_size > block_size {
            blocks.push(std::mem::take(&mut current_block));
            current_size = 0;
        }

        current_size += entry_size;
        current_block.push(entry.clone());
    }

    if !current_block.is_empty() || blocks.is_empty() {
        blocks.push(current_block);
    }

    blocks
}

/// Calculate the serialized size of a file entry
fn file_entry_size(entry: &PatchFileEntry, file_key_size: u8) -> usize {
    // num_patches(1) + target_ckey + decoded_size(5)
    let base = 1 + file_key_size as usize + 5;
    // Per patch: source_ekey(16) + source_decoded_size(5) + patch_ekey(16)
    //   + patch_size(4) + patch_index(1) = 42 with 16-byte keys
    let patch_size = (16 + 5 + 16 + 4 + 1) * entry.patches.len();
    base + patch_size
}

/// Serialize file entries for a single block (including 0x00 sentinel)
fn serialize_block_data(
    entries: &[PatchFileEntry],
    header: &PatchArchiveHeader,
) -> PatchArchiveResult<Vec<u8>> {
    let mut data = Vec::new();

    for entry in entries {
        // num_patches
        data.push(entry.patches.len() as u8);

        // target_ckey
        write_key(&mut data, &entry.target_ckey, header.file_key_size)?;

        // decoded_size (uint40 BE)
        write_uint40_be(&mut data, entry.decoded_size)?;

        // patches
        for patch in &entry.patches {
            write_key(&mut data, &patch.source_ekey, header.old_key_size)?;
            write_uint40_be(&mut data, patch.source_decoded_size)?;
            write_key(&mut data, &patch.patch_ekey, header.patch_key_size)?;
            data.write_all(&patch.patch_size.to_be_bytes())?;
            data.push(patch.patch_index);
        }
    }

    // End of block sentinel
    data.push(0x00);

    Ok(data)
}

/// Write encoding info to writer
fn write_encoding_info<W: Write>(
    writer: &mut W,
    info: &PatchArchiveEncodingInfo,
    file_key_size: u8,
) -> PatchArchiveResult<()> {
    write_key(writer, &info.encoding_ckey, file_key_size)?;
    write_key(writer, &info.encoding_ekey, file_key_size)?;
    writer.write_all(&info.decoded_size.to_be_bytes())?;
    writer.write_all(&info.encoded_size.to_be_bytes())?;

    let espec_bytes = info.espec.as_bytes();
    writer.write_all(&[espec_bytes.len() as u8])?;
    writer.write_all(espec_bytes)?;

    Ok(())
}

/// Calculate serialized size of encoding info
fn encoding_info_size(info: Option<&PatchArchiveEncodingInfo>, file_key_size: u8) -> usize {
    match info {
        Some(info) => {
            // encoding_ckey + encoding_ekey + decoded_size(4) + encoded_size(4)
            // + espec_length(1) + espec
            (file_key_size as usize) * 2 + 4 + 4 + 1 + info.espec.len()
        }
        None => 0,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::CascFormat;
    use crate::patch_archive::PatchArchive;

    #[test]
    fn test_builder_creation() {
        let builder = PatchArchiveBuilder::new();
        assert_eq!(builder.version, 2);
        assert_eq!(builder.block_size_bits, 16);
        assert_eq!(builder.entry_count(), 0);
    }

    #[test]
    fn test_builder_configuration() {
        let builder = PatchArchiveBuilder::new().version(1).block_size_bits(14);
        assert_eq!(builder.version, 1);
        assert_eq!(builder.block_size_bits, 14);
    }

    #[test]
    fn test_build_with_entries() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        assert_eq!(archive.header.block_count, 1);
        assert_eq!(archive.blocks.len(), 1);
        assert_eq!(archive.blocks[0].file_entries.len(), 1);
        assert_eq!(archive.blocks[0].file_entries[0].target_ckey, [0x02; 16]);
    }

    #[test]
    fn test_build_with_encoding_info() {
        let mut builder = PatchArchiveBuilder::new();
        builder = builder.encoding_info(PatchArchiveEncodingInfo {
            encoding_ckey: [0xAA; 16],
            encoding_ekey: [0xBB; 16],
            decoded_size: 50_000_000,
            encoded_size: 49_500_000,
            espec: "b:{*=z}".to_string(),
        });
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        assert!(archive.header.has_extended_header());
        let info = archive.encoding_info.as_ref().unwrap();
        assert_eq!(info.encoding_ckey, [0xAA; 16]);
        assert_eq!(info.decoded_size, 50_000_000);
        assert_eq!(info.espec, "b:{*=z}");
    }

    #[test]
    fn test_round_trip() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x11; 16],
            10000,
            vec![
                ([0xAA; 16], 5000, [0xCC; 16], 300, 0),
                ([0xBB; 16], 8000, [0xDD; 16], 400, 1),
            ],
        );
        builder.add_file_entry(
            [0x22; 16],
            20000,
            vec![([0xEE; 16], 15000, [0xFF; 16], 500, 0)],
        );

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        assert_eq!(archive.total_file_entries(), 2);
        let entries: Vec<_> = archive.all_file_entries().collect();
        // Entries are sorted by target_ckey
        assert_eq!(entries[0].target_ckey, [0x11; 16]);
        assert_eq!(entries[0].patches.len(), 2);
        assert_eq!(entries[1].target_ckey, [0x22; 16]);
        assert_eq!(entries[1].patches.len(), 1);
    }

    #[test]
    fn test_sort_entries() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0xFF; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );
        builder.add_file_entry(
            [0x01; 16],
            2000,
            vec![([0x04; 16], 1000, [0x06; 16], 300, 0)],
        );

        builder.sort_entries();
        assert_eq!(builder.entries()[0].target_ckey, [0x01; 16]);
        assert_eq!(builder.entries()[1].target_ckey, [0xFF; 16]);
    }

    #[test]
    fn test_build_empty() {
        let builder = PatchArchiveBuilder::new();
        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");
        assert_eq!(archive.header.block_count, 1); // Always at least one block
        assert_eq!(archive.total_file_entries(), 0);
    }

    #[test]
    fn test_block_md5_computed() {
        let mut builder = PatchArchiveBuilder::new();
        builder.add_file_entry(
            [0x02; 16],
            1000,
            vec![([0x01; 16], 500, [0x03; 16], 200, 0)],
        );

        let data = builder.build().expect("build should succeed");
        let archive = PatchArchive::parse(&data).expect("parse should succeed");

        // Block MD5 should be non-zero (computed from block data)
        assert_ne!(archive.blocks[0].block_md5, [0u8; 16]);
    }
}

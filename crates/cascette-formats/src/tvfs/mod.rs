//! TVFS (TACT Virtual File System) format implementation
//!
//! TVFS is the virtual file system introduced in WoW 8.2 (CASC v3) that maps
//! file paths to encoding keys via a prefix tree, VFS span table, and container
//! file table. The binary format matches CascLib's `CascRootFile_TVFS.cpp`.
//!
//! # Tables
//!
//! - **Path table**: Recursive prefix tree encoding file paths. Each file leaf
//!   stores a VFS byte offset.
//! - **VFS table**: Span-based entries (span_count + N spans). Each span has a
//!   file_offset, span_length, and CFT byte offset.
//! - **Container file table (CFT)**: Fixed-stride entries addressed by byte
//!   offset. Each entry has an EKey, encoded size, and optional CKey/EST/patch
//!   fields depending on header flags.
//! - **Encoding spec table (EST)**: Null-terminated encoding specification
//!   strings, present when `TVFS_FLAG_ENCODING_SPEC` is set.

mod builder;
mod container_table;
mod error;
mod est_table;
mod header;
mod path_table;
#[allow(dead_code)]
mod utils;
mod vfs_table;

pub use builder::TvfsBuilder;
pub use container_table::{ContainerEntry, ContainerFileTable};
pub use error::{TvfsError, TvfsResult};
pub use est_table::EstTable;
#[allow(deprecated)]
pub use header::{
    TVFS_FLAG_ENCODING_SPEC, TVFS_FLAG_INCLUDE_CKEY, TVFS_FLAG_PATCH_SUPPORT,
    TVFS_FLAG_WRITE_SUPPORT, TvfsHeader,
};
pub use path_table::{PathFileEntry, PathTable, PathTreeNode};
pub use vfs_table::{VfsEntry, VfsSpan, VfsTable};

use crate::CascFormat;
use binrw::{BinRead, BinWrite};
use std::io::Cursor;

/// Complete TVFS file structure
#[derive(Debug, Clone)]
pub struct TvfsFile {
    /// TVFS header
    pub header: TvfsHeader,
    /// Path table with prefix tree structure
    pub path_table: PathTable,
    /// VFS table with file span mappings
    pub vfs_table: VfsTable,
    /// Container file table with EKeys
    pub container_table: ContainerFileTable,
    /// Encoding spec table (present when TVFS_FLAG_ENCODING_SPEC is set)
    pub est_table: Option<EstTable>,
}

impl TvfsFile {
    /// Parse TVFS from decompressed data.
    pub fn parse(data: &[u8]) -> TvfsResult<Self> {
        // Read header via binrw (handles the mixed endianness)
        let mut cursor = Cursor::new(data);
        let header = TvfsHeader::read_options(&mut cursor, binrw::Endian::Big, ())?;
        header.validate()?;

        // Extract table slices from the data using header offsets
        let path_start = header.path_table_offset as usize;
        let path_end = path_start + header.path_table_size as usize;
        if path_end > data.len() {
            return Err(TvfsError::InvalidTableOffset(header.path_table_offset));
        }
        let path_data = &data[path_start..path_end];

        let vfs_start = header.vfs_table_offset as usize;
        let vfs_end = vfs_start + header.vfs_table_size as usize;
        if vfs_end > data.len() {
            return Err(TvfsError::InvalidTableOffset(header.vfs_table_offset));
        }
        let vfs_data = &data[vfs_start..vfs_end];

        let cft_start = header.cft_table_offset as usize;
        let cft_end = cft_start + header.cft_table_size as usize;
        if cft_end > data.len() {
            return Err(TvfsError::InvalidTableOffset(header.cft_table_offset));
        }
        let cft_data = &data[cft_start..cft_end];

        // Parse tables
        let path_table = PathTable::parse(path_data)?;
        let vfs_table = VfsTable::parse(vfs_data, &header)?;
        let container_table = ContainerFileTable::parse(cft_data, &header)?;

        // Parse EST table if present
        let est_table = if header.has_encoding_spec() {
            if let (Some(offset), Some(size)) = (header.est_table_offset, header.est_table_size) {
                let est_start = offset as usize;
                let est_end = est_start + size as usize;
                if est_end > data.len() {
                    return Err(TvfsError::InvalidTableOffset(offset));
                }
                let est_data = &data[est_start..est_end];
                let mut est_cursor = Cursor::new(est_data);
                Some(EstTable::read_options(
                    &mut est_cursor,
                    binrw::Endian::Big,
                    (size,),
                )?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            header,
            path_table,
            vfs_table,
            container_table,
            est_table,
        })
    }

    /// Load TVFS file from BLTE-compressed data.
    pub fn load_from_blte(blte_data: &[u8]) -> TvfsResult<Self> {
        let blte = <crate::blte::BlteFile as CascFormat>::parse(blte_data).map_err(
            |e: Box<dyn std::error::Error>| {
                TvfsError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))
            },
        )?;
        let decompressed = blte.decompress()?;
        Self::parse(&decompressed)
    }

    /// Build TVFS to bytes.
    pub fn build(&self) -> TvfsResult<Vec<u8>> {
        let mut output = Vec::new();

        // Build table data first to compute sizes
        let path_data = self.path_table.data.clone();
        let vfs_data = self.vfs_table.data.clone();
        let cft_data = self.container_table.build(&self.header);
        let est_data = self.est_table.as_ref().map(|est| {
            let mut buf = Vec::new();
            for spec in &est.specs {
                buf.extend_from_slice(spec.as_bytes());
                buf.push(0); // null terminator
            }
            buf
        });

        // Compute a header with correct offsets
        let mut header = self.header.clone();
        let header_size = header.header_size as u32;

        // Table layout: header → path_table → est_table(optional) → cft_table → vfs_table
        let path_offset = header_size;
        let path_size = path_data.len() as u32;

        let est_offset;
        let est_size;
        let cft_start_offset;

        if let Some(ref est) = est_data {
            est_offset = path_offset + path_size;
            est_size = est.len() as u32;
            cft_start_offset = est_offset + est_size;
            header.est_table_offset = Some(est_offset);
            header.est_table_size = Some(est_size);
        } else {
            est_offset = 0;
            est_size = 0;
            let _ = (est_offset, est_size);
            cft_start_offset = path_offset + path_size;
        }

        let cft_size = cft_data.len() as u32;
        let vfs_offset = cft_start_offset + cft_size;
        let vfs_size = vfs_data.len() as u32;

        header.path_table_offset = path_offset;
        header.path_table_size = path_size;
        header.vfs_table_offset = vfs_offset;
        header.vfs_table_size = vfs_size;
        header.cft_table_offset = cft_start_offset;
        header.cft_table_size = cft_size;

        // Write header
        let mut cursor = Cursor::new(&mut output);
        header.write_options(&mut cursor, binrw::Endian::Big, ())?;

        // Write tables in layout order
        output.extend_from_slice(&path_data);
        if let Some(ref est) = est_data {
            output.extend_from_slice(est);
        }
        output.extend_from_slice(&cft_data);
        output.extend_from_slice(&vfs_data);

        Ok(output)
    }

    /// Resolve a file path to its container entry.
    pub fn resolve_path(&self, path: &str) -> Option<&ContainerEntry> {
        let vfs_offset = self.path_table.resolve_path(path)?;
        // Find the VFS entry at this offset
        let vfs_entry = self
            .vfs_table
            .entries
            .iter()
            .find(|e| e.offset == vfs_offset)?;
        let span = vfs_entry.spans.first()?;
        self.container_table
            .entries
            .iter()
            .find(|e| e.offset == span.cft_offset)
    }

    /// Enumerate all files in the TVFS.
    pub fn enumerate_files(&self) -> impl Iterator<Item = (&PathFileEntry, Option<&VfsEntry>)> {
        self.path_table.files.iter().map(move |file| {
            let vfs_entry = self
                .vfs_table
                .entries
                .iter()
                .find(|e| e.offset == file.vfs_offset);
            (file, vfs_entry)
        })
    }
}

impl crate::CascFormat for TvfsFile {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_tvfs_header_parsing() {
        let mut header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        header.update_table_info(38, 10, 48, 20, 68, 33, 1);

        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        let mut cursor = std::io::Cursor::new(&buffer);
        let parsed_header = TvfsHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        assert_eq!(parsed_header.magic, *b"TVFS");
        assert_eq!(parsed_header.format_version, 1);
        assert_eq!(parsed_header.header_size, 38);
        assert_eq!(parsed_header.ekey_size, 9);
        assert_eq!(parsed_header.pkey_size, 9);
        assert_eq!(parsed_header.flags, TVFS_FLAG_INCLUDE_CKEY);
    }

    #[test]
    fn test_tvfs_header_with_est() {
        let flags = TVFS_FLAG_INCLUDE_CKEY | TVFS_FLAG_ENCODING_SPEC | TVFS_FLAG_PATCH_SUPPORT;
        let header = TvfsHeader::new(flags);
        assert_eq!(header.header_size, 46);
        assert!(header.has_encoding_spec());
        assert!(header.has_patch_support());
        assert!(header.includes_content_keys());
    }

    #[test]
    fn test_tvfs_parse_rejects_invalid_format_version() {
        let mut header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        header.update_table_info(38, 10, 48, 20, 68, 33, 1);

        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        // Corrupt format_version (byte 4)
        buffer[4] = 2;
        buffer.resize(200, 0);

        let result = TvfsFile::parse(&buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_tvfs_parse_rejects_invalid_key_sizes() {
        let mut header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        header.update_table_info(38, 10, 48, 20, 68, 33, 1);

        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        // Corrupt ekey_size (byte 6)
        buffer[6] = 16;
        buffer.resize(200, 0);

        let result = TvfsFile::parse(&buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_cft_offs_size() {
        let header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        // Default cft_table_size is 0, so cft_offs_size should be 1
        assert_eq!(header.cft_offs_size(), 1);
    }

    #[test]
    fn test_offset_field_size_thresholds() {
        // Test via headers with different CFT sizes
        let mut h = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);

        h.cft_table_size = 0xFF;
        assert_eq!(h.cft_offs_size(), 1);

        h.cft_table_size = 0x100;
        assert_eq!(h.cft_offs_size(), 2);

        h.cft_table_size = 0xFFFF;
        assert_eq!(h.cft_offs_size(), 2);

        h.cft_table_size = 0x1_0000;
        assert_eq!(h.cft_offs_size(), 3);

        h.cft_table_size = 0xFF_FFFF;
        assert_eq!(h.cft_offs_size(), 3);

        h.cft_table_size = 0x100_0000;
        assert_eq!(h.cft_offs_size(), 4);
    }
}

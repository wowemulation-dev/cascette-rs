//! TVFS (TACT Virtual File System) format implementation
//!
//! TVFS is the virtual file system introduced in WoW 8.2 (CASC v3) that provides
//! a unified interface for managing content across multiple products and build
//! configurations. It replaces direct file path mappings with a flexible
//! namespace-based system that enables content deduplication and multi-product support.
//!
//! # Features
//!
//! - Parser and builder for TVFS manifests
//! - Hierarchical path table with prefix tree structure
//! - VFS table for file span mappings
//! - Container file table with EKeys and optional content keys
//! - BLTE integration for compressed manifests
//! - Streaming support for large files
//! - Round-trip validation

mod builder;
mod container_table;
mod error;
mod est_table;
mod header;
mod path_table;
mod utils;
mod vfs_table;

pub use builder::TvfsBuilder;
pub use container_table::{ContainerEntry, ContainerFileTable};
pub use error::{TvfsError, TvfsResult};
pub use est_table::EstTable;
pub use header::{
    TVFS_FLAG_ENCODING_SPEC, TVFS_FLAG_INCLUDE_CKEY, TVFS_FLAG_PATCH_SUPPORT,
    TVFS_FLAG_WRITE_SUPPORT, TvfsHeader,
};
pub use path_table::{PathNode, PathTable};
pub use vfs_table::{VfsEntry, VfsTable};

use crate::CascFormat;
use binrw::io::{Read, Seek, SeekFrom};
use binrw::{BinRead, BinResult, BinWrite};
use std::io::Cursor;

/// Complete TVFS file structure
#[derive(Debug, Clone)]
pub struct TvfsFile {
    /// TVFS header
    pub header: TvfsHeader,
    /// Path table with hierarchical structure
    pub path_table: PathTable,
    /// VFS table with file span mappings
    pub vfs_table: VfsTable,
    /// Container file table with EKeys
    pub container_table: ContainerFileTable,
    /// Encoding spec table (present when TVFS_FLAG_ENCODING_SPEC is set)
    pub est_table: Option<EstTable>,
}

impl BinRead for TvfsFile {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read header
        let header = TvfsHeader::read_options(reader, endian, ())?;

        // Load path table
        reader.seek(SeekFrom::Start(u64::from(header.path_table_offset)))?;
        let path_table = PathTable::read_options(reader, endian, (header.path_table_size,))?;

        // Load VFS table
        reader.seek(SeekFrom::Start(u64::from(header.vfs_table_offset)))?;
        let vfs_table = VfsTable::read_options(reader, endian, (header.vfs_table_size,))?;

        // Load container file table
        reader.seek(SeekFrom::Start(u64::from(header.cft_table_offset)))?;
        let container_table = ContainerFileTable::read_options(
            reader,
            endian,
            (header.cft_table_size, header.flags, header.ekey_size),
        )?;

        // Load EST table if present
        let est_table = if header.has_encoding_spec() {
            if let (Some(offset), Some(size)) = (header.est_table_offset, header.est_table_size) {
                reader.seek(SeekFrom::Start(u64::from(offset)))?;
                Some(EstTable::read_options(reader, endian, (size,))?)
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
}

impl BinWrite for TvfsFile {
    type Args<'a> = ();

    fn write_options<W: binrw::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write header
        self.header.write_options(writer, endian, ())?;

        // Write path table
        self.path_table.write_options(writer, endian, ())?;

        // Write VFS table
        self.vfs_table.write_options(writer, endian, ())?;

        // Write container file table
        self.container_table.write_options(
            writer,
            endian,
            (self.header.flags, self.header.ekey_size),
        )?;

        // Write EST table if present
        if let Some(ref est_table) = self.est_table {
            est_table.write_options(writer, endian, ())?;
        }

        Ok(())
    }
}

impl TvfsFile {
    /// Load TVFS file from BLTE-compressed data
    pub fn load_from_blte(blte_data: &[u8]) -> TvfsResult<Self> {
        // First decompress BLTE container
        let blte = crate::blte::BlteFile::parse(blte_data).map_err(|e| {
            TvfsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;
        let decompressed = blte.decompress()?;

        // Then parse TVFS structure
        let mut cursor = Cursor::new(&decompressed);
        let tvfs = Self::read_options(&mut cursor, binrw::Endian::Big, ())?;
        tvfs.header.validate()?;

        Ok(tvfs)
    }

    /// Parse TVFS from decompressed data
    pub fn parse(data: &[u8]) -> TvfsResult<Self> {
        let mut cursor = Cursor::new(data);
        let tvfs = Self::read_options(&mut cursor, binrw::Endian::Big, ())?;
        tvfs.header.validate()?;
        Ok(tvfs)
    }

    /// Build TVFS to bytes
    pub fn build(&self) -> TvfsResult<Vec<u8>> {
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);
        self.write_options(&mut cursor, binrw::Endian::Big, ())?;
        Ok(output)
    }

    /// Resolve file path to container entry
    pub fn resolve_path(&self, path: &str) -> Option<&ContainerEntry> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current_node = &self.path_table.root_node;

        // Navigate through path components
        for component in &components {
            current_node = self.find_child_node(current_node, component)?;
        }

        // Get file reference
        let file_id = current_node.file_id?;
        let vfs_entry = self.vfs_table.get_entry(file_id)?;
        let container_entry = self.container_table.get_entry(vfs_entry.container_index)?;

        Some(container_entry)
    }

    /// Find child node by name
    fn find_child_node(&self, parent: &PathNode, name: &str) -> Option<&PathNode> {
        for &child_id in &parent.children {
            let child = self.path_table.get_node(child_id)?;
            if child.path_part == name {
                return Some(child);
            }
        }
        None
    }

    /// Enumerate all files in the TVFS
    pub fn enumerate_files(&self) -> TvfsIterator<'_> {
        TvfsIterator {
            tvfs: self,
            stack: vec![(&self.path_table.root_node, String::new())],
        }
    }
}

/// Iterator for enumerating files in TVFS
pub struct TvfsIterator<'a> {
    tvfs: &'a TvfsFile,
    stack: Vec<(&'a PathNode, String)>, // (node, full_path)
}

impl<'a> Iterator for TvfsIterator<'a> {
    type Item = (String, &'a ContainerEntry);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, path)) = self.stack.pop() {
            // Add children to stack
            for &child_id in &node.children {
                if let Some(child) = self.tvfs.path_table.get_node(child_id) {
                    let child_path = if path.is_empty() {
                        child.path_part.clone()
                    } else {
                        format!("{}/{}", path, child.path_part)
                    };

                    self.stack.push((child, child_path));
                }
            }

            // Return file entry if this is a file
            if let Some(file_id) = node.file_id
                && let Some(vfs_entry) = self.tvfs.vfs_table.get_entry(file_id)
                && let Some(container_entry) = self
                    .tvfs
                    .container_table
                    .get_entry(vfs_entry.container_index)
            {
                return Some((path, container_entry));
            }
        }

        None
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
        // Create a minimal TVFS header for testing
        let mut header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        header.update_table_info(46, 10, 56, 20, 76, 33, 1);

        // Test serialization
        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        // Test parsing
        let mut cursor = std::io::Cursor::new(&buffer);
        let parsed_header = TvfsHeader::read_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        assert_eq!(parsed_header.magic, *b"TVFS");
        assert_eq!(parsed_header.format_version, 1);
        assert_eq!(parsed_header.ekey_size, 9);
        assert_eq!(parsed_header.pkey_size, 9);
    }

    #[test]
    fn test_path_node_creation() {
        let mut root = PathNode::root();
        let child = PathNode::new("test".to_string(), false);
        root.add_child(1);

        assert!(root.is_directory);
        assert_eq!(root.children.len(), 1);
        assert_eq!(child.path_part, "test");
        assert!(!child.is_directory);
    }

    #[test]
    fn test_tvfs_parse_rejects_invalid_format_version() {
        // Build a valid TVFS, then corrupt the format_version
        let mut header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        header.update_table_info(46, 10, 56, 20, 76, 33, 1);

        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        header
            .write_options(&mut cursor, binrw::Endian::Big, ())
            .expect("Operation should succeed");

        // Corrupt format_version (byte 4, after 4-byte magic)
        buffer[4] = 2;

        // Pad buffer to satisfy table offsets
        buffer.resize(200, 0);

        let result = TvfsFile::parse(&buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_tvfs_parse_rejects_invalid_key_sizes() {
        let mut header = TvfsHeader::new(TVFS_FLAG_INCLUDE_CKEY);
        header.update_table_info(46, 10, 56, 20, 76, 33, 1);

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
    fn test_vfs_entry_flags() {
        let entry = VfsEntry::new(1, 0, 0, 1024, false, true);

        assert!(!entry.is_directory());
        assert!(entry.is_compressed());
        assert_eq!(entry.file_size, 1024);
    }

    #[test]
    fn test_container_entry() {
        let ekey = [0x12; 9];
        let content_key = [0x34; 16];
        let entry = ContainerEntry::new(ekey, 1024, Some(512), Some(content_key));

        assert_eq!(entry.ekey, ekey.to_vec());
        assert_eq!(entry.file_size, 1024);
        assert_eq!(entry.effective_compressed_size(), 512);
        assert!(entry.has_content_key());
    }
}

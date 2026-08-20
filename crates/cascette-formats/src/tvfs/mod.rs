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

    /// Resolve a file path to its encoding key (truncated EKey bytes).
    ///
    /// Returns the raw ekey at the VFS span's CFT offset. The CFT entry
    /// stride is derived empirically (GCD of span offsets) so both the
    /// fixed-stride and flat-ekey (patched VFS, 1.15.4+) layouts resolve.
    pub fn resolve_path(&self, path: &str) -> Option<Vec<u8>> {
        let vfs_offset = self.path_table.resolve_path(path)?;
        let vfs_entry = self
            .vfs_table
            .entries
            .iter()
            .find(|e| e.offset == vfs_offset)?;
        let span = vfs_entry.spans.first()?;
        // Fall back to the flag-derived entry size when the CFT is too
        // small to infer a stride (degenerate single-entry manifests).
        let stride = self.cft_stride().unwrap_or_else(|| self.header.cft_entry_size());
        if (span.cft_offset as usize) % stride != 0 {
            return None; // offset not on an entry boundary (unresolvable layout)
        }
        self.container_table
            .ekey_at(span.cft_offset, self.header.ekey_size as usize)
            .map(|bytes| bytes.to_vec())
    }

    /// Empirically derive the CFT entry stride as the GCD of all span
    /// cft_offsets. The CFT entry size is not fixed across builds: patched
    /// VFS shards (1.15.4+) store a flat 9-byte ekey array where spans
    /// align to ekey_size, not to the flag-derived entry size.
    pub fn cft_stride(&self) -> Option<usize> {
        let mut stride = 0usize;
        for entry in &self.vfs_table.entries {
            for span in &entry.spans {
                stride = gcd(stride, span.cft_offset as usize);
            }
        }
        if stride == 0 {
            None
        } else {
            Some(stride)
        }
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

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(0, 9), 9);
        assert_eq!(gcd(9, 18), 9);
        assert_eq!(gcd(27, 9), 9);
        assert_eq!(gcd(35, 70), 35);
        assert_eq!(gcd(0, 0), 0);
    }

    #[test]
    fn test_resolve_path_flat_cft() {
        // Patched-VFS layout: CFT is a flat 9-byte ekey array; span
        // cft_offsets are multiples of ekey_size (9), not the flag-derived
        // entry size (27 with flags=0x7). GCD-stride resolution must find
        // the ekey at offsets 0, 9, 18 (which land mid-27-byte "entry").
        let ekeys: Vec<Vec<u8>> = (0..3).map(|i| vec![i + 1; 9]).collect();
        let cft_data: Vec<u8> = ekeys.iter().flatten().copied().collect();

        // VFS table: 3 single-span entries at cft offsets 0, 9, 18
        let mut vfs_data = Vec::new();
        for off in [0u32, 9, 18] {
            vfs_data.push(1); // span_count
            vfs_data.extend_from_slice(&0u32.to_be_bytes()); // file_offset
            vfs_data.extend_from_slice(&100u32.to_be_bytes()); // span_length
            vfs_data.push(off as u8); // cft_offset (1-byte width)
        }

        // Path table: 3 files, one per VFS entry
        let mut path_data = Vec::new();
        for i in 0..3 {
            let name = format!("f{i}").into_bytes();
            path_data.push(0); // separator
            path_data.push(name.len() as u8);
            path_data.extend_from_slice(&name);
            path_data.push(0xFF); // leaf
            path_data.extend_from_slice(&((i * 10) as u32).to_be_bytes()); // vfs_offset
        }

        // Assemble: header(46) + path + est(0) + cft + vfs
        let path_off = 46u32;
        let cft_off = path_off + path_data.len() as u32;
        let vfs_off = cft_off + cft_data.len() as u32;
        let mut header = Vec::new();
        header.extend_from_slice(b"TVFS");
        header.extend_from_slice(&[1, 46, 9, 9]); // ver, hdr_sz, ekey, pkey
        header.extend_from_slice(&0x07u32.to_be_bytes()); // flags: ckey+est+patch
        header.extend_from_slice(&path_off.to_be_bytes());
        header.extend_from_slice(&(path_data.len() as u32).to_be_bytes());
        header.extend_from_slice(&vfs_off.to_be_bytes());
        header.extend_from_slice(&(vfs_data.len() as u32).to_be_bytes());
        header.extend_from_slice(&cft_off.to_be_bytes());
        header.extend_from_slice(&(cft_data.len() as u32).to_be_bytes());
        header.extend_from_slice(&0u16.to_be_bytes()); // max_depth
        header.extend_from_slice(&0u32.to_be_bytes()); // est_off
        header.extend_from_slice(&0u32.to_be_bytes()); // est_sz

        let mut blob = header;
        blob.extend_from_slice(&path_data);
        blob.extend_from_slice(&cft_data);
        blob.extend_from_slice(&vfs_data);

        let tvfs = TvfsFile::parse(&blob).expect("parse");
        assert_eq!(tvfs.resolve_path("f0").unwrap(), ekeys[0]);
        assert_eq!(tvfs.resolve_path("f1").unwrap(), ekeys[1]); // offset 9
        assert_eq!(tvfs.resolve_path("f2").unwrap(), ekeys[2]); // offset 18
        assert_eq!(tvfs.cft_stride(), Some(9));
    }
}

/// Greatest common divisor (Euclid).
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

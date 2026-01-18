//! TVFS VFS table structures and parsing

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};

/// VFS table with file span mappings
#[derive(Debug, Clone)]
pub struct VfsTable {
    /// VFS entries
    pub entries: Vec<VfsEntry>,
}

/// VFS entry mapping file spans to container entries
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)] // Big-endian
#[bw(big)]
pub struct VfsEntry {
    /// File ID (unique identifier)
    pub file_id: u32,
    /// Container index (index into container file table)
    pub container_index: u32,
    /// File offset within container
    pub file_offset: u64,
    /// File size
    pub file_size: u32,
    /// Entry flags
    pub flags: u16,
}

// VFS entry flags
const VFS_FLAG_DIRECTORY: u16 = 0x8000;
const VFS_FLAG_COMPRESSED: u16 = 0x4000;

impl BinRead for VfsTable {
    type Args<'a> = (u32,); // table_size

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let table_size = args.0;
        let entry_size = std::mem::size_of::<VfsEntry>();
        let entry_count = table_size as usize / entry_size;

        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let entry = VfsEntry::read_options(reader, endian, ())?;
            entries.push(entry);
        }

        Ok(VfsTable { entries })
    }
}

impl BinWrite for VfsTable {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        for entry in &self.entries {
            entry.write_options(writer, endian, ())?;
        }
        Ok(())
    }
}

impl VfsTable {
    /// Create a new empty VFS table
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Get entry by file ID
    pub fn get_entry(&self, file_id: u32) -> Option<&VfsEntry> {
        self.entries.iter().find(|entry| entry.file_id == file_id)
    }

    /// Get entry by index
    pub fn get_entry_by_index(&self, index: usize) -> Option<&VfsEntry> {
        self.entries.get(index)
    }

    /// Add a new entry
    pub fn add_entry(&mut self, entry: VfsEntry) {
        self.entries.push(entry);
    }

    /// Get total table size in bytes
    pub fn table_size(&self) -> u32 {
        (self.entries.len() * std::mem::size_of::<VfsEntry>()) as u32
    }
}

impl Default for VfsTable {
    fn default() -> Self {
        Self::new()
    }
}

impl VfsEntry {
    /// Create a new VFS entry
    pub fn new(
        file_id: u32,
        container_index: u32,
        file_offset: u64,
        file_size: u32,
        is_directory: bool,
        is_compressed: bool,
    ) -> Self {
        let mut flags = 0u16;
        if is_directory {
            flags |= VFS_FLAG_DIRECTORY;
        }
        if is_compressed {
            flags |= VFS_FLAG_COMPRESSED;
        }

        Self {
            file_id,
            container_index,
            file_offset,
            file_size,
            flags,
        }
    }

    /// Check if this entry represents a directory
    pub fn is_directory(&self) -> bool {
        (self.flags & VFS_FLAG_DIRECTORY) != 0
    }

    /// Check if this entry is compressed
    pub fn is_compressed(&self) -> bool {
        (self.flags & VFS_FLAG_COMPRESSED) != 0
    }

    /// Set directory flag
    pub fn set_directory(&mut self, is_directory: bool) {
        if is_directory {
            self.flags |= VFS_FLAG_DIRECTORY;
        } else {
            self.flags &= !VFS_FLAG_DIRECTORY;
        }
    }

    /// Set compressed flag
    pub fn set_compressed(&mut self, is_compressed: bool) {
        if is_compressed {
            self.flags |= VFS_FLAG_COMPRESSED;
        } else {
            self.flags &= !VFS_FLAG_COMPRESSED;
        }
    }
}

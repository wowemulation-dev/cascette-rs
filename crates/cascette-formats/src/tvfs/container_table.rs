//! TVFS container file table structures and parsing

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};

use crate::tvfs::header::TVFS_FLAG_INCLUDE_CKEY;

/// Container file table with EKeys and optional content keys
#[derive(Debug, Clone)]
pub struct ContainerFileTable {
    /// Container entries
    pub entries: Vec<ContainerEntry>,
}

/// Container entry with EKey and optional content key
#[derive(Debug, Clone)]
pub struct ContainerEntry {
    /// Encoding key (variable length, typically 9 bytes for TACT)
    pub ekey: Vec<u8>,
    /// File size (decompressed)
    pub file_size: u32,
    /// Compressed size (optional, present when INCLUDE_CKEY flag is set)
    pub compressed_size: Option<u32>,
    /// Content key (optional, present when INCLUDE_CKEY flag is set)
    pub content_key: Option<[u8; 16]>,
}

impl BinRead for ContainerFileTable {
    type Args<'a> = (u32, u32, u8); // table_size, flags, ekey_size

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let (table_size, flags, ekey_size) = args;
        let include_ckey = (flags & TVFS_FLAG_INCLUDE_CKEY) != 0;

        let mut entries = Vec::new();
        let start_pos = reader.stream_position()?;

        while reader.stream_position()? - start_pos < u64::from(table_size) {
            let entry = ContainerEntry::read_with_flags(reader, endian, include_ckey, ekey_size)?;
            entries.push(entry);
        }

        Ok(ContainerFileTable { entries })
    }
}

impl BinWrite for ContainerFileTable {
    type Args<'a> = (u32, u8); // flags, ekey_size

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let (flags, ekey_size) = args;
        let include_ckey = (flags & TVFS_FLAG_INCLUDE_CKEY) != 0;

        for entry in &self.entries {
            entry.write_with_flags(writer, endian, include_ckey, ekey_size)?;
        }

        Ok(())
    }
}

impl ContainerFileTable {
    /// Create a new empty container file table
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Get entry by index
    pub fn get_entry(&self, index: u32) -> Option<&ContainerEntry> {
        self.entries.get(index as usize)
    }

    /// Add a new entry
    pub fn add_entry(&mut self, entry: ContainerEntry) {
        self.entries.push(entry);
    }

    /// Calculate table size in bytes
    pub fn calculate_size(&self, include_ckey: bool, ekey_size: u8) -> u32 {
        let base_size = ekey_size as usize + 4; // ekey + file_size (4)
        let entry_size = if include_ckey {
            base_size + 4 + 16 // + compressed_size (4) + content_key (16)
        } else {
            base_size
        };

        (self.entries.len() * entry_size) as u32
    }
}

impl Default for ContainerFileTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerEntry {
    /// Create a new container entry with a fixed-size ekey
    pub fn new(
        ekey: [u8; 9],
        file_size: u32,
        compressed_size: Option<u32>,
        content_key: Option<[u8; 16]>,
    ) -> Self {
        Self {
            ekey: ekey.to_vec(),
            file_size,
            compressed_size,
            content_key,
        }
    }

    /// Create a new container entry with a variable-length ekey
    pub fn with_ekey(
        ekey: Vec<u8>,
        file_size: u32,
        compressed_size: Option<u32>,
        content_key: Option<[u8; 16]>,
    ) -> Self {
        Self {
            ekey,
            file_size,
            compressed_size,
            content_key,
        }
    }

    /// Read container entry with flag-dependent fields
    fn read_with_flags<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        include_ckey: bool,
        ekey_size: u8,
    ) -> BinResult<Self> {
        // Read EKey (variable length from header)
        let mut ekey = vec![0u8; ekey_size as usize];
        reader.read_exact(&mut ekey)?;

        // Read file size
        let file_size = u32::read_options(reader, endian, ())?;

        // Read compressed size and content key if flag is set
        let (compressed_size, content_key) = if include_ckey {
            let comp_size = u32::read_options(reader, endian, ())?;
            let mut ckey = [0u8; 16];
            reader.read_exact(&mut ckey)?;
            (Some(comp_size), Some(ckey))
        } else {
            (None, None)
        };

        Ok(ContainerEntry {
            ekey,
            file_size,
            compressed_size,
            content_key,
        })
    }

    /// Write container entry with flag-dependent fields
    fn write_with_flags<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        include_ckey: bool,
        ekey_size: u8,
    ) -> BinResult<()> {
        // Write EKey, padded or truncated to ekey_size
        let size = ekey_size as usize;
        if self.ekey.len() >= size {
            writer.write_all(&self.ekey[..size])?;
        } else {
            writer.write_all(&self.ekey)?;
            writer.write_all(&vec![0u8; size - self.ekey.len()])?;
        }

        // Write file size
        self.file_size.write_options(writer, endian, ())?;

        // Write compressed size and content key if present
        if include_ckey {
            if let Some(comp_size) = self.compressed_size {
                comp_size.write_options(writer, endian, ())?;
            } else {
                // Default to file size if compressed size not set
                self.file_size.write_options(writer, endian, ())?;
            }

            if let Some(ref ckey) = self.content_key {
                writer.write_all(ckey)?;
            } else {
                // Write zero content key if not set
                writer.write_all(&[0u8; 16])?;
            }
        }

        Ok(())
    }

    /// Get EKey as hex string
    pub fn ekey_hex(&self) -> String {
        hex::encode(&self.ekey)
    }

    /// Get content key as hex string (if present)
    pub fn content_key_hex(&self) -> Option<String> {
        self.content_key.as_ref().map(hex::encode)
    }

    /// Check if entry has content key
    pub fn has_content_key(&self) -> bool {
        self.content_key.is_some()
    }

    /// Get effective compressed size (compressed_size or file_size)
    pub fn effective_compressed_size(&self) -> u32 {
        self.compressed_size.unwrap_or(self.file_size)
    }
}

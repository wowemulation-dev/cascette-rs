//! TVFS container file table (CFT) structures and parsing
//!
//! The CFT stores encoding keys and sizes for content referenced by VFS spans.
//! Entries are addressed by byte offset (from VFS span CftOffset), not by
//! sequential index. Entry size depends on header flags.

use crate::tvfs::error::{TvfsError, TvfsResult};
use crate::tvfs::header::{TVFS_FLAG_ENCODING_SPEC, TVFS_FLAG_PATCH_SUPPORT, TvfsHeader};

/// Container file table — raw byte blob addressed by offset from VFS spans.
#[derive(Debug, Clone)]
pub struct ContainerFileTable {
    /// Raw table bytes. VFS span CftOffset values index into this blob.
    pub data: Vec<u8>,

    /// Parsed entries for enumeration (byte offset → entry).
    /// Populated during parsing; empty for builder-created tables until build.
    pub entries: Vec<ContainerEntry>,
}

/// A single CFT entry extracted at a given byte offset.
#[derive(Debug, Clone)]
pub struct ContainerEntry {
    /// Byte offset within the CFT where this entry starts
    pub offset: u32,
    /// Encoding key (EKey), truncated to `ekey_size` bytes
    pub ekey: Vec<u8>,
    /// Encoded (compressed) size of the content
    pub encoded_size: u32,
    /// Content key (CKey), present when `TVFS_FLAG_INCLUDE_CKEY` is set
    pub content_key: Option<Vec<u8>>,
    /// Encoding spec index, present when `TVFS_FLAG_ENCODING_SPEC` is set
    pub est_index: Option<u32>,
    /// Patch entry CFT offset, present when `TVFS_FLAG_PATCH_SUPPORT` is set
    pub patch_offset: Option<u32>,
}

impl ContainerFileTable {
    /// Create an empty container file table
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Parse the CFT from raw bytes, enumerating all sequential entries.
    pub fn parse(data: &[u8], header: &TvfsHeader) -> TvfsResult<Self> {
        let entry_size = header.cft_entry_size();
        let mut entries = Vec::new();
        let mut offset = 0usize;

        while offset + entry_size <= data.len() {
            let entry = Self::read_entry_at(data, offset, header)?;
            entries.push(entry);
            offset += entry_size;
        }

        Ok(Self {
            data: data.to_vec(),
            entries,
        })
    }

    /// Read a single entry at the given byte offset.
    pub fn read_entry_at(
        data: &[u8],
        offset: usize,
        header: &TvfsHeader,
    ) -> TvfsResult<ContainerEntry> {
        let ekey_size = header.ekey_size as usize;
        let mut pos = offset;

        // EKey
        if pos + ekey_size > data.len() {
            return Err(TvfsError::CftTableTruncated(pos));
        }
        let ekey = data[pos..pos + ekey_size].to_vec();
        pos += ekey_size;

        // EncodedSize (4 bytes BE)
        if pos + 4 > data.len() {
            return Err(TvfsError::CftTableTruncated(pos));
        }
        let encoded_size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        // CKey (pkey_size bytes) if INCLUDE_CKEY
        let content_key = if header.includes_content_keys() {
            let pkey_size = header.pkey_size as usize;
            if pos + pkey_size > data.len() {
                return Err(TvfsError::CftTableTruncated(pos));
            }
            let ckey = data[pos..pos + pkey_size].to_vec();
            pos += pkey_size;
            Some(ckey)
        } else {
            None
        };

        // EST index (est_offs_size bytes BE) if ENCODING_SPEC
        let est_index = if (header.flags & TVFS_FLAG_ENCODING_SPEC) != 0 {
            let sz = header.est_offs_size() as usize;
            if pos + sz > data.len() {
                return Err(TvfsError::CftTableTruncated(pos));
            }
            let val = read_be_uint(&data[pos..pos + sz]);
            pos += sz;
            Some(val)
        } else {
            None
        };

        // Patch offset (cft_offs_size bytes BE) if PATCH_SUPPORT
        let patch_offset = if (header.flags & TVFS_FLAG_PATCH_SUPPORT) != 0 {
            let sz = header.cft_offs_size() as usize;
            if pos + sz > data.len() {
                return Err(TvfsError::CftTableTruncated(pos));
            }
            let val = read_be_uint(&data[pos..pos + sz]);
            pos += sz;
            let _ = pos; // suppress unused assignment warning
            Some(val)
        } else {
            None
        };

        Ok(ContainerEntry {
            offset: offset as u32,
            ekey,
            encoded_size,
            content_key,
            est_index,
            patch_offset,
        })
    }

    /// Look up an entry by byte offset into the CFT.
    pub fn get_entry_at_offset(
        &self,
        cft_offset: u32,
        header: &TvfsHeader,
    ) -> TvfsResult<ContainerEntry> {
        Self::read_entry_at(&self.data, cft_offset as usize, header)
    }

    /// Get entry by sequential index (for enumeration).
    pub fn get_entry(&self, index: u32) -> Option<&ContainerEntry> {
        self.entries.get(index as usize)
    }

    /// Calculate table size in bytes for the given entries.
    pub fn calculate_size(header: &TvfsHeader, entry_count: usize) -> u32 {
        (entry_count * header.cft_entry_size()) as u32
    }

    /// Build raw CFT bytes from entries.
    pub fn build(&self, header: &TvfsHeader) -> Vec<u8> {
        let entry_size = header.cft_entry_size();
        let mut data = Vec::with_capacity(self.entries.len() * entry_size);

        for entry in &self.entries {
            entry.write_to(&mut data, header);
        }

        data
    }
}

impl Default for ContainerFileTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerEntry {
    /// Create a new container entry with a 9-byte EKey.
    pub fn new(ekey: [u8; 9], encoded_size: u32, content_key: Option<Vec<u8>>) -> Self {
        Self {
            offset: 0,
            ekey: ekey.to_vec(),
            encoded_size,
            content_key,
            est_index: None,
            patch_offset: None,
        }
    }

    /// Write this entry into a byte buffer.
    pub fn write_to(&self, out: &mut Vec<u8>, header: &TvfsHeader) {
        let ekey_size = header.ekey_size as usize;

        // EKey (padded/truncated to ekey_size)
        if self.ekey.len() >= ekey_size {
            out.extend_from_slice(&self.ekey[..ekey_size]);
        } else {
            out.extend_from_slice(&self.ekey);
            out.resize(out.len() + ekey_size - self.ekey.len(), 0);
        }

        // EncodedSize (4 bytes BE)
        out.extend_from_slice(&self.encoded_size.to_be_bytes());

        // CKey if INCLUDE_CKEY
        if header.includes_content_keys() {
            let pkey_size = header.pkey_size as usize;
            if let Some(ref ckey) = self.content_key {
                if ckey.len() >= pkey_size {
                    out.extend_from_slice(&ckey[..pkey_size]);
                } else {
                    out.extend_from_slice(ckey);
                    out.resize(out.len() + pkey_size - ckey.len(), 0);
                }
            } else {
                out.resize(out.len() + pkey_size, 0);
            }
        }

        // EST index if ENCODING_SPEC
        if (header.flags & TVFS_FLAG_ENCODING_SPEC) != 0 {
            let sz = header.est_offs_size() as usize;
            write_be_uint(out, self.est_index.unwrap_or(0), sz);
        }

        // Patch offset if PATCH_SUPPORT
        if (header.flags & TVFS_FLAG_PATCH_SUPPORT) != 0 {
            let sz = header.cft_offs_size() as usize;
            write_be_uint(out, self.patch_offset.unwrap_or(0), sz);
        }
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
}

/// Read a big-endian unsigned integer of 1-4 bytes.
fn read_be_uint(bytes: &[u8]) -> u32 {
    let mut val = 0u32;
    for &b in bytes {
        val = (val << 8) | u32::from(b);
    }
    val
}

/// Write a big-endian unsigned integer in exactly `width` bytes.
fn write_be_uint(out: &mut Vec<u8>, val: u32, width: usize) {
    let bytes = val.to_be_bytes();
    // Take the last `width` bytes from the 4-byte BE representation
    out.extend_from_slice(&bytes[4 - width..]);
}

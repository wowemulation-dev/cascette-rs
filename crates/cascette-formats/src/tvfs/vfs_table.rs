//! TVFS VFS table structures and parsing
//!
//! The VFS table stores file span mappings. Each file in the path table
//! references its VFS entry by byte offset. Entries are variable-length:
//! `span_count(1) + N × (file_offset(4 BE) + span_length(4 BE) + cft_offset(cft_offs_size BE))`.

use crate::tvfs::error::{TvfsError, TvfsResult};
use crate::tvfs::header::TvfsHeader;

/// VFS table — raw byte blob addressed by offset from path table NodeValues.
#[derive(Debug, Clone)]
pub struct VfsTable {
    /// Raw table bytes. NodeValues from the path table index into this blob.
    pub data: Vec<u8>,

    /// Parsed entries for enumeration.
    pub entries: Vec<VfsEntry>,
}

/// A single VFS entry with one or more spans.
#[derive(Debug, Clone)]
pub struct VfsEntry {
    /// Byte offset within the VFS table where this entry starts
    pub offset: u32,
    /// Spans making up this file
    pub spans: Vec<VfsSpan>,
}

/// A single span within a VFS entry.
#[derive(Debug, Clone)]
pub struct VfsSpan {
    /// Offset within the referenced content (used for multi-span assembly)
    pub file_offset: u32,
    /// Content size of this span
    pub span_length: u32,
    /// Byte offset into the container file table (CFT)
    pub cft_offset: u32,
}

/// Maximum valid span count for a file entry.
/// Values 1-224 are file entries, 225-254 are "other", 255 is deleted.
const MAX_FILE_SPAN_COUNT: u8 = 224;

impl VfsTable {
    /// Create an empty VFS table.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Parse the VFS table from raw bytes, enumerating all sequential entries.
    pub fn parse(data: &[u8], header: &TvfsHeader) -> TvfsResult<Self> {
        let cft_offs_size = header.cft_offs_size() as usize;
        let span_size = 4 + 4 + cft_offs_size; // file_offset + span_length + cft_offset
        let mut entries = Vec::new();
        let mut pos = 0usize;

        while pos < data.len() {
            let entry_offset = pos as u32;

            // Read span count
            if pos >= data.len() {
                break;
            }
            let span_count = data[pos];
            pos += 1;

            // Skip deleted (255) or "other" (225-254) entries
            if span_count > MAX_FILE_SPAN_COUNT {
                // Skip the spans for this entry
                let skip = span_count as usize * span_size;
                if pos + skip > data.len() {
                    break;
                }
                pos += skip;
                continue;
            }

            if span_count == 0 {
                // Zero spans — skip
                entries.push(VfsEntry {
                    offset: entry_offset,
                    spans: Vec::new(),
                });
                continue;
            }

            let needed = span_count as usize * span_size;
            if pos + needed > data.len() {
                return Err(TvfsError::VfsTableTruncated(pos));
            }

            let mut spans = Vec::with_capacity(span_count as usize);
            for _ in 0..span_count {
                let file_offset =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;

                let span_length =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;

                let cft_offset = read_be_uint(&data[pos..pos + cft_offs_size]);
                pos += cft_offs_size;

                spans.push(VfsSpan {
                    file_offset,
                    span_length,
                    cft_offset,
                });
            }

            entries.push(VfsEntry {
                offset: entry_offset,
                spans,
            });
        }

        Ok(Self {
            data: data.to_vec(),
            entries,
        })
    }

    /// Read a single VFS entry at the given byte offset.
    pub fn read_entry_at(data: &[u8], offset: usize, header: &TvfsHeader) -> TvfsResult<VfsEntry> {
        let cft_offs_size = header.cft_offs_size() as usize;
        let span_size = 4 + 4 + cft_offs_size;
        let mut pos = offset;

        if pos >= data.len() {
            return Err(TvfsError::VfsTableTruncated(pos));
        }
        let span_count = data[pos];
        pos += 1;

        if span_count > MAX_FILE_SPAN_COUNT {
            return Err(TvfsError::InvalidSpanCount {
                count: span_count,
                offset,
            });
        }

        let needed = span_count as usize * span_size;
        if pos + needed > data.len() {
            return Err(TvfsError::VfsTableTruncated(pos));
        }

        let mut spans = Vec::with_capacity(span_count as usize);
        for _ in 0..span_count {
            let file_offset =
                u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;

            let span_length =
                u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;

            let cft_offset = read_be_uint(&data[pos..pos + cft_offs_size]);
            pos += cft_offs_size;

            spans.push(VfsSpan {
                file_offset,
                span_length,
                cft_offset,
            });
        }

        Ok(VfsEntry {
            offset: offset as u32,
            spans,
        })
    }

    /// Get entry by sequential index.
    pub fn get_entry_by_index(&self, index: usize) -> Option<&VfsEntry> {
        self.entries.get(index)
    }

    /// Build raw VFS table bytes from entries.
    pub fn build(entries: &[VfsEntry], header: &TvfsHeader) -> Vec<u8> {
        let cft_offs_size = header.cft_offs_size() as usize;
        let mut data = Vec::new();

        for entry in entries {
            data.push(entry.spans.len() as u8);
            for span in &entry.spans {
                data.extend_from_slice(&span.file_offset.to_be_bytes());
                data.extend_from_slice(&span.span_length.to_be_bytes());
                write_be_uint(&mut data, span.cft_offset, cft_offs_size);
            }
        }

        data
    }

    /// Get total table size in bytes.
    pub fn table_size(&self) -> u32 {
        self.data.len() as u32
    }
}

impl Default for VfsTable {
    fn default() -> Self {
        Self::new()
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
    out.extend_from_slice(&bytes[4 - width..]);
}

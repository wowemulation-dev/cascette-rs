//! TVFS (TACT Virtual File System) parser
//!
//! TVFS is a modern manifest format that defines a virtual filesystem
//! for game assets, used in newer Blizzard games.

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

use byteorder::{LittleEndian, ReadBytesExt};
use tracing::{debug, trace};

use crate::utils::read_uint40_from;
use crate::{Error, Result};

/// TVFS header structure
#[derive(Debug, Clone)]
pub struct TVFSHeader {
    /// Magic bytes "TVFS"
    pub magic: [u8; 4],
    /// Version (typically 1)
    pub version: u8,
    /// Header size in bytes
    pub header_size: u8,
    /// ESpec size in bytes
    pub espec_size: u8,
    /// Path table offset (40-bit)
    pub path_table_offset: u64,
    /// Path table size (40-bit)
    pub path_table_size: u64,
    /// VFS table offset (40-bit)
    pub vfs_table_offset: u64,
    /// VFS table size (40-bit)
    pub vfs_table_size: u64,
    /// Container file table offset (40-bit)
    pub cft_table_offset: u64,
    /// Container file table size (40-bit)
    pub cft_table_size: u64,
    /// Maximum path depth
    pub max_path_depth: u16,
    /// Flags (write support, patch references)
    pub flags: u8,
    /// ESpec table offset (40-bit) - if flags & 0x04
    pub espec_table_offset: Option<u64>,
    /// ESpec table size (40-bit) - if flags & 0x04
    pub espec_table_size: Option<u64>,
}

impl TVFSHeader {
    /// Parse TVFS header
    pub fn parse<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        if &magic != b"TVFS" {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid TVFS magic: {:?}", magic),
            )));
        }

        let version = reader.read_u8()?;
        if version != 1 {
            debug!("Unexpected TVFS version: {}", version);
        }

        let header_size = reader.read_u8()?;
        let espec_size = reader.read_u8()?;
        let path_table_offset = read_uint40_from(reader)?;
        let path_table_size = read_uint40_from(reader)?;
        let vfs_table_offset = read_uint40_from(reader)?;
        let vfs_table_size = read_uint40_from(reader)?;
        let cft_table_offset = read_uint40_from(reader)?;
        let cft_table_size = read_uint40_from(reader)?;
        let max_path_depth = reader.read_u16::<LittleEndian>()?;
        let flags = reader.read_u8()?;

        // Check for ESpec table (flags bit 2)
        let (espec_table_offset, espec_table_size) = if flags & 0x04 != 0 {
            (Some(read_uint40_from(reader)?), Some(read_uint40_from(reader)?))
        } else {
            (None, None)
        };

        Ok(TVFSHeader {
            magic,
            version,
            header_size,
            espec_size,
            path_table_offset,
            path_table_size,
            vfs_table_offset,
            vfs_table_size,
            cft_table_offset,
            cft_table_size,
            max_path_depth,
            flags,
            espec_table_offset,
            espec_table_size,
        })
    }

    /// Check if TVFS has write support
    pub fn has_write_support(&self) -> bool {
        self.flags & 0x01 != 0
    }

    /// Check if TVFS has patch references
    pub fn has_patch_references(&self) -> bool {
        self.flags & 0x02 != 0
    }

    /// Check if TVFS has ESpec table
    pub fn has_espec_table(&self) -> bool {
        self.flags & 0x04 != 0
    }
}

/// Path table entry
#[derive(Debug, Clone)]
pub struct PathEntry {
    /// Path string
    pub path: String,
    /// Path hash
    pub hash: u64,
}

/// VFS entry type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VFSEntryType {
    /// Regular file
    File,
    /// Deleted file
    Deleted,
    /// Inline data
    Inline,
    /// Link to another entry
    Link,
}

/// VFS table entry
#[derive(Debug, Clone)]
pub struct VFSEntry {
    /// Entry type
    pub entry_type: VFSEntryType,
    /// Span offset in CFT table
    pub span_offset: u32,
    /// Span count
    pub span_count: u32,
    /// Path index
    pub path_index: u32,
    /// File offset (for inline data)
    pub file_offset: Option<u64>,
    /// File size (for inline data)
    pub file_size: Option<u32>,
}

/// Container file table entry (file span)
#[derive(Debug, Clone)]
pub struct CFTEntry {
    /// Encoding key (or content key)
    pub ekey: Vec<u8>,
    /// File size
    pub file_size: u64,
    /// ESpec index (optional)
    pub espec_index: Option<u32>,
}

/// TVFS manifest
#[derive(Debug, Clone)]
pub struct TVFSManifest {
    /// Header information
    pub header: TVFSHeader,
    /// Path table
    pub path_table: Vec<PathEntry>,
    /// VFS table
    pub vfs_table: Vec<VFSEntry>,
    /// Container file table
    pub cft_table: Vec<CFTEntry>,
    /// ESpec table (optional)
    pub espec_table: Option<Vec<String>>,
    /// Path to VFS entry mapping
    path_map: HashMap<String, usize>,
}

impl TVFSManifest {
    /// Parse a TVFS manifest from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let header = TVFSHeader::parse(&mut cursor)?;
        
        debug!(
            "Parsing TVFS v{} with {} bytes, flags: {:#04x}",
            header.version,
            data.len(),
            header.flags
        );

        // Parse path table
        cursor.seek(SeekFrom::Start(header.path_table_offset))?;
        let path_table = Self::parse_path_table(&mut cursor, header.path_table_size)?;
        
        // Parse VFS table
        cursor.seek(SeekFrom::Start(header.vfs_table_offset))?;
        let vfs_table = Self::parse_vfs_table(&mut cursor, header.vfs_table_size)?;
        
        // Parse CFT table
        cursor.seek(SeekFrom::Start(header.cft_table_offset))?;
        let cft_table = Self::parse_cft_table(&mut cursor, header.cft_table_size, header.espec_size)?;
        
        // Parse ESpec table if present
        let espec_table = if header.has_espec_table() {
            if let (Some(offset), Some(size)) = (header.espec_table_offset, header.espec_table_size) {
                cursor.seek(SeekFrom::Start(offset))?;
                Some(Self::parse_espec_table(&mut cursor, size)?)
            } else {
                None
            }
        } else {
            None
        };

        // Build path map for quick lookups
        let mut path_map = HashMap::new();
        for (idx, entry) in vfs_table.iter().enumerate() {
            if entry.path_index < path_table.len() as u32 {
                let path = &path_table[entry.path_index as usize].path;
                path_map.insert(path.clone(), idx);
            }
        }

        Ok(TVFSManifest {
            header,
            path_table,
            vfs_table,
            cft_table,
            espec_table,
            path_map,
        })
    }

    /// Parse path table
    fn parse_path_table<R: Read>(reader: &mut R, size: u64) -> Result<Vec<PathEntry>> {
        let mut entries = Vec::new();
        let mut bytes_read = 0u64;
        
        debug!("Parsing path table with size: {}", size);
        
        while bytes_read < size {
            // Read varint for path length from reader
            let mut path_len = 0u32;
            let mut shift = 0;
            
            for _ in 0..5 {
                let byte = reader.read_u8()?;
                bytes_read += 1;
                
                // Extract 7-bit value
                let value = (byte & 0x7F) as u32;
                path_len |= value << shift;
                
                // Check continuation bit
                if byte & 0x80 == 0 {
                    break;
                }
                
                shift += 7;
            }
            
            if path_len == 0 || bytes_read >= size {
                break; // End of table or empty entry
            }
            
            // Read path string
            let mut path_bytes = vec![0u8; path_len as usize];
            reader.read_exact(&mut path_bytes)?;
            bytes_read += path_len as u64;
            
            let path = String::from_utf8(path_bytes).map_err(|e| {
                Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid UTF-8 in path: {}", e),
                ))
            })?;
            
            // Calculate path hash (Jenkins3)
            let hash = crate::utils::jenkins3_hashpath(&path);
            
            trace!("Path entry: {} (hash: {:#x})", path, hash);
            
            entries.push(PathEntry { path, hash });
        }
        
        debug!("Parsed {} path entries", entries.len());
        Ok(entries)
    }

    /// Parse VFS table
    fn parse_vfs_table<R: Read>(reader: &mut R, size: u64) -> Result<Vec<VFSEntry>> {
        let mut entries = Vec::new();
        let mut bytes_read = 0u64;
        
        while bytes_read < size {
            if bytes_read >= size {
                break;
            }
            
            // Read entry type and flags
            let type_byte = reader.read_u8()?;
            bytes_read += 1;
            
            let entry_type = match type_byte & 0x03 {
                0 => VFSEntryType::File,
                1 => VFSEntryType::Deleted,
                2 => VFSEntryType::Inline,
                3 => VFSEntryType::Link,
                _ => unreachable!(),
            };
            
            // Read span info for files
            let (span_offset, span_count) = if entry_type == VFSEntryType::File {
                // Read varint for span offset directly
                let mut offset = 0u32;
                let mut shift = 0;
                for _ in 0..5 {
                    let byte = reader.read_u8()?;
                    bytes_read += 1;
                    let value = (byte & 0x7F) as u32;
                    offset |= value << shift;
                    if byte & 0x80 == 0 {
                        break;
                    }
                    shift += 7;
                }
                
                // Read varint for span count directly
                let mut count = 0u32;
                shift = 0;
                for _ in 0..5 {
                    let byte = reader.read_u8()?;
                    bytes_read += 1;
                    let value = (byte & 0x7F) as u32;
                    count |= value << shift;
                    if byte & 0x80 == 0 {
                        break;
                    }
                    shift += 7;
                }
                
                (offset, count)
            } else {
                (0, 0)
            };
            
            // Read path index varint directly
            let mut path_index = 0u32;
            let mut shift = 0;
            for _ in 0..5 {
                let byte = reader.read_u8()?;
                bytes_read += 1;
                let value = (byte & 0x7F) as u32;
                path_index |= value << shift;
                if byte & 0x80 == 0 {
                    break;
                }
                shift += 7;
            }
            
            // Read inline data info if applicable
            let (file_offset, file_size) = if entry_type == VFSEntryType::Inline {
                let offset = read_uint40_from(reader)?;
                bytes_read += 5;
                let size = reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
                (Some(offset), Some(size))
            } else {
                (None, None)
            };
            
            entries.push(VFSEntry {
                entry_type,
                span_offset,
                span_count,
                path_index,
                file_offset,
                file_size,
            });
        }
        
        debug!("Parsed {} VFS entries", entries.len());
        Ok(entries)
    }

    /// Parse container file table
    fn parse_cft_table<R: Read>(reader: &mut R, size: u64, espec_size: u8) -> Result<Vec<CFTEntry>> {
        let mut entries = Vec::new();
        let mut bytes_read = 0u64;
        
        while bytes_read < size {
            // Read encoding key (16 bytes MD5)
            let mut ekey = vec![0u8; 16];
            reader.read_exact(&mut ekey)?;
            bytes_read += 16;
            
            // Read file size (40-bit)
            let file_size = read_uint40_from(reader)?;
            bytes_read += 5;
            
            // Read ESpec index if present
            let espec_index = if espec_size > 0 {
                let mut index = 0u32;
                for i in 0..espec_size {
                    let byte = reader.read_u8()?;
                    bytes_read += 1;
                    index |= (byte as u32) << (i * 8);
                }
                Some(index)
            } else {
                None
            };
            
            entries.push(CFTEntry {
                ekey,
                file_size,
                espec_index,
            });
        }
        
        debug!("Parsed {} CFT entries", entries.len());
        Ok(entries)
    }

    /// Parse ESpec table
    fn parse_espec_table<R: Read>(reader: &mut R, size: u64) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        let mut bytes_read = 0u64;
        
        while bytes_read < size {
            // Read varint for string length directly
            let mut spec_len = 0u32;
            let mut shift = 0;
            for _ in 0..5 {
                let byte = reader.read_u8()?;
                bytes_read += 1;
                let value = (byte & 0x7F) as u32;
                spec_len |= value << shift;
                if byte & 0x80 == 0 {
                    break;
                }
                shift += 7;
            }
            
            if spec_len == 0 || bytes_read >= size {
                break;
            }
            
            // Read ESpec string
            let mut spec_bytes = vec![0u8; spec_len as usize];
            reader.read_exact(&mut spec_bytes)?;
            bytes_read += spec_len as u64;
            
            let spec = String::from_utf8(spec_bytes).map_err(|e| {
                Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid UTF-8 in ESpec: {}", e),
                ))
            })?;
            
            entries.push(spec);
        }
        
        debug!("Parsed {} ESpec entries", entries.len());
        Ok(entries)
    }

    /// Resolve a file path to its file information
    pub fn resolve_path(&self, path: &str) -> Option<FileInfo> {
        // Look up VFS entry by path
        let vfs_index = *self.path_map.get(path)?;
        let vfs_entry = &self.vfs_table[vfs_index];
        
        match vfs_entry.entry_type {
            VFSEntryType::File => {
                // Collect file spans
                let mut spans = Vec::new();
                for i in 0..vfs_entry.span_count {
                    let cft_index = (vfs_entry.span_offset + i) as usize;
                    if cft_index < self.cft_table.len() {
                        let cft_entry = &self.cft_table[cft_index];
                        spans.push(FileSpan {
                            ekey: cft_entry.ekey.clone(),
                            file_size: cft_entry.file_size,
                            espec: cft_entry.espec_index.and_then(|idx| {
                                self.espec_table.as_ref()?.get(idx as usize).cloned()
                            }),
                        });
                    }
                }
                
                Some(FileInfo {
                    path: path.to_string(),
                    entry_type: vfs_entry.entry_type,
                    spans,
                    inline_data: None,
                })
            }
            VFSEntryType::Inline => {
                Some(FileInfo {
                    path: path.to_string(),
                    entry_type: vfs_entry.entry_type,
                    spans: Vec::new(),
                    inline_data: Some((vfs_entry.file_offset?, vfs_entry.file_size?)),
                })
            }
            _ => None,
        }
    }

    /// List all files in a directory
    pub fn list_directory(&self, dir_path: &str) -> Vec<DirEntry> {
        let mut entries = Vec::new();
        let dir_prefix = if dir_path.ends_with('/') {
            dir_path.to_string()
        } else if dir_path.is_empty() {
            String::new()
        } else {
            format!("{}/", dir_path)
        };
        
        for path_entry in &self.path_table {
            if path_entry.path.starts_with(&dir_prefix) {
                let relative_path = &path_entry.path[dir_prefix.len()..];
                
                // Check if it's a direct child (no additional slashes)
                if !relative_path.contains('/') && !relative_path.is_empty() {
                    if let Some(vfs_index) = self.path_map.get(&path_entry.path) {
                        let vfs_entry = &self.vfs_table[*vfs_index];
                        
                        let is_directory = false; // TVFS doesn't have explicit directories
                        let size = if vfs_entry.entry_type == VFSEntryType::File {
                            self.calculate_file_size(*vfs_index)
                        } else {
                            0
                        };
                        
                        entries.push(DirEntry {
                            name: relative_path.to_string(),
                            path: path_entry.path.clone(),
                            is_directory,
                            size,
                        });
                    }
                }
            }
        }
        
        entries
    }

    /// Calculate total size of a file (sum of all spans)
    fn calculate_file_size(&self, vfs_index: usize) -> u64 {
        let vfs_entry = &self.vfs_table[vfs_index];
        let mut total_size = 0u64;
        
        for i in 0..vfs_entry.span_count {
            let cft_index = (vfs_entry.span_offset + i) as usize;
            if cft_index < self.cft_table.len() {
                total_size += self.cft_table[cft_index].file_size;
            }
        }
        
        total_size
    }

    /// Get file count
    pub fn file_count(&self) -> usize {
        self.vfs_table.iter()
            .filter(|e| e.entry_type == VFSEntryType::File || e.entry_type == VFSEntryType::Inline)
            .count()
    }

    /// Get deleted file count
    pub fn deleted_count(&self) -> usize {
        self.vfs_table.iter()
            .filter(|e| e.entry_type == VFSEntryType::Deleted)
            .count()
    }

    /// Get total size of all files
    pub fn total_size(&self) -> u64 {
        self.cft_table.iter()
            .map(|e| e.file_size)
            .sum()
    }
}

/// File span information
#[derive(Debug, Clone)]
pub struct FileSpan {
    /// Encoding key for this span
    pub ekey: Vec<u8>,
    /// Size of this span
    pub file_size: u64,
    /// ESpec string (optional)
    pub espec: Option<String>,
}

/// File information
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File path
    pub path: String,
    /// Entry type
    pub entry_type: VFSEntryType,
    /// File spans (for regular files)
    pub spans: Vec<FileSpan>,
    /// Inline data location (offset, size) for inline entries
    pub inline_data: Option<(u64, u32)>,
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Entry name (relative to directory)
    pub name: String,
    /// Full path
    pub path: String,
    /// Whether this is a directory
    pub is_directory: bool,
    /// File size (0 for directories)
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tvfs_header_flags() {
        let header = TVFSHeader {
            magic: *b"TVFS",
            version: 1,
            header_size: 38,
            espec_size: 0,
            path_table_offset: 100,
            path_table_size: 200,
            vfs_table_offset: 300,
            vfs_table_size: 400,
            cft_table_offset: 700,
            cft_table_size: 500,
            max_path_depth: 10,
            flags: 0x07, // All flags set
            espec_table_offset: Some(1200),
            espec_table_size: Some(100),
        };

        assert!(header.has_write_support());
        assert!(header.has_patch_references());
        assert!(header.has_espec_table());
    }

    #[test]
    fn test_vfs_entry_type() {
        assert_eq!(VFSEntryType::File as u8, 0);
        assert_eq!(VFSEntryType::Deleted as u8, 1);
        assert_eq!(VFSEntryType::Inline as u8, 2);
        assert_eq!(VFSEntryType::Link as u8, 3);
    }
}
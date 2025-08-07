# File Format Parsers Implementation Guide

## Overview

This guide provides implementation instructions for all TACT file format parsers based on the prototype and reference implementations.

## Parser Architecture

```
tact-parser/
├── encoding.rs    - Encoding file (CKey → EKey mapping)
├── root.rs        - Root file (FileDataID → CKey mapping)
├── install.rs     - Install manifest (installation files)
├── download.rs    - Download manifest (update files)
├── build_config.rs - Build configuration
├── cdn_config.rs   - CDN configuration
├── tvfs.rs        - TVFS (modern manifest format)
└── size.rs        - Size file (file sizes)
```

## 1. Encoding File Parser

### Binary Structure
```rust
// Magic: "EN" (0x45 0x4E)
// Critical: All multi-byte values are BIG-ENDIAN

#[derive(Debug)]
pub struct EncodingHeader {
    pub magic: [u8; 2],           // "EN"
    pub version: u8,              // Version 1
    pub ckey_hash_size: u8,       // Usually 16 (MD5)
    pub ekey_hash_size: u8,       // Usually 16 (MD5)
    pub ckey_page_size_kb: u16,   // BE! Page size in KB
    pub ekey_page_size_kb: u16,   // BE! Page size in KB
    pub ckey_page_count: u32,     // BE! Number of pages
    pub ekey_page_count: u32,     // BE! Number of pages
    pub unk: u8,                  // Unknown, must be 0
    pub espec_block_size: u32,    // BE! ESpec block size
}
```

### Implementation
```rust
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};

pub struct EncodingFile {
    header: EncodingHeader,
    ckey_entries: HashMap<Vec<u8>, EncodingEntry>,
    ekey_to_ckey: HashMap<Vec<u8>, Vec<u8>>,
}

impl EncodingFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let mut magic = [0u8; 2];
        cursor.read_exact(&mut magic)?;
        ensure!(magic == [0x45, 0x4E], "Invalid encoding file magic");
        
        let version = cursor.read_u8()?;
        ensure!(version == 1, "Unsupported encoding version: {}", version);
        
        let ckey_hash_size = cursor.read_u8()?;
        let ekey_hash_size = cursor.read_u8()?;
        let ckey_page_size_kb = cursor.read_u16::<BigEndian>()?;
        let ekey_page_size_kb = cursor.read_u16::<BigEndian>()?;
        let ckey_page_count = cursor.read_u32::<BigEndian>()?;
        let ekey_page_count = cursor.read_u32::<BigEndian>()?;
        let unk = cursor.read_u8()?;
        let espec_block_size = cursor.read_u32::<BigEndian>()?;
        
        // Parse page tables
        let ckey_pages = Self::parse_page_table(
            &mut cursor, 
            ckey_page_count as usize,
            ckey_hash_size as usize
        )?;
        
        // Parse CEKey pages
        let mut ckey_entries = HashMap::new();
        let page_size = ckey_page_size_kb as usize * 1024;
        
        for page_info in ckey_pages {
            let page_data = Self::read_page(&mut cursor, page_size)?;
            Self::parse_ckey_page(
                &page_data,
                ckey_hash_size,
                ekey_hash_size,
                &mut ckey_entries
            )?;
        }
        
        // Similar for EKey pages...
        
        Ok(Self {
            header,
            ckey_entries,
            ekey_to_ckey: HashMap::new(), // Build reverse map
        })
    }
    
    fn parse_ckey_page(
        data: &[u8],
        ckey_size: u8,
        ekey_size: u8,
        entries: &mut HashMap<Vec<u8>, EncodingEntry>
    ) -> Result<()> {
        let mut offset = 0;
        
        while offset < data.len() {
            // Check for zero padding
            if data[offset..].iter().all(|&b| b == 0) {
                break;
            }
            
            // Read key count
            let key_count = data[offset];
            offset += 1;
            
            // Read file size (40-bit integer!)
            let size = read_uint40(&data[offset..offset + 5]);
            offset += 5;
            
            // Read content key
            let ckey = data[offset..offset + ckey_size as usize].to_vec();
            offset += ckey_size as usize;
            
            // Read encoding keys
            let mut ekeys = Vec::new();
            for _ in 0..key_count {
                let ekey = data[offset..offset + ekey_size as usize].to_vec();
                offset += ekey_size as usize;
                ekeys.push(ekey);
            }
            
            entries.insert(ckey.clone(), EncodingEntry {
                content_key: ckey,
                encoding_keys: ekeys,
                size,
            });
        }
        
        Ok(())
    }
}

// Critical: 40-bit integer reading
fn read_uint40(data: &[u8]) -> u64 {
    (data[0] as u64) |
    ((data[1] as u64) << 8) |
    ((data[2] as u64) << 16) |
    ((data[3] as u64) << 24) |
    ((data[4] as u64) << 32)
}
```

## 2. Root File Parser

### Multiple Formats Support
```rust
pub enum RootFile {
    V1(RootFileV1),  // Classic WoW
    V2(RootFileV2),  // Modern WoW
    TVFS(TVFSRoot),  // Latest format
}

// Version 1: MFST format
pub struct RootFileV1 {
    pub entries: Vec<RootEntryV1>,
}

pub struct RootEntryV1 {
    pub locale_flags: u32,
    pub content_flags: u32,
    pub file_data_id: u32,
    pub ckey: [u8; 16],
    pub name_hash: u64,  // Jenkins hash
}

// Version 2: Block-based format
pub struct RootFileV2 {
    pub blocks: Vec<RootBlock>,
}

pub struct RootBlock {
    pub locale_flags: u32,
    pub content_flags: u32,
    pub entries: Vec<(u32, [u8; 16])>,  // (FileDataID, CKey)
}
```

### Parser Implementation
```rust
impl RootFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        // Check magic to determine version
        if data.starts_with(b"TSFM") {
            Ok(RootFile::V2(Self::parse_v2(data)?))
        } else if data.starts_with(b"MFST") {
            Ok(RootFile::V1(Self::parse_v1(data)?))
        } else if data.starts_with(b"TVFS") {
            Ok(RootFile::TVFS(TVFSRoot::parse(data)?))
        } else {
            bail!("Unknown root file format");
        }
    }
    
    fn parse_v2(data: &[u8]) -> Result<RootFileV2> {
        let mut cursor = Cursor::new(data);
        
        // Skip magic
        cursor.set_position(4);
        
        let total_files = cursor.read_u32::<LittleEndian>()?;
        let named_files = cursor.read_u32::<LittleEndian>()?;
        
        let mut blocks = Vec::new();
        
        while cursor.position() < data.len() as u64 {
            let locale_flags = cursor.read_u32::<LittleEndian>()?;
            let content_flags = cursor.read_u32::<LittleEndian>()?;
            let count = cursor.read_u32::<LittleEndian>()?;
            
            let mut entries = Vec::new();
            let mut last_file_id = 0u32;
            
            for _ in 0..count {
                // File IDs are delta-encoded!
                let delta = read_varint(&mut cursor)?;
                last_file_id += delta;
                
                let mut ckey = [0u8; 16];
                cursor.read_exact(&mut ckey)?;
                
                entries.push((last_file_id, ckey));
            }
            
            blocks.push(RootBlock {
                locale_flags,
                content_flags,
                entries,
            });
        }
        
        Ok(RootFileV2 { blocks })
    }
}
```

## 3. Install Manifest Parser

### Structure with Tag System
```rust
pub struct InstallManifest {
    pub version: u8,
    pub hash_size: u8,
    pub tags: Vec<InstallTag>,
    pub entries: Vec<InstallEntry>,
}

pub struct InstallTag {
    pub name: String,
    pub tag_type: u16,
    pub files_mask: BitVec,  // Which files have this tag
}

pub struct InstallEntry {
    pub path: String,
    pub ckey: Vec<u8>,
    pub size: u32,
    pub tags: Vec<String>,  // Resolved tag names
}
```

### Implementation
```rust
impl InstallManifest {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Check magic "IN"
        let magic = [cursor.read_u8()?, cursor.read_u8()?];
        ensure!(magic == [0x49, 0x4E], "Invalid install manifest magic");
        
        let version = cursor.read_u8()?;
        let hash_size = cursor.read_u8()?;
        let num_tags = cursor.read_u16::<BigEndian>()?;
        let num_entries = cursor.read_u32::<BigEndian>()?;
        
        // Important: Calculate bits per tag
        let bytes_per_tag = (num_entries + 7) / 8;
        
        // Parse tags
        let mut tags = Vec::new();
        for _ in 0..num_tags {
            let name = read_cstring(&mut cursor)?;
            let tag_type = cursor.read_u16::<BigEndian>()?;
            
            // Read bitmask for this tag
            let mut mask_bytes = vec![0u8; bytes_per_tag as usize];
            cursor.read_exact(&mut mask_bytes)?;
            
            tags.push(InstallTag {
                name,
                tag_type,
                files_mask: BitVec::from_bytes(&mask_bytes),
            });
        }
        
        // Parse entries
        let mut entries = Vec::new();
        for i in 0..num_entries {
            let path = read_cstring(&mut cursor)?;
            
            let mut ckey = vec![0u8; hash_size as usize];
            cursor.read_exact(&mut ckey)?;
            
            let size = cursor.read_u32::<BigEndian>()?;
            
            // Resolve tags for this entry
            let mut entry_tags = Vec::new();
            for tag in &tags {
                if tag.files_mask[i as usize] {
                    entry_tags.push(tag.name.clone());
                }
            }
            
            entries.push(InstallEntry {
                path,
                ckey,
                size,
                tags: entry_tags,
            });
        }
        
        Ok(InstallManifest {
            version,
            hash_size,
            tags,
            entries,
        })
    }
}
```

## 4. Build/CDN Config Parsers

### Simple Key-Value Format
```rust
pub struct BuildConfig {
    pub values: HashMap<String, String>,
    pub hashes: HashMap<String, HashPair>,
}

pub struct HashPair {
    pub hash: String,
    pub size: u64,
}

impl BuildConfig {
    pub fn parse(text: &str) -> Result<Self> {
        let mut values = HashMap::new();
        let mut hashes = HashMap::new();
        
        for line in text.lines() {
            let line = line.trim();
            
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some((key, value)) = line.split_once(" = ") {
                let key = key.trim();
                let value = value.trim();
                
                // Check if value contains hash and size
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() == 2 && is_hex_hash(parts[0]) {
                    hashes.insert(key.to_string(), HashPair {
                        hash: parts[0].to_string(),
                        size: parts[1].parse()?,
                    });
                }
                
                values.insert(key.to_string(), value.to_string());
            }
        }
        
        Ok(BuildConfig { values, hashes })
    }
    
    pub fn get_hash(&self, key: &str) -> Option<&str> {
        self.hashes.get(key).map(|hp| hp.hash.as_str())
    }
}
```

## 5. TVFS Parser (Modern Format)

### Complex Structure
```rust
pub struct TVFSParser {
    pub header: TVFSHeader,
    pub path_table: Vec<PathEntry>,
    pub vfs_table: Vec<VFSEntry>,
    pub cft_table: Vec<CFTEntry>,
}

pub struct TVFSHeader {
    pub magic: u32,           // 'TVFS'
    pub version: u8,          // Always 1
    pub header_size: u8,      // >= 0x26
    pub ekey_size: u8,        // Usually 9
    pub patch_key_size: u8,   // Usually 9
    pub flags: u32,
    pub path_table_offset: u64,  // 40-bit!
    pub path_table_size: u64,    // 40-bit!
    pub vfs_table_offset: u64,   // 40-bit!
    pub vfs_table_size: u64,     // 40-bit!
    pub cft_table_offset: u64,   // 40-bit!
    pub cft_table_size: u64,     // 40-bit!
}
```

### Parser Implementation
```rust
impl TVFSParser {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let magic = cursor.read_u32::<BigEndian>()?;
        ensure!(magic == 0x54564653, "Invalid TVFS magic");
        
        let version = cursor.read_u8()?;
        ensure!(version == 1, "Unsupported TVFS version");
        
        let header_size = cursor.read_u8()?;
        let ekey_size = cursor.read_u8()?;
        let patch_key_size = cursor.read_u8()?;
        let flags = cursor.read_u32::<LittleEndian>()?;
        
        // Read 40-bit offsets and sizes
        let path_table_offset = read_uint40_cursor(&mut cursor)?;
        let path_table_size = read_uint40_cursor(&mut cursor)?;
        let vfs_table_offset = read_uint40_cursor(&mut cursor)?;
        let vfs_table_size = read_uint40_cursor(&mut cursor)?;
        let cft_table_offset = read_uint40_cursor(&mut cursor)?;
        let cft_table_size = read_uint40_cursor(&mut cursor)?;
        
        // Parse path table
        cursor.set_position(path_table_offset);
        let path_table = Self::parse_path_table(
            &mut cursor,
            path_table_size as usize
        )?;
        
        // Parse VFS table
        cursor.set_position(vfs_table_offset);
        let vfs_table = Self::parse_vfs_table(
            &mut cursor,
            vfs_table_size as usize,
            patch_key_size
        )?;
        
        // Parse CFT table
        cursor.set_position(cft_table_offset);
        let cft_table = Self::parse_cft_table(
            &mut cursor,
            cft_table_size as usize,
            ekey_size
        )?;
        
        Ok(TVFSParser {
            header: TVFSHeader { /* fields */ },
            path_table,
            vfs_table,
            cft_table,
        })
    }
}
```

## Common Utilities

### Variable-Length Integer
```rust
fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut result = 0u32;
    let mut shift = 0;
    
    loop {
        let byte = cursor.read_u8()?;
        result |= ((byte & 0x7F) as u32) << shift;
        
        if byte & 0x80 == 0 {
            break;
        }
        
        shift += 7;
        if shift >= 35 {
            bail!("Varint too long");
        }
    }
    
    Ok(result)
}
```

### C-String Reading
```rust
fn read_cstring(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let mut bytes = Vec::new();
    
    loop {
        let byte = cursor.read_u8()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    
    String::from_utf8(bytes)
        .map_err(|e| anyhow!("Invalid UTF-8: {}", e))
}
```

## Testing Strategies

### From Reference Implementations

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encoding_file_parsing() {
        // Test data from TACT.Net test suite
        let test_data = include_bytes!("../test_data/encoding.bin");
        let encoding = EncodingFile::parse(test_data).unwrap();
        
        // Verify known entries
        let known_ckey = hex::decode("abc123...").unwrap();
        assert!(encoding.ckey_entries.contains_key(&known_ckey));
    }
    
    #[test]
    fn test_40bit_integer() {
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A];
        let result = read_uint40(&data);
        assert_eq!(result, 0x9A78563412);
    }
    
    #[test]
    fn test_install_manifest_tags() {
        let manifest = InstallManifest::parse(TEST_MANIFEST).unwrap();
        
        // Find Windows-only files
        let windows_files: Vec<_> = manifest.entries.iter()
            .filter(|e| e.tags.contains(&"Windows".to_string()))
            .collect();
        
        assert!(!windows_files.is_empty());
    }
}
```

## Performance Considerations

### From TACTSharp
- Use memory-mapped files for large encoding files
- Parse pages on-demand, not all at once
- Cache frequently accessed entries

### From CascLib
- Binary search in sorted structures
- Skip zero-padded sections quickly
- Validate checksums in parallel

## Common Pitfalls

1. **Byte Order**: Encoding file uses BIG-ENDIAN
2. **40-bit Integers**: Used throughout TACT
3. **Delta Encoding**: File IDs in root files
4. **Variable-Length**: Many integers are variable-length
5. **Padding**: Pages often have zero padding

## Integration

### With BLTE Decompression
```rust
pub async fn load_encoding_file(
    cdn: &CdnClient,
    build_config: &BuildConfig,
) -> Result<EncodingFile> {
    let hash = build_config.get_hash("encoding")
        .ok_or_else(|| anyhow!("No encoding hash"))?;
    
    let data = cdn.download(hash).await?;
    let decompressed = blte::decompress(&data)?;
    
    EncodingFile::parse(&decompressed)
}
```

## Next Steps

1. Implement parsers in order of dependency
2. Add comprehensive error handling
3. Create integration tests with real data
4. Optimize for performance
5. Add streaming support for large files
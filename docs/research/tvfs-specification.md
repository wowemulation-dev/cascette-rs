# TVFS (TACT Virtual File System) - Complete Specification

## Overview

TVFS (TACT Virtual File System) is a modern manifest format introduced by Blizzard to replace older root file formats. It provides a flexible, extensible virtual file system with support for patches, encoding specifications, and optimized file lookups.

## Binary Structure

### Header Format

```c
struct TVFSHeader {
    char     magic[4];              // 'TVFS' (0x53465654)
    uint8_t  version;               // Always 1
    uint8_t  headerSize;            // Minimum 0x26 (38 bytes)
    uint8_t  EKeySize;              // Encoding key size (usually 9)
    uint8_t  PKeySize;              // Patch key size (usually 9)
    uint8_t  flags;                 // FileManifestFlags
    uint40_t pathTableOffset;       // 5 bytes! Offset to path table
    uint40_t pathTableSize;         // 5 bytes! Size of path table
    uint40_t vfsTableOffset;        // 5 bytes! Offset to VFS table
    uint40_t vfsTableSize;          // 5 bytes! Size of VFS table
    uint40_t cftTableOffset;        // 5 bytes! Container File Table offset
    uint40_t cftTableSize;          // 5 bytes! Container File Table size
    uint16_t maxMetafileSize;       // Maximum metafile size
    uint32_t buildVersion;          // Build version number
    
    // Optional fields (if headerSize > 0x26)
    uint8_t  rootCKey[16];          // Optional root content key
    uint8_t  patchEKey[9];          // Optional patch encoding key
};
```

### Flags

```c
enum FileManifestFlags {
    TVFS_FLAG_INCLUDE_CKEY  = 0x01,  // Include CKey in content records
    TVFS_FLAG_WRITE_SUPPORT = 0x02,  // Enable write support
    TVFS_FLAG_PATCH_SUPPORT = 0x04,  // Include patch file records
    TVFS_FLAG_LOWERCASE     = 0x08,  // Force lowercase paths
};
```

## Path Table

The path table contains the directory structure and file paths.

### Path Entry Structure

```c
struct PathEntry {
    uint8_t  entryType;             // Entry type (see below)
    varint   pathLength;            // Variable-length encoded
    char     path[pathLength];      // Path string (no null terminator)
    varint   nodeIndex;             // Index in VFS table
    
    // Optional fields based on type
    union {
        struct FileData {
            varint spanIndex;       // Index into span table
            uint8_t ckey[16];       // If INCLUDE_CKEY flag
        } file;
        
        struct InlineData {
            varint dataSize;
            uint8_t data[dataSize];
        } inline_data;
        
        struct LinkData {
            varint targetIndex;     // Link target
        } link;
    };
};
```

### Entry Types

```c
enum PathEntryType {
    ENTRY_NONE     = 0,  // Invalid/deleted entry
    ENTRY_FILE     = 1,  // Regular file
    ENTRY_DELETED  = 2,  // Deleted file (tombstone)
    ENTRY_INLINE   = 3,  // Inline data (small files)
    ENTRY_LINK     = 4,  // Symbolic link
};
```

### Path Table Parsing

```python
def parse_path_table(data, offset, size):
    """Parse TVFS path table"""
    entries = []
    end = offset + size
    
    while offset < end:
        entry_type = data[offset]
        offset += 1
        
        if entry_type == ENTRY_NONE:
            continue
        
        # Read path length
        path_length, bytes_read = read_varint(data, offset)
        offset += bytes_read
        
        # Read path
        path = data[offset:offset+path_length].decode('utf-8')
        offset += path_length
        
        # Read node index
        node_index, bytes_read = read_varint(data, offset)
        offset += bytes_read
        
        entry = {
            'type': entry_type,
            'path': path,
            'node_index': node_index
        }
        
        # Type-specific data
        if entry_type == ENTRY_FILE:
            span_index, bytes_read = read_varint(data, offset)
            offset += bytes_read
            entry['span_index'] = span_index
            
            if flags & TVFS_FLAG_INCLUDE_CKEY:
                entry['ckey'] = data[offset:offset+16]
                offset += 16
                
        elif entry_type == ENTRY_INLINE:
            data_size, bytes_read = read_varint(data, offset)
            offset += bytes_read
            entry['data'] = data[offset:offset+data_size]
            offset += data_size
            
        elif entry_type == ENTRY_LINK:
            target_index, bytes_read = read_varint(data, offset)
            offset += bytes_read
            entry['target'] = target_index
        
        entries.append(entry)
    
    return entries
```

## VFS Table

The VFS table contains file span information for content addressing.

### VFS Entry Structure

```c
struct VFSEntry {
    varint   spanCount;             // Number of spans for this file
    VFSSpan  spans[spanCount];      // Array of spans
};

struct VFSSpan {
    varint   offset;                // Offset within file
    varint   size;                  // Size of span
    varint   cftIndex;              // Index into Container File Table
    
    // If PATCH_SUPPORT flag
    uint8_t  patchEKey[PKeySize];   // Patch encoding key
};
```

### VFS Table Parsing

```python
def parse_vfs_table(data, offset, size, ekey_size, pkey_size, flags):
    """Parse TVFS VFS table"""
    entries = []
    end = offset + size
    
    while offset < end:
        # Read span count
        span_count, bytes_read = read_varint(data, offset)
        offset += bytes_read
        
        spans = []
        for _ in range(span_count):
            # Read span offset
            span_offset, bytes_read = read_varint(data, offset)
            offset += bytes_read
            
            # Read span size
            span_size, bytes_read = read_varint(data, offset)
            offset += bytes_read
            
            # Read CFT index
            cft_index, bytes_read = read_varint(data, offset)
            offset += bytes_read
            
            span = {
                'offset': span_offset,
                'size': span_size,
                'cft_index': cft_index
            }
            
            # Optional patch key
            if flags & TVFS_FLAG_PATCH_SUPPORT:
                span['patch_ekey'] = data[offset:offset+pkey_size]
                offset += pkey_size
            
            spans.append(span)
        
        entries.append({'spans': spans})
    
    return entries
```

## Container File Table (CFT)

The CFT maps logical files to their physical storage.

### CFT Entry Structure

```c
struct CFTEntry {
    uint8_t  EKey[EKeySize];        // Encoding key
    uint40_t fileSize;              // 5 bytes! Compressed file size
    
    // Optional based on flags
    uint8_t  CKey[16];              // If INCLUDE_CKEY flag
    uint32_t especIndex;            // Encoding specification index
};
```

### CFT Parsing

```python
def parse_cft_table(data, offset, size, ekey_size, flags):
    """Parse Container File Table"""
    entries = []
    end = offset + size
    
    while offset < end:
        entry = {}
        
        # Read encoding key
        entry['ekey'] = data[offset:offset+ekey_size]
        offset += ekey_size
        
        # Read file size (40-bit)
        entry['size'] = read_uint40(data, offset)
        offset += 5
        
        # Optional content key
        if flags & TVFS_FLAG_INCLUDE_CKEY:
            entry['ckey'] = data[offset:offset+16]
            offset += 16
        
        # Optional encoding spec
        if has_espec:  # Determined by other means
            espec_index, bytes_read = read_varint(data, offset)
            offset += bytes_read
            entry['espec_index'] = espec_index
        
        entries.append(entry)
    
    return entries
```

## ESpec Table

Optional table containing encoding specifications.

### Structure

```c
struct ESpecTable {
    uint32_t specCount;             // Number of specifications
    char     specs[];               // Null-terminated spec strings
};
```

### ESpec Format

```
Examples:
"n"                  - No compression
"z"                  - ZLib default
"z,9"                - ZLib level 9
"b:{n}:{z,9}"        - Block table with mixed compression
"e,s,{16*1024}:z"    - Encrypted with Salsa20, then ZLib
```

## File Resolution

### Path to Content Resolution

```python
class TVFSResolver:
    def __init__(self, tvfs_data):
        self.header = parse_header(tvfs_data)
        self.path_table = parse_path_table(tvfs_data, self.header.path_table_offset)
        self.vfs_table = parse_vfs_table(tvfs_data, self.header.vfs_table_offset)
        self.cft_table = parse_cft_table(tvfs_data, self.header.cft_table_offset)
    
    def resolve_file(self, path):
        """Resolve file path to content keys"""
        # Normalize path
        if self.header.flags & TVFS_FLAG_LOWERCASE:
            path = path.lower()
        
        # Find in path table
        path_entry = self.find_path_entry(path)
        if not path_entry:
            return None
        
        if path_entry['type'] == ENTRY_FILE:
            # Get VFS entry
            vfs_entry = self.vfs_table[path_entry['node_index']]
            
            # Get content info from spans
            content_info = []
            for span in vfs_entry['spans']:
                cft_entry = self.cft_table[span['cft_index']]
                content_info.append({
                    'offset': span['offset'],
                    'size': span['size'],
                    'ekey': cft_entry['ekey'],
                    'ckey': cft_entry.get('ckey'),
                })
            
            return content_info
            
        elif path_entry['type'] == ENTRY_INLINE:
            # Return inline data
            return {'inline': path_entry['data']}
            
        elif path_entry['type'] == ENTRY_LINK:
            # Follow link
            target = self.path_table[path_entry['target']]
            return self.resolve_file(target['path'])
```

## Optimizations

### Variable-Size References

TVFS uses optimized integer sizes based on table sizes:

```python
def get_reference_size(table_size):
    """Calculate optimal reference size"""
    if table_size < 256:
        return 1  # uint8
    elif table_size < 65536:
        return 2  # uint16
    elif table_size < 16777216:
        return 3  # uint24
    else:
        return 4  # uint32
```

### Path Compression

Paths can be delta-encoded:

```python
def encode_paths_delta(paths):
    """Delta-encode sorted paths"""
    encoded = []
    prev_path = ""
    
    for path in sorted(paths):
        # Find common prefix
        common = 0
        for i in range(min(len(prev_path), len(path))):
            if prev_path[i] == path[i]:
                common += 1
            else:
                break
        
        # Encode as (common_length, suffix)
        suffix = path[common:]
        encoded.append((common, suffix))
        prev_path = path
    
    return encoded
```

## Game-Specific Variations

### World of Warcraft

- Uses 9-byte EKeys
- Always includes CKeys
- Extensive use of patches

### Warcraft III Reforged

- Uses full 16-byte EKeys
- Simpler structure (no patches)
- Inline data for small files

### Call of Duty

- Custom flags for platform-specific content
- Advanced streaming hints
- Texture quality levels

## Implementation Example

```rust
use std::collections::HashMap;

pub struct TVFS {
    header: TVFSHeader,
    path_entries: Vec<PathEntry>,
    vfs_entries: Vec<VFSEntry>,
    cft_entries: Vec<CFTEntry>,
    path_index: HashMap<String, usize>,
}

impl TVFS {
    pub fn parse(data: &[u8]) -> Result<Self, TVFSError> {
        let header = TVFSHeader::parse(data)?;
        
        // Parse tables
        let path_table = Self::parse_path_table(
            data,
            header.path_table_offset as usize,
            header.path_table_size as usize,
        )?;
        
        let vfs_table = Self::parse_vfs_table(
            data,
            header.vfs_table_offset as usize,
            header.vfs_table_size as usize,
        )?;
        
        let cft_table = Self::parse_cft_table(
            data,
            header.cft_table_offset as usize,
            header.cft_table_size as usize,
        )?;
        
        // Build path index
        let mut path_index = HashMap::new();
        for (i, entry) in path_table.iter().enumerate() {
            path_index.insert(entry.path.clone(), i);
        }
        
        Ok(Self {
            header,
            path_entries: path_table,
            vfs_entries: vfs_table,
            cft_entries: cft_table,
            path_index,
        })
    }
    
    pub fn resolve_file(&self, path: &str) -> Option<FileInfo> {
        let normalized = if self.header.flags & TVFS_FLAG_LOWERCASE != 0 {
            path.to_lowercase()
        } else {
            path.to_string()
        };
        
        let index = *self.path_index.get(&normalized)?;
        let entry = &self.path_entries[index];
        
        match entry.entry_type {
            ENTRY_FILE => self.resolve_file_entry(entry),
            ENTRY_INLINE => self.resolve_inline_entry(entry),
            ENTRY_LINK => self.resolve_link_entry(entry),
            _ => None,
        }
    }
}
```

## Advantages Over Root Files

1. **Flexibility**: Supports multiple file representations
2. **Efficiency**: Optimized integer sizes
3. **Extensibility**: Version field allows format evolution
4. **Patches**: Native patch support
5. **Inline Data**: Small files stored directly
6. **Links**: Symbolic link support
7. **Streaming**: Better support for streaming scenarios

## Migration from Root Files

```python
def convert_root_to_tvfs(root_file):
    """Convert traditional root file to TVFS"""
    tvfs = TVFSBuilder()
    
    for entry in root_file.entries:
        # Add to path table
        path = lookup_path(entry.file_data_id)
        tvfs.add_file(path, entry.ckey, entry.locale_flags, entry.content_flags)
    
    return tvfs.build()
```

## Future Extensions

### Planned Features

- Compression hints
- Streaming priorities
- Platform-specific optimizations
- Directory metadata
- File attributes (permissions, timestamps)
- Incremental updates

### Version 2 Considerations

- SHA-256 hashes
- Better compression
- Cloud storage hints
- Delta encoding improvements
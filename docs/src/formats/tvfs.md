# TVFS (TACT Virtual File System)

TVFS is the virtual file system introduced in WoW 8.2 (CASC v3), providing a
unified
interface for managing content across multiple products and build
configurations. It replaces direct file path mappings with a more flexible
namespace-based system.

## How TVFS is Accessed

TVFS manifests are referenced through `vfs-*` fields in BuildConfig files:

1. **BuildConfig** contains `vfs-root` and numbered `vfs-1` through `vfs-N`
fields
2. Each VFS field contains two hashes: content key and encoding key
3. The encoding key (second hash) is used to fetch the TVFS manifest from CDN
4. The manifest is BLTE-encoded and must be decompressed
5. Once decoded, the manifest describes the virtual file system structure

Example from BuildConfig:

```text
vfs-root = fd2ea24073fcf282cc2a5410c1d0baef 14d8c981bb49ed169e8558c1c4a9b5e5
vfs-root-size = 50071 33487
```

Modern builds contain 1,500+ VFS entries for different product/region/platform
combinations.

## Overview

TVFS provides:

- Virtual file system abstraction

- Multi-product content management

- Namespace-based file organization

- Build-agnostic file references

- Content deduplication across products

## Architecture

### Namespace Hierarchy

```text
TVFS Root
├── Product Namespace (e.g., "wow")
│   ├── Build Namespace (e.g., "1.15.7.61582")
│   │   ├── Root Files
│   │   └── Content Trees
│   └── Shared Namespace
│       └── Common Assets
└── Global Namespace
    └── Cross-Product Assets
```

## File Structure

TVFS manifest is BLTE-encoded:

```text
[BLTE Container]
  [Header]
  [Namespace Definitions]
  [Directory Entries]
  [File Entries]
  [Content Mappings]
```

## Binary Format

Based on analysis of 5 TVFS samples from WoW builds 11.0.2.56313 through
11.2.0.62748.

### TVFS Header

```c
struct TvfsHeader {  // 38 bytes minimum, 46 with EST table
    uint8_t  magic[4];           // "TVFS" (0x54564653)
    uint8_t  format_version;     // Format version (1; agent accepts <= 1)
    uint8_t  header_size;        // Header size (not read by agent parser)
    uint8_t  ekey_size;          // EKey size (always 9)
    uint8_t  pkey_size;          // PKey size (always 9)
    uint32_t flags;              // Format flags (big-endian)
    uint32_t path_table_offset;  // Offset to path table (big-endian)
    uint32_t path_table_size;    // Size of path table (big-endian)
    uint32_t vfs_table_offset;   // Offset to VFS table (big-endian)
    uint32_t vfs_table_size;     // Size of VFS table (big-endian)
    uint32_t cft_table_offset;   // Offset to container file table (big-endian)
    uint32_t cft_table_size;     // Size of container file table (big-endian)
    uint16_t max_depth;          // Maximum path depth
    // Optional EST fields (only present if TVFS_FLAG_ENCODING_SPEC is set)
    uint32_t est_table_offset;   // Encoding spec table offset
    uint32_t est_table_size;     // Encoding spec table size
};
```

**Verified Header Properties:**

- Magic bytes: Always "TVFS" (0x54564653) in ASCII

- Format version: Always 1 across all samples

- Header size: 38 bytes minimum, 46 with EST table

- EKey size: 9 bytes (TACT standard)

- PKey size: 9 bytes (TACT standard)

- All multi-byte integer fields are big-endian (NGDP standard)

**Format Flags (Implementation Details):**

```rust
// TVFS format flags
const TVFS_FLAG_INCLUDE_CKEY: u32 = 0x01;      // Include content keys
const TVFS_FLAG_WRITE_SUPPORT: u32 = 0x02;     // Write support / EST present
const TVFS_FLAG_PATCH_SUPPORT: u32 = 0x04;     // Patch support enabled
```

- **Value 7 (0x7)**: Include C-key + Write support + Patch support (all
features)

- **EST Table Present**: When bit 1 (0x02) is set. The agent checks `flags &
  2` for encoding specifier presence.

- **Header Size**: 38 bytes minimum (without EST), 46 bytes with EST table
  fields

**Sample Analysis Results:**

- File sizes: 49,896 - 50,844 bytes (decompressed)

- All files use identical header format

- Table offsets and sizes are consistent with file structure

- Two retail builds (11.2.0.62706 and 11.2.0.62748) are byte-identical

### Table Structure

**Path Table** (PathTableOffset + PathTableSize):

- **Prefix Tree Structure**: Hierarchical path storage with shared prefixes

- **Node Types**: Directory nodes and file nodes with different flags

- **Path Components**: Each node contains a path component string

- **Child Linking**: Nodes reference child node indices for traversal

- **Maximum Depth**: Tracked in header for validation and optimization

**VFS Table** (VfsTableOffset + VfsTableSize):

- **File Spans**: Maps logical file ranges to physical container locations

- **Container Indices**: References into the container file table

- **Size Information**: File size and compressed size tracking

- **Compression Flags**: Indicates whether content is compressed

- **Directory Flags**: Distinguishes files from directories

**Container File Table** (CftTableOffset + CftTableSize):

- **EKeys**: 9-byte encoding keys for CDN content lookup

- **File Sizes**: Uncompressed file sizes

- **Compressed Sizes**: Optional compressed size information

- **Content Keys**: Optional 16-byte content keys (if INCLUDE_CKEY flag set)

- **Effective Size Calculation**: Handles cases where compressed size may be
absent

- **Variable-Width Size Encoding**: Size fields use 1-4 bytes based on the
  maximum value in the table. The agent determines width as: > 0xFFFFFF = 4
  bytes, > 0xFFFF = 3 bytes, > 0xFF = 2 bytes, else 1 byte. This applies to
  both EST table size fields and container file table size fields.

**Encoding Specifier Table** (Optional, if write support enabled):

- Contains encoding specifications for file creation

- Only present if flag bit 1 (0x02) is set

- Required for writing files to underlying storage

**Sample Table Sizes (Build 11.2.0.62748):**

```text
Path Table:      Offset 46,     Size 11,814 bytes
VFS Table:       Offset 41,527, Size 9,317 bytes
Container Table: Offset 11,882, Size 29,645 bytes
```

## Format Analysis Status

**Verified Information:**

- Header format and magic bytes confirmed

- Version consistency across builds established

- File size ranges and compression confirmed

- String patterns and hierarchical structure observed

**Requires Further Analysis:**

- Complete binary structure specification

- Entry format definitions

- Namespace and directory organization

- File reference mechanisms

- Cross-reference resolution with encoding files

## Content Resolution

### Path Resolution Algorithm

```rust
fn resolve_path(tvfs: &TVFS, path: &str) -> Option<TVFSFileEntry> {
    let parts: Vec<&str> = path.split('/').collect();

    // Start from root or current namespace
    let mut current_ns = tvfs.get_current_namespace();
    let mut current_dir = current_ns.root_directory_id;

    // Navigate path components
    for part in &parts[..parts.len() - 1] {
        if let Some(dir) = tvfs.find_subdirectory(current_dir, part) {
            current_dir = dir.directory_id;
        } else {
            return None;
        }
    }

    // Find file in final directory
    let filename = parts.last()?;
    tvfs.find_file(current_dir, filename)
}
```

### Content Key Mapping

```rust
struct ContentMapping {
    content_key: [u8; 16],
    encoding_key: [u8; 16],
    archive_info: Option<ArchiveLocation>,
}

impl TVFS {
    pub fn get_content(&self, file_id: u32) -> Option<ContentMapping> {
        let file = self.get_file(file_id)?;

        // Look up encoding key from content key
        let encoding_key = self.encoding_lookup(&file.content_key)?;

        // Find archive location if applicable
        let archive_info = self.find_archive_location(&encoding_key);

        Some(ContentMapping {
            content_key: file.content_key,
            encoding_key,
            archive_info,
        })
    }
}
```

## Virtual File Operations

### Directory Traversal

```rust
struct TVFSIterator {
    tvfs: Arc<TVFS>,
    stack: Vec<u32>,  // Directory ID stack
}

impl Iterator for TVFSIterator {
    type Item = TVFSEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(dir_id) = self.stack.pop() {
            let dir = self.tvfs.get_directory(dir_id)?;

            // Add subdirectories to stack
            for subdir in &dir.subdirectories {
                self.stack.push(subdir.directory_id);
            }

            // Return directory entry
            return Some(TVFSEntry::Directory(dir));
        }

        None
    }
}
```

### File Filtering

```rust
impl TVFS {
    pub fn find_files_by_extension(&self, ext: &str) -> Vec<TVFSFileEntry> {
        self.files
            .values()
            .filter(|f| f.name.ends_with(ext))
            .cloned()
            .collect()
    }

    pub fn find_files_by_pattern(&self, pattern: &Regex) -> Vec<TVFSFileEntry> {
        self.files
            .values()
            .filter(|f| pattern.is_match(&f.name))
            .cloned()
            .collect()
    }
}
```

## Multi-Product Support

### Product Isolation

```rust
struct MultiProductTVFS {
    products: HashMap<String, ProductNamespace>,
    shared: SharedNamespace,
}

impl MultiProductTVFS {
    pub fn get_product_files(&self, product: &str) -> Vec<TVFSFileEntry> {
        if let Some(ns) = self.products.get(product) {
            ns.enumerate_files()
        } else {
            Vec::new()
        }
    }

    pub fn get_shared_files(&self) -> Vec<TVFSFileEntry> {
        self.shared.enumerate_files()
    }
}
```

### Content Deduplication

```rust
struct DeduplicationIndex {
    content_map: HashMap<[u8; 16], Vec<FileReference>>,
}

impl DeduplicationIndex {
    pub fn find_duplicates(&self) -> Vec<([u8; 16], Vec<FileReference>)> {
        self.content_map
            .iter()
            .filter(|(_, refs)| refs.len() > 1)
            .map(|(key, refs)| (*key, refs.clone()))
            .collect()
    }
}
```

## Build Management

### Build Switching

```rust
impl TVFS {
    pub fn switch_build(&mut self, build_id: &str) -> Result<()> {
        // Save current build state
        self.save_current_state()?;

        // Load new build namespace
        let build_ns = self.load_build_namespace(build_id)?;

        // Update active namespace
        self.active_namespace = build_ns;

        // Refresh content mappings
        self.refresh_mappings()?;

        Ok(())
    }
}
```

### Patch Application

```rust
struct TVFSPatch {
    base_build: String,
    target_build: String,
    added_files: Vec<TVFSFileEntry>,
    modified_files: Vec<(u32, TVFSFileEntry)>,
    removed_files: Vec<u32>,
}

impl TVFS {
    pub fn apply_patch(&mut self, patch: TVFSPatch) -> Result<()> {
        // Remove deleted files
        for file_id in patch.removed_files {
            self.remove_file(file_id)?;
        }

        // Update modified files
        for (file_id, new_entry) in patch.modified_files {
            self.update_file(file_id, new_entry)?;
        }

        // Add new files
        for entry in patch.added_files {
            self.add_file(entry)?;
        }

        Ok(())
    }
}
```

## Implementation Example

```rust
struct TVFS {
    header: TVFSHeader,
    namespaces: HashMap<u32, TVFSNamespace>,
    directories: HashMap<u32, TVFSDirectory>,
    files: HashMap<u32, TVFSFileEntry>,
    active_namespace: u32,
}

impl TVFS {
    pub fn open_file(&self, path: &str) -> Result<Vec<u8>> {
        // Resolve path to file entry
        let file = self.resolve_path(path)
            .ok_or("File not found")?;

        // Get content mapping
        let mapping = self.get_content(file.file_id)
            .ok_or("Content not found")?;

        // Fetch and decompress content
        let compressed = self.fetch_content(&mapping.encoding_key)?;
        let decompressed = decompress_blte(compressed)?;

        Ok(decompressed)
    }
}
```

## Performance Optimization

### Caching Strategy

```rust
struct TVFSCache {
    path_cache: LruCache<String, u32>,      // Path -> FileID
    content_cache: LruCache<u32, Vec<u8>>,  // FileID -> Content
    metadata_cache: HashMap<u32, FileMetadata>,
}
```

### Lazy Loading

```rust
impl TVFS {
    pub fn lazy_load_directory(&mut self, dir_id: u32) -> Result<()> {
        if self.directories.contains_key(&dir_id) {
            return Ok(()); // Already loaded
        }

        // Load directory metadata on demand
        let dir_data = self.fetch_directory_data(dir_id)?;
        let directory = parse_directory(dir_data)?;

        self.directories.insert(dir_id, directory);
        Ok(())
    }
}
```

## Common Issues

1. **Namespace conflicts**: Multiple products using same paths
2. **Build inconsistencies**: Missing files between builds
3. **Permission management**: Access control across namespaces
4. **Cache invalidation**: Stale data after build switches
5. **Memory usage**: Large file trees consuming RAM

## Future Extensions

### Symbolic Links

Support for virtual symlinks:

```rust
struct TVFSSymlink {
    link_id: u32,
    target_path: String,
    namespace_id: u32,
}
```

### Metadata Extensions

Additional file metadata:

```rust
struct ExtendedMetadata {
    created_time: u64,
    modified_time: u64,
    attributes: u32,
    permissions: u32,
    custom_data: HashMap<String, String>,
}
```

## References

- See [Root File](root.md) for legacy file mapping

- See [Encoding Documentation](encoding.md) for content resolution

- See [Archives](archives.md) for storage details

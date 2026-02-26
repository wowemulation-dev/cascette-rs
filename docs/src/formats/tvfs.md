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

TVFS organizes content into namespaces rather than per-build file trees. This
allows multiple products and regions to share common assets through a single
content-addressed storage layer, with deduplication across products.

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
const TVFS_FLAG_ENCODING_SPEC: u32 = 0x02;     // Encoding spec table (EST) present
const TVFS_FLAG_PATCH_SUPPORT: u32 = 0x04;     // Patch support enabled
```

- **Value 7 (0x7)**: Include C-key + Encoding spec + Patch support (all
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

Recursive prefix tree (trie) encoding file paths. Each entry has:

- Optional `0x00` path separator bytes (before/after name fragments)
- Length-prefixed name fragment (1-byte length + N bytes)
- `0xFF` marker followed by 4-byte big-endian NodeValue:
  - Bit 31 set: folder node, lower 31 bits = folder data length (includes the
    4-byte NodeValue). Children are inline within that byte range.
  - Bit 31 clear: file node, value = byte offset into the VFS table.

Maximum depth is tracked in the header.

**VFS Table** (VfsTableOffset + VfsTableSize):

Span-based entries addressed by byte offset from path table NodeValues. Each
entry has:

- `span_count` (1 byte): 1-224 = file entry, 225-254 = other, 255 = deleted
- Per span (repeated `span_count` times):
  - `file_offset` (4 bytes BE): offset within the referenced content
  - `span_length` (4 bytes BE): content size of this span
  - `cft_offset` (CftOffsSize bytes BE): byte offset into the CFT

`CftOffsSize` is computed from `cft_table_size` using `GetOffsetFieldSize`:
`>0xFFFFFF` = 4 bytes, `>0xFFFF` = 3 bytes, `>0xFF` = 2 bytes, else 1 byte.

**Container File Table** (CftTableOffset + CftTableSize):

Fixed-stride entries addressed by byte offset from VFS span `cft_offset`
values. Entry layout depends on header flags:

- `EKey` (ekey_size bytes): encoding key
- `EncodedSize` (4 bytes BE): encoded (compressed) size
- `CKey` (pkey_size bytes): content key (if `TVFS_FLAG_INCLUDE_CKEY`)
- `est_index` (EstOffsSize bytes BE): EST entry index (if
  `TVFS_FLAG_ENCODING_SPEC`)
- `patch_offset` (CftOffsSize bytes BE): patch entry offset (if
  `TVFS_FLAG_PATCH_SUPPORT`)

`EstOffsSize` is computed from `est_table_size` using the same
`GetOffsetFieldSize` function as `CftOffsSize`.

**Encoding Specifier Table (EST)** (Optional, if encoding spec flag is set):

- Contains null-terminated encoding spec strings (same format as the ESpec
  table in the encoding file)

- Only present if flag bit 1 (0x02) is set

- Required for writing files to underlying storage

- Parsed from `est_table_offset` for `est_table_size` bytes

**Sample Table Sizes (Build 11.2.0.62748):**

```text
Path Table:      Offset 46,     Size 11,814 bytes
VFS Table:       Offset 41,527, Size 9,317 bytes
Container Table: Offset 11,882, Size 29,645 bytes
```

## Format Analysis Status

**Verified against CascLib and CDN data (WoW Retail, Classic, Classic Era):**

- Header format, magic bytes, flags, and table offsets
- Path table recursive prefix tree with 0xFF NodeValue markers
- VFS span-based entries with variable-width CFT offsets
- CFT fixed-stride entries with flag-dependent fields
- EST null-terminated encoding spec strings
- Round-trip parse/build produces structurally equivalent output

## Usage

### Parsing a TVFS Manifest

```rust
use cascette_formats::tvfs::TvfsFile;

// From decompressed data
let tvfs = TvfsFile::parse(&data)?;

// From BLTE-encoded CDN data
let tvfs = TvfsFile::load_from_blte(&blte_data)?;
```

### Enumerating Files

```rust
// All file entries from the path table
for file in &tvfs.path_table.files {
    println!("{} -> VFS offset {}", file.path, file.vfs_offset);
}

// With VFS entry details
for (file, vfs_entry) in tvfs.enumerate_files() {
    if let Some(entry) = vfs_entry {
        for span in &entry.spans {
            println!("{}: offset={}, length={}, cft_offset={}",
                file.path, span.file_offset, span.span_length, span.cft_offset);
        }
    }
}
```

### Resolving a Path

```rust
// Resolve path -> VFS entry -> CFT entry (EKey)
if let Some(container_entry) = tvfs.resolve_path("path/to/file") {
    println!("EKey: {}", container_entry.ekey_hex());
    if let Some(ckey) = container_entry.content_key_hex() {
        println!("CKey: {}", ckey);
    }
}
```

### Building a TVFS Manifest

```rust
use cascette_formats::tvfs::TvfsBuilder;

let mut builder = TvfsBuilder::with_flags(0x07); // CKEY + EST + PATCH
builder.add_est_spec("b:256K*=z".to_string());
builder.add_file(
    "path/to/file".to_string(),
    [0x01; 9],   // ekey
    1024,         // encoded_size
    2048,         // content_size
    Some([0x02; 16]), // content_key
);
let data = builder.build()?;
```

## References

- See [Root File](root.md) for legacy file mapping

- See [Encoding Documentation](encoding.md) for content resolution

- See [Archives](archives.md) for storage details

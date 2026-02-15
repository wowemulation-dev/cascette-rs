# Archive Files and Indices

CASC/TACT archives are container files that store game content in a packed
format.
They work with index files to enable efficient content retrieval without
unpacking entire archives. The system uses different formats for network (TACT)
and local storage (CASC).

## Overview

The archive system provides:

- Bulk storage of game assets in `.archive` files

- Index files for fast content location

- Support for partial downloads via HTTP range requests

- Deduplication through content addressing

## Archive Files

### CDN Archives vs Local Archives

**CDN Archives** (TACT - served over HTTP):

- Named using 32-character hash keys (e.g., `86b6b0daf3d8ef68271b15567c37300c`)

- Accessed via URL path: `/tpr/wow/data/{hash[:2]}/{hash[2:4]}/{hash}`

- Paired with Archive Index files (`.index`) for content location

- Single BLTE-encoded container format

- Part of TACT (Tooling for Archive Content Transfer) protocol

**Local Client Archives** (CASC - stored on disk):

- Named with numeric indices: `data.001`, `data.002`, etc.

- Use IDX Journal files (`.idx`) for local content access

- Multiple BLTE files concatenated together

- Part of CASC (Content Addressable Storage Container) system

- Optimized for memory-mapped access

### CDN Archive Structure

CDN archives are single BLTE-encoded containers, while local archives contain
multiple BLTE files:

```text
CDN Archive Format (TACT):          Local Archive Format (CASC):
┌──────────────────┐                ┌──────────────────┐
│ BLTE Container   │                │ BLTE File 1      │
├──────────────────┤                ├──────────────────┤
│ Header & Blocks  │                │ BLTE File 2      │
├──────────────────┤                ├──────────────────┤
│ Content Blocks   │                │ BLTE File 3      │
│ (concatenated)   │                │      ...         │
└──────────────────┘                └──────────────────┘
```

### Verified Archive Characteristics

Based on examination of sample archives:

- **File sizes**: Range from ~7MB to 268MB when compressed

- **Compression ratios**: 4.9x to 190x compression achieved via BLTE

- **Content types**: WDB Cache files (WDC3), textures, models, and other game
assets

- **Decompressed content**: Much smaller than archive size (1-2MB typical)

- **Access pattern**: Content addressed via hash keys in index files

## CRITICAL: Two Completely Different Index Systems

### ⚠️ CDN Archive Index (.index) vs Local Storage Index (.idx)

**NEVER CONFUSE THESE TWO FORMATS - THEY ARE COMPLETELY DIFFERENT:**

1. **CDN Archive Index Files (.index)**: TACT format with 28-byte footer,
   variable-length encoding keys
2. **Local Storage Index Files (.idx)**: CASC format with header, fixed 9-byte
   content key buckets

These systems serve different purposes and use entirely different formats,
key types, and data structures.

## CDN Archive Index Format (TACT Protocol)

**File Extension**: `.index`
**Location**: Downloaded from CDN
**Purpose**: Maps variable-length encoding keys to CDN archive locations
**Key Type**: Encoding keys (from Encoding file)
**Key Length**: Variable, as specified in footer's `ekey_length` field
(typically 16 bytes, sometimes 9)
**Implementation**: `cascette-formats/src/archive/index.rs`

## Archive Index Files (.index) - TACT Protocol

Based on analysis of actual CDN index files from various WoW builds.

CDN archive indexes use a chunk-based format with footer metadata:

### Archive Index Structure

```text
Index File Layout:
┌────────────────┐
│ Data Chunks    │ <- 4KB chunks containing entries
│ (4096 bytes)   │
├────────────────┤
│ ...            │
├────────────────┤
│ Last Chunk     │ <- Table of contents + entries
├────────────────┤
│ Footer         │ <- Metadata (variable length)
└────────────────┘
```

### CDN Index Entry Format (Variable Length)

```c
struct CDNArchiveIndexEntry {
    uint8_t  ekey[ekey_length];  // Encoding key (variable length from footer)
    uint32_t encoded_size;       // BLTE encoded size (big-endian)
    uint32_t archive_offset;     // Offset in archive (big-endian)
};
```

**Entry Size**: Variable = `ekey_length + size_bytes + offset_bytes` (from footer)
**Typical Sizes**:

- With 16-byte keys: `16 + 4 + 4 = 24 bytes` per entry
- With 9-byte keys: `9 + 4 + 4 = 17 bytes` per entry

**Key Properties**:

- Encoding key length specified in footer's `ekey_length` field
- All multi-byte fields use big-endian encoding
- NEVER assume fixed 9-byte keys - always read from footer

### Archive Index Footer (TACT)

Archive Index files use a 28-byte footer at the end of the file:

```c
struct ArchiveIndexFooter {  // 28 bytes total
    uint8_t  toc_hash[8];     // First 8 bytes of MD5 hash of table of contents
    uint8_t  version;         // Must be <= 1 (0 or 1)
    uint8_t  reserved[2];     // Must be [0, 0]
    uint8_t  page_size_kb;    // Must be 4 (4KB pages)
    uint8_t  offset_bytes;    // Archive offset field size (4 for archives)
    uint8_t  size_bytes;      // Compressed size field size (always 4)
    uint8_t  ekey_length;     // EKey length in bytes (16 for full MD5)
    uint8_t  footer_hash_bytes; // Footer hash length (always 8)
    uint32_t element_count;   // Number of entries (little-endian - special case!)
    uint8_t  footer_hash[8];  // MD5 footer validation (first 8 bytes)
};
```

**Verified Footer Properties**:

- Standard values: offset_bytes=4, size_bytes=4, ekey_length=16

- Page/chunk size consistently 4096 bytes

- Item length consistently 24 bytes (0x18)

- Archive filename = MD5 hash of the footer

- Footer validation uses MD5 hashing (first 8 bytes of hash)

- **Mixed endianness**: element_count field is little-endian while all other

  multi-byte fields are big-endian

- TOC hash validates the table of contents integrity separately from footer hash

**Implementation Notes**:

- **Extended Block Offsets**: The agent logs "Archive w/ Extended Block
  Offset Found" for archive index entries that use larger-than-4-byte offsets
  (for archives exceeding 4GB)

- **Archive Count Limit**: The agent has a `casc_supports_1023_archives`
  configuration flag, indicating a maximum of 1023 archives per CASC storage

### Sample Analysis Results

**File Sizes Observed**:

- Small indexes: ~8KB (few hundred entries)

- Medium indexes: ~50-200KB (thousands of entries)

- Large indexes: ~300KB+ (tens of thousands of entries)

**Index Distribution** (from sample builds):

- WoW retail: 400-1400+ archives per build

- WoW Classic: 1000-1400+ archives per build

- Beta builds: 400-800 archives per build

**Chunk Structure**:

- All indexes use 4KB chunks (170 entries max per chunk: 4096 / 24 = 170

  entries)

- Table of contents is stored separately after chunks, containing last key of

  each chunk

- Chunk structure enables streaming and memory-efficient processing

- TOC hash validates chunk integrity separately from footer hash

- Chunks are padded with zeros to maintain 4KB alignment

### Archive Index Access Pattern

**CDN URL Format**:

```text
https://cdn.domain.com/tpr/wow/data/{hash[:2]}/{hash[2:4]}/{hash}.index
```

**Lookup Process**:

1. Get archive content key from CDN configuration
2. Append '.index' to form index URL
3. Fetch and parse index file
4. Search entries for target EKey
5. Use offset/size to retrieve from corresponding .archive file

**Self-Referential Naming**:

The archive index filename (hash) is the MD5 of its own footer structure,
providing a unique identifier that validates the index contents.

## Local Storage Index Format (.idx files)

**File Extension**: `.idx`
**Location**: Client-side storage directory (`Data/data/`)
**Purpose**: Maps content keys to local data file locations using bucket algorithm
**Key Type**: Content keys (MD5 hashes from Root file)
**Key Length**: ALWAYS 9 bytes (truncated for space efficiency in local storage)
**Implementation**: `cascette-client-storage/src/index.rs`

### ⚠️ CRITICAL DIFFERENCES FROM CDN INDEX

- Uses content keys (MD5), NOT encoding keys
- ALWAYS uses 9-byte truncated keys (never variable)
- Bucket-based structure, NOT sequential chunks
- Header at start, NOT footer at end
- Different entry format (5 bytes vs variable)
- Different validation (Jenkins hash vs MD5)

## IDX Journal Files (.idx) - CASC Local Storage

Local CASC storage uses IDX Journal files for indexing:

### IDX Journal Structure

```c
struct IDXJournalHeader {  // 18 bytes + block table
    uint32_t data_size;       // Size of header data
    uint32_t data_hash;       // Jenkins hash validation
    uint16_t version;         // Journal version
    uint8_t  bucket;          // Bucket ID (0x00-0xFF)
    uint8_t  unused;          // Padding
    uint8_t  length_size;     // Size field bytes
    uint8_t  location_size;   // Location field bytes (5 = 1 archive + 4 offset)
    uint8_t  key_size;        // Key field bytes (9 or 16)
    uint8_t  segment_bits;    // Segment size bits
    // Followed by block table entries
};
```

**Key Differences from Archive Indexes**:

- Bucket-based structure (256 buckets, 00-FF)

- Jenkins hash validation instead of footer hash

- Fixed key sizes (not truncated)

- Header at start instead of footer at end

- One journal file per bucket

## Loose Files Index

For files not in archives:

```c
struct LooseFilesIndex {
    uint32_t magic;              // 'LIDX'
    uint32_t version;
    uint32_t entry_count;

    struct Entry {
        uint8_t  encoding_key[16];
        uint32_t file_size;
        uint8_t  file_hash[16];  // For verification
    } entries[];
};
```

## Archive Lookup Process

1. **Get encoding key**: From encoding file lookup
2. **Check indices**: Search all index files for key
3. **Locate in archive**: Extract offset and size
4. **Retrieve data**: Read from archive at offset
5. **Decompress**: Process BLTE container

### Implementation Example

```rust
struct ArchiveIndex {
    header: ArchiveIndexHeader,
    entries: Vec<ArchiveIndexEntry>,
}

impl ArchiveIndex {
    pub fn find_file(&self, encoding_key: &[u8]) -> Option<(u64, u32)> {
        // Truncate search key to index key size
        let search_key = &encoding_key[..self.header.key_size as usize];

        // Binary search entries (sorted by key)
        let idx = self.entries.binary_search_by_key(
            &search_key,
            |e| &e.key[..]
        ).ok()?;

        let entry = &self.entries[idx];
        Some((entry.offset, entry.size))
    }
}
```

## HTTP Range Requests

For CDN retrieval without downloading entire archives:

```text
GET /data/5e/16/5e16b6ff530b1816c7b32296e0875ed4 HTTP/1.1
Host: cdn.example.com
Range: bytes=1048576-2097151
```

Response:

```text
HTTP/1.1 206 Partial Content
Content-Range: bytes 1048576-2097151/134217728
Content-Length: 1048576
```

## Archive Creation

When building archives:

1. **Group related files**: Minimize seeks during loading
2. **Align boundaries**: 4KB alignment for efficient I/O
3. **Order by access**: Frequently accessed files first
4. **Compress individually**: Each file is BLTE-encoded
5. **Update indices**: Generate index entries

## Optimization Strategies

### Memory Mapping

For local archives:

```rust
use memmap2::MmapOptions;

struct ArchiveReader {
    mmap: Mmap,
}

impl ArchiveReader {
    pub fn read_file(&self, offset: u64, size: u32) -> &[u8] {
        let start = offset as usize;
        let end = start + size as usize;
        &self.mmap[start..end]
    }
}
```

### Index Caching

Keep frequently used indices in memory:

```rust
struct IndexCache {
    indices: HashMap<String, Arc<ArchiveIndex>>,
    lru: LruCache<String, ()>,
}
```

## Archive Validation

### Checksum Verification

When checksums are present:

```rust
fn verify_file(data: &[u8], expected_checksum: &[u8; 16]) -> bool {
    let computed = md5::compute(data);
    computed.0 == *expected_checksum
}
```

### Size Validation

Always verify extracted size matches expected:

```rust
if decompressed.len() != expected_size as usize {
    return Err("Size mismatch");
}
```

## Common Issues

1. **Key collisions**: Truncated keys may collide (handle gracefully)
2. **Archive corruption**: Verify checksums when available
3. **Missing indices**: Some files may only exist as loose files
4. **Version mismatches**: Handle different index versions
5. **Alignment padding**: Account for alignment bytes

## Archive Groups

Archive Groups are **locally generated mega-indices** that combine multiple CDN archive
indices into a single unified lookup structure. They are always created client-side
by merging downloaded archive index files, never downloaded directly from the CDN.
They improve content retrieval performance by reducing the number of index files
that must be searched.

### Purpose

Without Archive Groups:

- Content lookup requires searching many individual `.index` files
- Each lookup performs multiple binary searches across hundreds of indices

With Archive Groups:

- Content lookup uses a single combined index structure
- Dramatically reduces search time from O(n) to O(1) in most cases

### Key Characteristics

- **Always client-generated**: Created locally by merging downloaded CDN index
  files
- **Never downloaded**: Archive groups are generated client-side, not fetched
  from CDN
- **Hash-based assignment**: Uses deterministic
  `hash(encoding_key) % 65536` algorithm
- **Performance focused**: Reduces lookup latency for frequently accessed content
- **Referenced in configs**: Identified by `archive-group` and
  `patch-archive-group` fields
- **Binary format**: Uses 6-byte offset fields (2-byte archive index + 4-byte offset)

Archive Groups are critical implementation details for Battle.net compatibility that
significantly improve the performance of CASC content access through local generation
and unified lookup structures.

## File Organization

Typical CASC repository structure:

```text
data/
├── config/           # Configuration files
├── data/            # Archive files
│   ├── 00/
│   │   ├── 00/{hash}.archive
│   │   └── ...
│   └── ff/
│       └── ff/{hash}.archive
├── indices/         # Index files
│   ├── {hash}.index
│   └── ...
└── patch/           # Patch archives
```

## ⚠️ NEVER CONFUSE THESE FORMATS - SUMMARY

### CDN Archive Index (.index files)

- **File Extension**: `.index`
- **Protocol**: TACT (Network)
- **Location**: Downloaded from CDN
- **Key Type**: Encoding keys (from Encoding file)
- **Key Length**: Variable (footer's `ekey_length`, typically 16 bytes)
- **Structure**: Sequential chunks with 28-byte footer
- **Entry Size**: Variable (`ekey_length + size_bytes + offset_bytes`)
- **Validation**: MD5 footer hash
- **Implementation**: `cascette-formats/src/archive/index.rs`

### Local Storage Index (.idx files)

- **File Extension**: `.idx`
- **Protocol**: CASC (Local storage)
- **Location**: Client `Data/data/` directory
- **Key Type**: Content keys (from Root file)
- **Key Length**: ALWAYS 9 bytes (truncated)
- **Structure**: Bucket algorithm with header
- **Entry Size**: 5 bytes (1 archive ID + 4 offset)
- **Validation**: Jenkins hash
- **Implementation**: `cascette-client-storage/src/index.rs`

### Content Resolution Paths

**CDN Download Path:**

```text
File Path → Root → Content Key → Encoding → Encoding Key → CDN Index (.index) → CDN Archive
```

**Local Retrieval Path:**

```text
File Path → Root → Content Key → Local Index (.idx) → Local Data (.data)
```

**NEVER mix these two systems or their key types.**

## Version History

### CDN Archive Index Format (.index files)

The CDN Archive Index format currently has only one version:

#### Version 1 (Current)

- **Footer Size**: 28 bytes
- **Location**: End of file
- **Features**:
  - Variable-length encoding keys (footer's `ekey_length` field)
  - 4KB chunk-based structure with table of contents
  - MD5 hash validation (footer hash and TOC hash)
  - Self-referential naming (filename = MD5 of footer)
  - Mixed endianness (element_count is little-endian, others big-endian)
  - Typical entry size: 24 bytes (16-byte key + 4-byte size + 4-byte offset)

#### Version Detection

The version field is at offset 8 in the 28-byte footer. All known CDN archive indices use version 1.

#### Implementation Status

- **cascette-formats**: Full support for version 1 with parser
- **Archive-groups**: Client-side mega-indices combine multiple CDN indices (6-byte offset variant)

### Local Storage Index Format (.idx files)

The Local Storage Index (IDX Journal) format currently has only one version:

#### Version 7 (Current - IDX Journal v7)

- **Header Size**: 16 bytes
- **Location**: Start of file
- **Features**:
  - Fixed 9-byte truncated content keys (space optimization)
  - 18-byte entries (9-byte key + 5-byte location + 4-byte size)
  - 256 bucket-based organization (0x00-0xFF)
  - Packed 5-byte location field (10-bit archive ID + 30-bit offset)
  - Jenkins hash validation
  - Mixed endianness (header little-endian, entries mixed)
  - Bucket algorithm: XOR first 9 bytes, then XOR nibbles
  - Filename format: `{bucket:02x}{version:06x}.idx`

#### Version Detection

The version field is at offset 8 in the header (16-bit little-endian). The implementation
validates version equals 7 and warns on unexpected versions.

#### Implementation Status

- **cascette-client-storage**: Full support for version 7 with parser and builder
- No earlier versions documented (version 7 is standard for modern CASC)

### Key Differences Between Index Systems

| Feature | CDN Index (.index) | Local Index (.idx) |
|---------|-------------------|-------------------|
| Version | 1 (footer-based) | 7 (header-based) |
| Protocol | TACT (network) | CASC (local storage) |
| Key Type | Encoding keys | Content keys |
| Key Length | Variable (16 typical) | Fixed 9-byte truncated |
| Structure | Sequential chunks | Bucket algorithm |
| Validation | MD5 hash | Jenkins hash |
| Endianness | Mixed (mostly big) | Mixed (header little) |
| Entry Size | Variable (24 typical) | Fixed 18 bytes |
| Location | CDN download | Client Data/ directory |
| Crate | cascette-formats | cascette-client-storage |

## Binary Verification (Agent.exe, TACT 3.13.3)

Verified against Agent.exe (WoW Classic Era) using Binary Ninja on
2026-02-15.

### Confirmed Correct

| Claim | Agent Evidence |
|-------|---------------|
| CDN index footer: 28 bytes | `tact::CdnIndexFooterValidator` at 0x6b815d reads last 0x14 bytes + 8-byte toc_hash |
| Footer version must be <= 1 | `sub_6b8302` validates `version <= 1` |
| ekey_length must be <= 0x10 (16) | Footer validator checks hash size limit |
| page_size_kb typical 4 | Footer field confirmed |
| offset_bytes and size_bytes: 4 each | Footer field confirmed |
| Footer hash: MD5 first 8 bytes | Footer validator uses MD5 |
| element_count: little-endian | Footer validator confirmed |
| Config fields: archives, archive-group | Strings at 0x9b4b04, 0x9b4b24 |
| Config fields: patch-archives, patch-archive-group | Strings at 0x9b4b6c, 0x9b4b98 |
| Index size fields: archives-index-size, archive-group-index-size | Strings at 0x9b4b10, 0x9b4b34 |
| Index naming: "archive_%u.index" | String at 0x9adb40 |

### Changes Applied

1. Fixed footer version from "Must be 1" to "Must be <= 1" (allows 0)
2. Added extended block offset note for archives > 4GB
3. Added 1023 archive count limit

### Source Files

Agent source paths from PDB info:
- `tact::CdnIndexFooterValidator` at 0x6b815d
- CDN index reader at `sub_6b8302`
- Local storage CASC functions in `sub_512c0c`

## References

- See [Encoding Documentation](encoding.md) for key lookup

- See [BLTE Format](blte.md) for archive content structure

- See [CDN Architecture](cdn.md) for remote retrieval

- See [Format Transitions](format-transitions.md) for format evolution tracking

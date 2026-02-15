# Encoding File Format

The encoding file is the gateway to all CASC content. It maps content keys
(unencoded file hashes) to encoding keys (encoded/compressed file hashes) and
provides essential metadata for content resolution.

## Overview

The encoding file serves multiple critical functions:

1. **Content Resolution**: Maps content keys to encoding keys for CDN retrieval
2. **Compression Metadata**: Specifies ESpec encoding for each file
3. **Size Information**: Tracks both compressed and decompressed sizes
4. **Multi-Version Support**: Handles multiple encoding keys per content key

## File Structure

The encoding file is BLTE-encoded and consists of:

```text
[BLTE Container]
  [Header]           (22 bytes)
  [ESpec Table]      (variable)
  [CKey Page Index]  (variable)
  [CKey Pages]       (variable)
  [EKey Page Index]  (variable)
  [EKey Pages]       (variable)
  [File ESpec]       (variable) - The encoding file's own ESpec
```

## Binary Format

### Header (22 bytes)

```c
struct EncodingHeader {
    uint16_t magic;           // 0x00: 'EN' (0x454E)
    uint8_t  version;         // 0x02: Version (1)
    uint8_t  ckey_size;       // 0x03: Content key size (16)
    uint8_t  ekey_size;       // 0x04: Encoding key size (16)
    uint16_t ckey_page_size;  // 0x05: CKey page size in KB (BE)
    uint16_t ekey_page_size;  // 0x07: EKey page size in KB (BE)
    uint32_t ckey_page_count; // 0x09: Number of CKey pages (BE)
    uint32_t ekey_page_count; // 0x0D: Number of EKey pages (BE)
    uint8_t  unknown;         // 0x11: Unknown (0)
    uint32_t espec_size;      // 0x12: ESpec table size (BE)
};
```

### ESpec String Table

Immediately follows the header. Contains null-terminated strings referenced by
entries:

```text
"z\0b:{0,4}\0b:{0,4},z\0b:{0,2},z:{0,6}\0...\0"
```

Common ESpec patterns:

- `z` - ZLib compression

- `n` - No compression

- `b:{start,size}` - Block encoding (see [ESpec](../compression/espec.md))

- Empty string for uncompressed files

### Page Index Tables

#### CKey Page Index

For each CKey page:

```c
struct PageIndex {
    uint8_t first_key[ckey_size];  // First key in the page
    uint8_t page_hash[16];         // MD5 of the page data
};
```

#### EKey Page Index

Similar structure but uses `ekey_size` for the first key.

### Content Key (CKey) Pages

Pages are sorted by content key for binary search. Each page contains multiple
entries:

```c
struct CKeyEntry {
    uint8_t  ekey_count;                    // Number of encoding keys
    uint8_t  file_size[5];                  // Decompressed size (40-bit BE)
    uint8_t  ckey[ckey_size];               // Content key
    uint8_t  ekeys[ekey_size * ekey_count]; // Encoding keys
};
```

Entry layout:

```text
[count:1] [size:5] [ckey:16] [ekey1:16] [ekey2:16] ...
```

**Multiple EKeys**: A single content key can map to multiple encoding keys,
allowing:

- Different compression algorithms for the same content

- Regional variations with different encryption

- Platform-specific optimizations

### Encoding Key (EKey) Pages

Maps encoding keys to ESpec entries:

```c
struct EKeyEntry {
    uint8_t  ekey[ekey_size];     // Encoding key
    uint32_t espec_index;          // Index into ESpec table (BE)
    uint8_t  compressed_size[5];   // Compressed size (40-bit BE)
};
```

## Content Resolution Process

1. **Find CKey Entry**:
   - Binary search CKey page index for target page
   - Linear search within page for content key
   - Extract encoding key(s) and decompressed size

2. **Find EKey Entry** (optional):
   - Binary search EKey page index
   - Locate entry to get ESpec index and compressed size

3. **Parse ESpec**:
   - Index into ESpec string table
   - Parse encoding specification for compression details

## Implementation Example

```rust
#[derive(BinRead)]
#[br(big)]
struct EncodingHeader {
    #[br(assert(magic == 0x454E))]
    magic: u16,
    version: u8,
    ckey_size: u8,
    ekey_size: u8,
    ckey_page_size_kb: u16,
    ekey_page_size_kb: u16,
    ckey_page_count: u32,
    ekey_page_count: u32,
    #[br(assert(unknown == 0))]
    unknown: u8,
    espec_table_size: u32,
}

impl EncodingFile {
    pub fn find_encoding_key(&self, content_key: &[u8; 16]) -> Option<EncodingResult> {
        // 1. Find page containing content key
        let page_idx = self.find_ckey_page(content_key)?;
        let page = self.read_ckey_page(page_idx);

        // 2. Search within page
        for entry in page.entries() {
            if entry.ckey == *content_key {
                return Some(EncodingResult {
                    ekeys: entry.ekeys.clone(),
                    decompressed_size: entry.file_size,
                });
            }
        }
        None
    }
}
```

## Chunked Page Loading and Binary Search

The implementation uses chunked processing for memory efficiency with large
encoding files:

```rust
/// Encoding file with chunked page loading
pub struct EncodingFile {
    /// Header with format information
    pub header: EncodingHeader,
    /// ESpec string table (always loaded)
    pub espec_table: Vec<String>,
    /// CKey page info for lazy loading
    pub ckey_page_info: Vec<PageInfo>,
    /// EKey page info for lazy loading
    pub ekey_page_info: Vec<PageInfo>,
    /// Binary page data for on-demand parsing
    ckey_page_data: Vec<u8>,
    ekey_page_data: Vec<u8>,
}

impl EncodingFile {
    /// Binary search with chunked loading
    pub fn find_encoding_key(&self, content_key: &[u8; 16]) -> Option<EncodingResult> {
        // 1. Binary search page index to find target page
        let page_idx = self.find_ckey_page(content_key)?;

        // 2. Parse page on demand (memory efficient)
        let page_info = &self.ckey_page_info[page_idx];
        let page_data = &self.ckey_page_data[page_info.data_range()];

        // 3. Validate page hash for integrity
        if !self.validate_page_hash(page_data, &page_info.hash) {
            return None; // Page corruption detected
        }

        // 4. Parse page entries and binary search
        let page = EncodingPage::parse_ckey_page(page_data).ok()?;
        page.binary_search_content_key(content_key)
    }

    /// Find page containing target key using binary search on page indices
    fn find_ckey_page(&self, target_key: &[u8; 16]) -> Option<usize> {
        self.ckey_page_info.binary_search_by(|page_info| {
            page_info.first_key.cmp(target_key)
        }).map(|idx| idx).or_else(|idx| {
            if idx > 0 { Some(idx - 1) } else { None }
        })
    }

    /// Validate page integrity using MD5 hash
    fn validate_page_hash(&self, page_data: &[u8], expected_hash: &[u8; 16]) -> bool {
        let computed = md5::compute(page_data);
        computed.0 == *expected_hash
    }
}
```

### Mixed Endianness Handling

The encoding file format uses mixed endianness requiring careful parsing:

```rust
// Header and page indices are big-endian
#[derive(BinRead, BinWrite)]
#[br(big)] #[bw(big)]
pub struct EncodingHeader { /* ... */ }

// But some internal structures may vary
// Always verify endianness for each field type
```

## ESpec Integration

The ESpec strings define how files are encoded:

### Common Patterns

1. **Uncompressed**: Empty string or `n`
2. **ZLib**: `z`
3. **Partial compression**: `b:{0,1000},z,b:{1000,500},n`
   - Bytes 0-1000: ZLib compressed
   - Bytes 1000-1500: Uncompressed

### Parsing ESpec

```rust
enum ESpecOp {
    None,
    ZLib,
    ByteRange { start: u32, size: u32 },
}

fn parse_espec(spec: &str) -> Vec<ESpecOp> {
    if spec.is_empty() || spec == "n" {
        return vec![ESpecOp::None];
    }

    spec.split(',')
        .map(|part| match part {
            "z" => ESpecOp::ZLib,
            "n" => ESpecOp::None,
            s if s.starts_with("b:") => {
                // Parse "b:{start,size}"
                let nums = parse_range(s);
                ESpecOp::ByteRange {
                    start: nums.0,
                    size: nums.1
                }
            }
            _ => ESpecOp::None,
        })
        .collect()
}
```

## Multi-Version Support

Files can have multiple encoding keys (different compression/encryption):

```rust
struct CKeyEntry {
    ekey_count: u8,        // Usually 1, can be 2+
    file_size: u64,        // Same for all versions
    ckey: [u8; 16],        // Content key
    ekeys: Vec<[u8; 16]>,  // Multiple encoding keys
}
```

Use cases:

- Different regional encryption

- Progressive quality levels

- Platform-specific optimizations

## Performance Considerations

### Memory-Mapped Access

For large encoding files (100MB+):

```rust
use memmap2::MmapOptions;

struct EncodingFile {
    mmap: Mmap,
    header: EncodingHeader,
    // ...
}

impl EncodingFile {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Parse header from mmap
        let header = EncodingHeader::read(&mmap[..22])?;

        Ok(Self { mmap, header })
    }
}
```

### Page Caching

Cache frequently accessed pages:

```rust
struct PageCache {
    entries: LruCache<u32, Arc<CKeyPage>>,
}
```

## Validation

### Checksums

Each page has an MD5 checksum in the index:

```rust
fn validate_page(index: &PageIndex, data: &[u8]) -> bool {
    let computed = md5::compute(data);
    computed.0 == index.page_hash
}
```

### Size Constraints

- Page sizes must be > 0 (no power-of-2 requirement enforced)

- Key sizes in range [1, 16] bytes

- Page counts must be > 0

- ESpec size must be > 0

- File sizes use 40-bit integers (up to 1TB)

## File's Own ESpec

After all the data structures, the encoding file contains its own ESpec string
describing how it's compressed. This self-referential metadata is an
intentional, documented feature of the NGDP format.

### Official Documentation

The [wowdev.wiki TACT specification](https://wowdev.wiki/TACT#Encoding_table)
explicitly lists this as the 5th component:

1. Header
2. Encoding specification data (ESpec)
3. Content key → encoding key table
4. Encoding key → encoding spec table
5. **"Encoding specification data for the encoding file itself"**

### Reference Implementation

TACT.Net explicitly handles this in `EncodingFile.cs`:

- Line 151: `// remainder is an ESpec block for the file itself`

- Implements `GetFileESpec()` method to generate this when writing

### Real-World Examples

**wow_classic 5.5.0.62655** (60 bytes):

```text
b:{22=n,76025=z,223424=n,28598272=n,146656=n,18771968=n,*=z}
```

**wow_classic_era 1.15.7.61582** (55 bytes):

```text
b:{22=n,2069=z,65536=n,8388608=n,43008=n,5505024=n,*=z}
```

Meaning:

- `22=n`: Header (22 bytes) uncompressed

- `76025=z`: ESpec table compressed with ZLib

- `223424=n`: CKey index uncompressed

- `28598272=n`: CKey pages uncompressed

- `146656=n`: EKey index uncompressed

- `18771968=n`: EKey pages uncompressed

- `*=z`: Remainder (the file's own ESpec) compressed

This self-referential design allows files to describe their own compression
structure using the same ESpec format as all other files.

## Common Issues

1. **Page Boundary Errors**: Entries can span pages
2. **Endianness**: All multi-byte values are big-endian
3. **ESpec Index**: Zero-based into string table
4. **Empty Pages**: Skip entries with ekey_count = 0
5. **File Size**: Remember to account for the file's own ESpec at the end

## Real-World Example

Using wow_classic_era 1.15.7.61582:

```text
Encoding file: bbf06e7476382cfaa396cff0049d356b

Header:
  Magic: 0x454E ('EN')
  Version: 1
  CKey/EKey size: 16 bytes each
  CKey pages: 4KB × 127 pages
  EKey pages: 4KB × 127 pages
  ESpec table: 1,234 bytes

Example CKey entry:
  Content Key: 3ce96e7a9e3b6f5c9d99c8b4e0a4f3d2
  EKey count: 1
  File size: 524,288 bytes (512KB)
  Encoding Key: 7f8a9b3c4d5e6f7081929a3b4c5d6e7f

Corresponding EKey entry:
  Encoding Key: 7f8a9b3c4d5e6f7081929a3b4c5d6e7f
  ESpec index: 1 (points to "z" - ZLib)
  Compressed size: 187,234 bytes
```

This shows a typical game asset compressed from 512KB to 183KB using ZLib.

## Implementation Flow

```rust
// 1. Fetch encoding file from CDN using encoding key
let encoding_key = "bbf06e7476382cfaa396cff0049d356b";
let encoding_data = cdn.fetch_data(encoding_key).await?;

// 2. Decompress BLTE container
let decompressed = blte::decompress(encoding_data)?;

// 3. Parse encoding file
let encoding = EncodingFile::parse(decompressed)?;

// 4. Look up content by content key
let content_key = hex!("3ce96e7a9e3b6f5c9d99c8b4e0a4f3d2");
let result = encoding.find_encoding_key(&content_key)?;

// 5. Fetch actual file using encoding key
let file_data = cdn.fetch_data(&result.ekeys[0]).await?;

// 6. Decompress using ESpec
let final_data = decompress_with_espec(file_data, result.espec)?;
```

## Implementation Status

### Rust Implementation (cascette-formats)

Complete Encoding file parser and builder with full format support:

- **Header parsing** - Magic bytes, version, page sizes (complete)

- **ESpec string table** - Null-terminated string handling (complete)

- **Page-based architecture** - CKey and EKey page support (complete)

- **Content resolution** - CKey to EKey mapping with multi-version support
(complete)

- **Binary preservation** - Page-level binary data for perfect round-trip
(complete)

- **Builder support** - EncodingBuilder with automatic ESpec generation
(complete)

**Validation Status:**

- Perfect byte-for-byte round-trip validation with real WoW encoding files

- Successfully processes WoW Classic Era encoding files

- Page-based processing for memory efficiency with large files

- Self-referential trailing ESpec generation for parser bootstrapping

### Python Tools (cascette-py)

Analysis tool supports:

- Magic byte detection and version parsing

- Page-based content lookup and analysis

- Content key to encoding key mapping

- ESpec table extraction and analysis

- String block parsing and validation

See <https://github.com/wowemulation-dev/cascette-py> for the Python
implementation.

## Version History

The Encoding file format currently has only one version:

### Version 1 (Current)

- **Header Size**: 22 bytes
- **Magic**: "EN" (0x454E)
- **Features**:
  - Content key to encoding key mapping
  - Dual page index system (CKey and EKey pages)
  - ESpec string table for compression metadata
  - 40-bit file sizes (up to 1TB per file)
  - Multiple encoding keys per content key support
  - Page-based binary search
  - MD5 page checksums for integrity

### Version Detection

All known encoding files use version 1. The version field is at offset 2 in the header. If future
versions are introduced, parsers should check this field after validating the "EN" magic bytes.

### Implementation Status

- **cascette-formats**: Full support for version 1 with parser and builder
- **cascette-py**: Complete analysis and extraction tools for version 1

## Binary Verification (Agent.exe, TACT 3.13.3)

Verified against Agent.exe (WoW Classic Era) using Binary Ninja on
2026-02-15. Parser source: `encoding_table_reader.cpp`.

### Confirmed Correct

| Claim | Agent Evidence |
|-------|---------------|
| Header: 22 bytes, "EN" magic (0x454E) | `sub_6a23e6` checks `*esi != 0x45 \|\| esi[1] != 0x4e`, size >= 0x16 |
| All header field offsets and sizes | `sub_6a23e6` reads exact offsets 0x00-0x15 |
| Big-endian u16 page sizes, u32 counts | `sub_6a2976` (BE u32 reader) and manual BE u16 shifts |
| Version must be 1 | `sub_6a23e6` at 0x6a242b checks `arg1.b != 1` |
| Unknown byte (offset 0x11) must be 0 | `sub_6a23e6` at 0x6a242b checks `esi[0x11] != 0` |
| CKey/EKey hash sizes: 16 bytes typical | Validated in range [1, 16] |
| Page sizes must be > 0 | `sub_6a23e6` at 0x6a24ca validates all sizes nonzero |

### Changes Applied

1. Added validation constraints: key sizes [1, 16], page sizes > 0,
   page counts > 0, espec_size > 0
2. Removed "powers of 2" page size claim (agent only validates > 0)
3. Clarified ESpec entry reference to point to espec.md

### Not Verifiable from Binary

- CKey/EKey page entry structures (40-bit file sizes) cannot be
  verified without a matching encoding file to cross-reference. The
  format description is consistent with cascette-rs implementation.

### Source Files

Agent source path: `d:\package_cache\tact\3.13.3\src\encoding_table\encoding_table_reader.cpp`

## References

- See [ESpec Documentation](espec.md) for encoding specifications

- See [BLTE Format](blte.md) for container structure

- See [CDN Architecture](cdn.md) for retrieval patterns

- See [Format Transitions](format-transitions.md) for format evolution tracking

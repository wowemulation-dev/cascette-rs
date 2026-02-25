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
    uint8_t  flags;            // 0x11: Flags (must be 0)
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

Entry layout (sizes from header):

```text
[count:1] [size:5] [ckey:ckey_size] [ekey1:ekey_size] [ekey2:ekey_size] ...
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
    uint8_t  file_size[5];         // Encoded file size (40-bit BE)
};
```

**Padding Detection**: EKey pages may contain padding entries that must be
skipped. Two sentinel patterns indicate padding:

1. `espec_index == 0xFFFFFFFF` (Agent.exe sentinel)
2. `espec_index == 0` with all key bytes `0x00` (zero-fill padding)

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

## Usage

### Parsing

```rust
use cascette_formats::encoding::EncodingFile;

// From decompressed data
let encoding = EncodingFile::parse(&data)?;

// From BLTE-encoded CDN data
let encoding = EncodingFile::parse_blte(&blte_data)?;
```

### Content Key Lookup

```rust
use cascette_crypto::ContentKey;

// Single lookup (binary search on page index, linear within page)
if let Some(ekey) = encoding.find_encoding(&content_key) {
    println!("Encoding key: {:?}", ekey);
}

// Get all encoding keys for a content key
let ekeys = encoding.find_all_encodings(&content_key);

// Batch lookup (sort-merge across pages)
let results = encoding.batch_find_encodings(&content_keys);
```

### EKey to ESpec Lookup

```rust
use cascette_crypto::EncodingKey;

if let Some(espec) = encoding.find_espec(&encoding_key) {
    println!("Compression spec: {}", espec);
}
```

### Building

```rust
use cascette_formats::encoding::{EncodingBuilder, CKeyEntryData, EKeyEntryData};

let mut builder = EncodingBuilder::new(); // 4KB pages
builder.add_ckey_entry(CKeyEntryData {
    content_key,
    file_size: 524_288,
    encoding_keys: vec![encoding_key],
});
builder.add_ekey_entry(EKeyEntryData {
    encoding_key,
    espec: "z".to_string(),
    file_size: 187_234,
});
let encoding_file = builder.build()?;
```

### Page Structure

All pages are loaded eagerly. Each page preserves its original binary data
for byte-exact round-trip reconstruction:

```rust
// Page<T> holds parsed entries and raw bytes
pub struct Page<T> {
    pub entries: Vec<T>,
    pub original_data: Vec<u8>,
}

// IndexEntry holds first key + MD5 checksum for integrity
pub struct IndexEntry {
    pub first_key: [u8; 16],
    pub checksum: [u8; 16],
}
```

All multi-byte header and page fields are big-endian.

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

Use cases include different regional encryption and progressive quality levels.

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
4. **CKey Padding**: Entries with `ekey_count = 0` indicate end of page data
5. **EKey Padding**: Entries with `espec_index = 0xFFFFFFFF` or all-zero keys
   indicate padding (see Padding Detection above)
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
use cascette_formats::encoding::EncodingFile;
use cascette_crypto::ContentKey;

// 1. Parse encoding file from BLTE-encoded CDN data
let encoding = EncodingFile::parse_blte(&cdn_data)?;

// 2. Look up content by content key
let ekey = encoding.find_encoding(&content_key)
    .ok_or("content key not found")?;

// 3. Optionally get the compression spec
let espec = encoding.find_espec(&ekey);

// 4. Fetch actual file from CDN using encoding key, then decompress
```

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

## References

- See [ESpec Documentation](espec.md) for encoding specifications

- See [BLTE Format](blte.md) for container structure

- See [CDN Architecture](cdn.md) for retrieval patterns

- See [Format Transitions](format-transitions.md) for format evolution tracking

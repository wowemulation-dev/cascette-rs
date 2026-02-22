# Root File Format

The Root file is the primary catalog of all files stored in CASC archives. It
maps file paths or FileDataIDs to content keys, enabling game clients to locate
and retrieve specific assets.

## Overview

The Root file serves as the master index for all game content:

- Maps FileDataIDs to content keys

- Supports multiple locales and content flags

- Groups files into blocks for efficient lookup

- Handles both named and unnamed entries

## File Structure

The Root file is BLTE-encoded and organized into blocks:

```text
[BLTE Container]
  [Header]
  [Block 1]
  [Block 2]
  ...
  [Block N]
```

## Binary Format

### Version Detection

The Root file format has evolved significantly:

- **Pre-30080**: No MFST magic, raw block data

- **Build 30080+ (v2)**: MFST magic with file counts

- **Build 50893+ (v3)**: Added header_size/version fields

- **Build 58221+ (v4)**: Extended content flags to 40 bits

### Header Structures

#### Version 2 (Build 30080+)

```c
struct RootHeaderV2 {
    uint32_t magic;              // 'MFST' (0x4D465354) or 'TSFM' (0x5453464D)
    uint32_t total_file_count;   // Total number of files
    uint32_t named_file_count;   // Number of named entries
};
```

**Note**: Some builds use 'TSFM' magic instead of 'MFST'. This appears to be
a little-endian representation. Both should be accepted as valid.

#### Version 3 (Build 50893+)

```c
struct RootHeaderV3 {
    uint32_t magic;              // 'MFST' (0x4D465354) or 'TSFM' (0x5453464D)
    uint32_t header_size;        // Size of header (20 bytes)
    uint32_t version;            // Version (1)
    uint32_t total_file_count;   // Total number of files
    uint32_t named_file_count;   // Number of named entries
    uint32_t padding;            // Padding (0)
};
```

**Note**: Version 3 also uses TSFM magic in observed builds, maintaining
consistency with Version 2.

**Version Detection Heuristic**: After reading the magic, check the next two
u32 values. If the first value (header_size) is in range [16, 100) and the
second value (version) is 2, 3, or 4, the file is v3+. Otherwise treat the
first value as total_file_count (v2).

### Block Structure

Each block contains file entries for specific locale and content flag
combinations. **Important**: The block header format changed significantly
between V1 and V2+.

#### V1 Block Header (Pre-30080, 12 bytes)

V1 files have no MFST/TSFM magic and use a 12-byte block header with
interleaved record format:

```c
struct RootBlockHeaderV1 {
    uint32_t num_records;        // Number of records in block
    uint32_t content_flags;      // Content flags (32-bit)
    uint32_t locale_flags;       // Locale flags (language/region)

    // FileDataID deltas (delta-encoded)
    int32_t fileDataIDDeltas[num_records];

    // Interleaved record data (content_key + name_hash per record)
    RootRecordInterleaved records[num_records];
};
```

#### V2+ Block Header (Build 30080+, 17 bytes)

V2 and later versions have MFST/TSFM magic and use a **17-byte** block header
with separated arrays. Per wowdev.wiki documentation for Version 2 (11.1.0+):

```c
#pragma pack(push, 1)
struct RootBlockHeaderV2 {
    uint32_t num_records;        // Number of records in block
    uint32_t locale_flags;       // Locale flags (MOVED - was third in V1!)
    uint32_t content_flags;      // Content flags (was second in V1)
    uint32_t unk2;               // Unknown field 2
    uint8_t  unk3;               // Unknown field 3 (flags via bit-shift)

    // FileDataID deltas (delta-encoded)
    int32_t fileDataIDDeltas[num_records];

    // Separated arrays (all content_keys, then all name_hashes)
    uint8_t content_keys[num_records][16];
    uint8_t name_hashes[num_records][8];  // Optional based on flags
};
#pragma pack(pop)
```

**Critical Implementation Note**: The field order change from V1 to V2+ is a
common source of parsing bugs. In V1, the order is `num_records, content_flags,
locale_flags`. In V2+, the order is `num_records, locale_flags, content_flags,
unk2, unk3`.

#### V4 Extended Content Flags

V4 (Build 58221+) extends content flags to 40 bits, increasing the block
header to **18 bytes** (the content_flags field grows from 4 to 5 bytes).
The 40-bit value is read as a `u32` (4 bytes) plus a `u8` (1 byte):

```c
uint32_t content_flags_low;   // Bits 0-31
uint8_t  content_flags_high;  // Bits 32-39
// Combined: content_flags = content_flags_low | (content_flags_high << 32)
```

### Record Formats

#### Old Format (Interleaved)

```c
struct RootRecordOld {
    uint8_t content_key[16];     // MD5 content key
    uint8_t name_hash[8];        // Jenkins96 name hash (optional)
};
```

#### New Format (Separated)

```c
struct RootRecordNew {
    // Arrays stored separately
    uint8_t content_keys[num_records][16];
    uint8_t name_hashes[num_records][8];  // Optional
};
```

## Content Flags

Content flags specify platform, architecture, and file attributes:

### 32-bit Flags (v2-v3)

| Bit | Flag | Description |
|-----|------|-------------|
| 0 | LoadOnWindows | Windows platform |
| 1 | LoadOnMacOS | macOS platform |
| 3 | LowViolence | Censored content |
| 9 | DoNotLoad | Skip file |
| 10 | UpdatePlugin | Launcher plugin |
| 11 | Arm64 | ARM64 architecture |
| 12 | Encrypted | Encrypted content |
| 13 | NoNameHash | No name hash in block |
| 14 | UncommonResolution | Non-standard resolution |
| 15 | Bundle | Bundled content |
| 16 | NoCompression | Uncompressed |
| 17 | NoTOCHash | No table of contents hash |

### 40-bit Flags (v4+)

Build 58221+ extends to 40 bits, stored as `u32` + `u8`:

- Bits 0-31: Standard content flags (same as v2/v3)

- Bits 32-39: Extended flags (single byte, shifted left by 32)

Common combinations:

- `0x00000000`: All platforms, default

- `0x00000001`: Windows only

- `0x00000002`: macOS only

- `0x00001000`: Encrypted content

- `0x00002000`: No name hash present

## Locale Flags

32-bit field representing language/region:

| Value | Locale | Description |
|-------|--------|-------------|
| 0x00000002 | enUS | English (US) |
| 0x00000004 | koKR | Korean |
| 0x00000010 | frFR | French |
| 0x00000020 | deDE | German |
| 0x00000040 | zhCN | Chinese (Simplified) |
| 0x00000080 | esES | Spanish (Spain) |
| 0x00000100 | zhTW | Chinese (Traditional) |
| 0x00000200 | enGB | English (UK) |
| 0x00000400 | enCN | English (China) |
| 0x00000800 | enTW | English (Taiwan) |
| 0x00001000 | esMX | Spanish (Mexico) |
| 0x00002000 | ruRU | Russian |
| 0x00004000 | ptBR | Portuguese (Brazil) |
| 0x00008000 | itIT | Italian |
| 0x00010000 | ptPT | Portuguese (Portugal) |
| 0xFFFFFFFF | All | All locales |

## FileDataID Delta Encoding

FileDataIDs use delta encoding for compression:

```rust
fn decode_file_data_ids(deltas: &[i32]) -> Vec<u32> {
    let mut ids = Vec::new();
    let mut current_id = 0u32;

    for (i, &delta) in deltas.iter().enumerate() {
        if i == 0 {
            // First entry: direct value, not a delta
            current_id = delta as u32;
        } else {
            // Subsequent entries: add delta to previous ID
            current_id = (current_id as i32 + delta) as u32;
        }
        ids.push(current_id);

        // Important: Increment for next iteration
        current_id += 1;
    }

    ids
}
```

**Note**: The algorithm increments current_id by 1 after each entry,
then applies the next delta. This handles sequential FileDataIDs efficiently.

## Lookup Process

1. **Parse Root file**: Decompress BLTE, read header and blocks
2. **Filter by flags**: Select blocks matching desired locale/content
3. **Find FileDataID**: Binary search or iterate through blocks
4. **Extract content key**: Retrieve corresponding MD5 hash
5. **Resolve via encoding**: Use content key to find encoding key

## Name Hash Calculation

For named files, Jenkins96 hash (hashlittle2) is used:

```rust
fn jenkins96_hash(filename: &str) -> u64 {
    // Normalize path: uppercase and backslashes to forward slashes
    let normalized = filename.to_uppercase().replace('\\', "/");
    let bytes = normalized.as_bytes();

    // Jenkins hashlittle2 with 0xDEADBEEF seed
    // Initial values: pc = 0, pb = 0 (passed by reference)
    let (pc, pb) = hashlittle2(bytes, 0, 0);

    // WoW swaps the high/low 32-bit halves
    let high = (hash64 >> 32) as u32;
    let low = (hash64 & 0xFFFF_FFFF) as u32;
    (u64::from(low) << 32) | u64::from(high)
}
```

**Important Jenkins96 Details**:

- Paths are normalized to uppercase with forward slashes

- The hash is 64-bit (8 bytes) not 96-bit despite the name

- Some blocks have `NoNameHash` flag, omitting name hashes entirely

- Uses Bob Jenkins' lookup3.c algorithm (hashlittle2 function)

- Processes data in 12-byte chunks with little-endian byte order

- The 0xDEADBEEF constant is added during initialization

- Python validation tool available in cascette-py project:
  <https://github.com/wowemulation-dev/cascette-py>

**Example Hashes**:

- Empty string: `0xDEADBEEFDEADBEEF`

- `Interface\Icons\INV_Misc_QuestionMark.blp`: `0x9EB59E3C76124837`

## Implementation Example

```rust
struct RootFile {
    header: RootHeader,
    blocks: Vec<RootBlock>,
}

impl RootFile {
    pub fn find_file(&self, file_data_id: u32) -> Option<MD5Hash> {
        for block in &self.blocks {
            // Check if block matches desired flags
            if !self.matches_flags(block) {
                continue;
            }

            // Search for FileDataID
            if let Some(idx) = block.find_file_index(file_data_id) {
                return Some(block.records[idx].content_key);
            }
        }
        None
    }
}
```

## Version History

- **Build 18125 (6.0.1)**: Initial CASC Root format (V1)
  - No magic header
  - 12-byte block header: `num_records, content_flags, locale_flags`
  - Interleaved record format: `(ckey, name_hash)` per record

- **Build 30080 (8.2.0)**: Added MFST magic signature (V2)
  - MFST/TSFM magic header with file counts
  - **17-byte block header**: `num_records, locale_flags, content_flags, unk2, unk3`
  - Field order changed: `locale_flags` moved before `content_flags`
  - Separated array format: all ckeys, then all name_hashes

- **Build 50893 (10.1.7)**: Added header_size/version fields (V3)
  - Extended header with `header_size`, `version`, `padding` fields
  - Same 17-byte block header format as V2

- **Build 58221 (11.1.0)**: Extended content flags to 40 bits (V4)
  - **18-byte block header** (content_flags grows from 4 to 5 bytes)
  - 40-bit content flags stored as `u32` + `u8`

### Version Detection Code

```rust
fn detect_root_version(data: &[u8]) -> RootVersion {
    if data.len() < 4 {
        return RootVersion::Invalid;
    }

    // Check for MFST or TSFM magic
    let magic = &data[0..4];
    if magic != b"MFST" && magic != b"TSFM" {
        return RootVersion::V1; // Pre-30080, no magic
    }

    // Read the two u32 values after magic
    let value1 = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let value2 = u32::from_le_bytes(data[8..12].try_into().unwrap());

    // Heuristic: header_size in [16, 100) and version in [2, 4]
    // indicates v3+ with explicit header_size/version fields
    if (16..100).contains(&value1) && matches!(value2, 2..=4) {
        // Version field at offset 8 distinguishes v3 and v4
        match value2 {
            4 => RootVersion::V4,
            _ => RootVersion::V3,
        }
    } else {
        RootVersion::V2 // 30080+, value1 is total_file_count
    }
}
```

## Parser Implementation Status

The Python parser (cascette-py) currently supports:

- Version detection (MFST/TSFM magic)

- Version 1-3 parsing

- Block-based extraction

- Content key retrieval

- Delta encoding detection (identifies but doesn't decode)

The parser can extract FileDataID to content key mappings from all current
WoW root file versions.

See <https://github.com/wowemulation-dev/cascette-py> for the Python
implementation.

## Common Issues

1. **V2 block header size**: V2+ uses a 17-byte block header, not 12 bytes like
   V1. Using the wrong header size causes all subsequent parsing to fail with
   garbage FileDataIDs and content keys.

2. **V2 field order change**: V2+ swapped `locale_flags` and `content_flags`
   positions. In V1: `num_records, content_flags, locale_flags`. In V2+:
   `num_records, locale_flags, content_flags, unk2, unk3`.

3. **Multiple matches**: Same file may exist in multiple blocks with different
   locales

4. **Missing entries**: Not all FileDataIDs have corresponding entries

5. **Flag interpretation**: Game-specific flag meanings vary

6. **Delta overflow**: Large gaps in FileDataIDs can cause integer overflow

## References

- See [Encoding Documentation](encoding.md) for content key resolution

- See [BLTE Format](blte.md) for container structure

- See [CDN Architecture](cdn.md) for file retrieval

- [wowdev.wiki TACT documentation](https://wowdev.wiki/TACT) - Authoritative
  source for CASC/TACT format specifications including Root file structure

# Size Manifest Format

The Size manifest maps encoding keys to estimated file sizes (eSize). It is used
when compressed size (cSize) is unavailable, allowing the agent to estimate disk
space requirements and report download progress for content that has not yet been
downloaded.

## Overview

The Size manifest provides:

- Estimated file sizes for pre-download space allocation

- Progress bar calculations during installation

- Disk space requirement checks

- Fallback sizing when compressed size is unknown

The agent log message "Loose files will estimate using eSize instead of cSize"
indicates when this manifest is active.

## Build Configuration Reference

The Size manifest is referenced by the `size` key in build configuration files:

```text
size = d1d9e612a645cc7a7e4b42628bde21ce 0d5704735f4985e555907a7e7647099a
size-size = 3637629 3076687
```

The first hash is the content key, the second is the encoding key used for CDN
fetch. The `size-size` field contains the unencoded and encoded sizes. Like other
manifests, the Size manifest is BLTE-encoded on CDN.

The config key `.tact:size_manifest` also references this manifest in the agent's
internal configuration.

## Community Documentation

This format is documented on [wowdev.wiki](https://wowdev.wiki/TACT) as the
"Download Size" manifest. The wiki documents version 1 from an older Agent build
(6700). The version 2 format with its 40-bit total size field is not documented
on the wiki. The wiki's "EKey Size" byte at offset 3 corresponds to the
`key_size_bits` field described below. Note that the wiki treats this as a
byte count, but the field stores *bits* (e.g., 72 for 9-byte keys). The wiki
does not document the tag section between header and entries.

## File Structure

The Size manifest is BLTE-encoded and contains:

```text
[BLTE Container]
  [Header]
  [Tags]       (0 or more, same format as install manifest tags)
  [Entries]
```

## Binary Format

All multi-byte integers are big-endian.

### Header

```c
struct SizeManifestHeader {
    char     magic[2];           // "DS" (0x44, 0x53)
    uint8_t  version;            // Version (1 or 2)
    uint8_t  key_size_bits;      // Encoding key width in bits (e.g. 72 = 9 bytes)
    uint32_t entry_count;        // Number of entries (big-endian)
    uint16_t tag_count;          // Number of tags (big-endian)

    // Version-specific fields follow
};
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 2 | `magic` | "DS" (0x44 0x53) |
| 2 | 1 | `version` | Format version (1 or 2) |
| 3 | 1 | `key_size_bits` | Encoding key width in **bits** (e.g. 72 = 9 bytes; byte count = `(bits + 7) >> 3`) |
| 4 | 4 | `entry_count` | Number of entries (big-endian) |
| 8 | 2 | `tag_count` | Number of tags after header (big-endian) |

#### Version 1 Header Extension (offset 10)

```c
struct SizeManifestHeaderV1 {
    // ... base header fields above ...
    uint64_t total_size;         // Total size across all entries (big-endian)
    uint8_t  esize_bytes;        // Byte width of eSize per entry (1-8)
};
// Total header size: 19 bytes (0x13)
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 10 | 8 | `total_size` | Sum of all entry esize values (big-endian) |
| 18 | 1 | `esize_bytes` | Byte width of esize per entry (1-8) |

The `esize_bytes` field determines how many bytes each entry's size value
occupies. Valid values are 1 through 8. Invalid values produce: "Invalid eSize
byte count '%u' in size manifest header."

#### Version 2 Header Extension (offset 10)

```c
struct SizeManifestHeaderV2 {
    // ... base header fields above ...
    uint8_t  total_size[5];      // Total size as 40-bit big-endian integer
};
// Total header size: 15 bytes (0x0F)
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 10 | 5 | `total_size` | Sum of all entry esize values, 40-bit big-endian (max ~1TB) |

Version 2 fixes `esize_bytes` at 4 (32-bit sizes per entry). The total size
uses a 40-bit integer (5 bytes), reducing header size compared to version 1.

### Minimum Size Validation

The parser validates two minimum sizes:

1. **15 bytes** (0x0F) -- enough to read the base header plus the shorter V2
   extension
2. **19 bytes** (0x13) -- required for version 1 headers (checked after reading
   the version byte)

If the data is too small: "Truncated data: expected {expected} bytes, got
{actual} bytes"

### Tags

Tags appear between the header and entries. The tag count is specified by the
`tag_count` field in the header. Tags use the same binary format as install
manifest tags (`InstallTag`), consisting of:

- A null-terminated name string
- A 2-byte type field (big-endian)
- A bitmask indicating which entries the tag applies to

Tags are used for platform and architecture filtering (e.g., "Windows",
"x86_64"), allowing the client to select entries relevant to the target system.

When `tag_count` is 0, no tags are present and entries follow the header
directly.

### Entry Format

Entries are stored sequentially after the tags:

```c
struct SizeManifestEntry {
    uint8_t  ekey[ekey_byte_count]; // Encoding key: (key_size_bits+7)>>3 bytes
    uint8_t  null_term;             // Null terminator (0x00) after the key
    uint16_t key_hash;              // 2-byte big-endian hash identifier
    uint8_t  esize[esize_bytes];    // Estimated size (variable width, big-endian)
};
```

| Field | Size | Description |
|-------|------|-------------|
| `ekey` | `(key_size_bits+7)>>3` bytes | Encoding key (raw bytes) |
| `null_term` | 1 byte | Null terminator (0x00) |
| `key_hash` | 2 bytes BE | 16-bit hash identifier; 0x0000 and 0xFFFF are reserved |
| `esize` | `esize_bytes` bytes | Estimated file size (big-endian, zero-extended to u64) |

The key length in bytes is computed from the header's `key_size_bits` field as
`(key_size_bits + 7) >> 3`. The entry stride is `key_bytes + 1 + 2 + esize_bytes`.

`key_hash` values 0x0000 and 0xFFFF are reserved sentinels; the parser rejects
entries with these values.

#### Entry Size Field

The `esize` field width depends on the version:

| Version | esize width | Source |
|---------|-------------|--------|
| 1 | `esize_bytes` from header (1-8) | Variable |
| 2 | 4 bytes (fixed) | Hardcoded |

The esize value is read as a big-endian unsigned integer and zero-extended to
a u64 for internal representation.

## Version History

| Version | Header size | esize width | total_size width | Notes |
|---------|-------------|-------------|------------------|-------|
| 1 | 19 bytes | Variable (1-8) | 64-bit | Original format, documented on wowdev.wiki |
| 2 | 15 bytes | Fixed (4) | 40-bit | Compact header, undocumented on wiki |

## Relationship to Other Manifests

The Size manifest is one of six manifest types in TACT:

| Config key | Magic | Format |
|------------|-------|--------|
| `encoding` | `EN` | Content key to encoding key mapping |
| `root` | (varies) | Path to content key mapping |
| `install` | `IN` | Install manifest with file tags |
| `download` | `DL` | Download manifest with priorities |
| `patch` | `PA` | Patch manifest for delta updates |
| `size` | `DS` | Size manifest (this format) |

## Validation

The parser validates manifests at parse time and via an explicit `validate()`
method:

- Magic bytes must be "DS"
- Version must be 1 or 2
- `key_size_bits` must produce a byte count of 1-16 (i.e., `key_size_bits` in 1-128)
- V1 `esize_bytes` must be 1-8
- Tag count matches the header's `tag_count` field
- Entry count matches the header's `entry_count` field
- Sum of all entry esize values matches the header's `total_size` field
- Each entry's key length matches `(key_size_bits + 7) >> 3`

## Error Messages

| Condition | Error |
|-----------|-------|
| Bad magic | `InvalidMagic` -- "Invalid magic: expected 'DS', got {bytes}" |
| Bad version | `UnsupportedVersion` -- "Unsupported version: {version}" |
| Truncated data | `TruncatedData` -- "Truncated data: expected {expected} bytes, got {actual} bytes" |
| Bad esize width (V1) | `InvalidEsizeWidth` -- "Invalid eSize byte count '{n}' in size manifest header" |
| Bad key_size_bits | `InvalidEKeySize` -- "Invalid key_size_bits: must be 1-128 (1-16 bytes), got {n}" |
| Reserved key_hash | `InvalidKeyHash` -- entry `key_hash` is 0x0000 or 0xFFFF |
| Tag count mismatch | `TagCountMismatch` -- "Tag count mismatch: header says {expected}, found {actual}" |
| Entry count mismatch | `EntryCountMismatch` -- "Entry count mismatch: header says {expected}, found {actual}" |
| Total size mismatch | `TotalSizeMismatch` -- "Total size mismatch: header says {expected}, sum of esizes is {actual}" |

## Implementation Status

Implemented in `cascette-formats` crate (`crates/cascette-formats/src/size/`).

The implementation provides:

- Parser and builder for both version 1 and version 2 formats
- Manual `BinRead`/`BinWrite` implementations for headers and entries
- Variable-width esize field support (1-8 bytes for V1, fixed 4 bytes for V2)
- 40-bit total_size handling for V2 headers
- Tag support using the same `InstallTag` format as install/download manifests
- Builder pattern with tag construction via `add_tag()` and `tag_file()`
- `CascFormat` trait implementation for round-trip support

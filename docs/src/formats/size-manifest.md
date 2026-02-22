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
(6700). The TACT 3.13.3 agent binary supports versions 1 and 2. The wiki's
"EKey Size" byte at offset 3 corresponds to the `flags` field described below.
The version 2 format with its 40-bit total size field is not documented on the
wiki.

## File Structure

The Size manifest is BLTE-encoded and contains:

```text
[BLTE Container]
  [Header]
  [Entries]
```

## Binary Format

All multi-byte integers are big-endian.

### Header

```c
struct SizeManifestHeader {
    char     magic[2];           // "DS" (0x44, 0x53)
    uint8_t  version;            // Version (1 or 2)
    uint8_t  flags;              // Flags byte
    uint32_t entry_count;        // Number of entries (big-endian)
    uint16_t key_size_bits;      // Key size in bits (big-endian)

    // Version-specific fields follow
};
```

#### Version 1 Header Extension (offset 10)

```c
struct SizeManifestHeaderV1 {
    // ... base header fields above ...
    uint64_t total_size;         // Total size across all entries (big-endian)
    uint8_t  esize_bytes;        // Byte width of eSize per entry (1-8)
};
// Total header size: 19 bytes (0x13)
```

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

Version 2 fixes `esize_bytes` at 4 (32-bit sizes per entry). The total size
uses a 40-bit integer (5 bytes), reducing header size compared to version 1.

### Minimum Size Validation

The parser validates two minimum sizes:

1. **15 bytes** (0x0F) -- enough to read magic, version, entry_count, and
   key_size_bits
2. **19 bytes** (0x13) -- full version 1 header (version 2 headers are shorter
   and pass this check)

If the data is too small: "Detected truncated size manifest. Only got %u bytes,
but minimum header size is %u bytes."

### Entry Format

Entries are stored sequentially after the header:

```c
struct SizeManifestEntry {
    uint8_t  key[];              // Encoding key, null-terminated
    uint16_t key_hash;           // 16-bit hash/identifier (big-endian)
    uint8_t  esize[];            // Estimated size (esize_bytes width, big-endian)
};
```

The key field length in bytes is `(key_size_bits + 7) / 8`, which rounds the
bit count up to the nearest byte. The key is stored as a null-terminated byte
string within this field.

#### Key Hash Validation

The 2-byte `key_hash` field after the key is validated. Values `0x0000` and
`0xFFFF` are treated as invalid sentinel values and cause the parser to reject
the entry.

#### Entry Size Field

The `esize` field width depends on the version:

| Version | esize width | Source |
|---------|-------------|--------|
| 1 | `esize_bytes` from header (1-8) | Variable |
| 2 | 4 bytes (fixed) | Hardcoded |

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

- Entry count matches the header's `entry_count` field
- Sum of all entry esize values matches the header's `total_size` field
- `key_size_bits` must be > 0
- Key hash sentinel values (0x0000, 0xFFFF) are rejected

## Error Messages

| Condition | Message |
|-----------|---------|
| Truncated data | "Detected truncated size manifest. Only got %u bytes, but minimum header size is %u bytes." |
| Bad magic | "Invalid magic string in size manifest." |
| Bad version | "Unsupported size manifest version: %u. This client only supports non-zero versions <= %u" |
| Bad esize width | "Invalid eSize byte count '%u' in size manifest header." |
| Zero key size | "Invalid key size: key_size_bits must be > 0" |
| Bad key hash | "Invalid key hash sentinel value: 0x{value:04X}" |
| Entry count mismatch | "Entry count mismatch: header says {expected}, found {actual}" |
| Total size mismatch | "Total size mismatch: header says {expected}, sum of esizes is {actual}" |

## Implementation Status

Implemented in `cascette-formats` crate (`crates/cascette-formats/src/size/`).

The implementation provides:

- Parser and builder for both version 1 and version 2 formats
- Manual `BinRead`/`BinWrite` implementations for headers and entries
- Variable-width esize field support (1-8 bytes for V1, fixed 4 bytes for V2)
- 40-bit total_size handling for V2 headers
- Key hash sentinel validation (rejects 0x0000 and 0xFFFF)
- `CascFormat` trait implementation for round-trip support
- Builder pattern for constructing manifests

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
(6700). The wiki's "EKey Size" byte at offset 3 stores the key length in
**bytes** (e.g., 9 for a 9-byte encoding key). The wiki does not document the
tag section between header and entries.

Note: The Blizzard Agent uses a different internal representation with
`key_size_bits` (key length in bits) and per-entry null terminators + key hashes.
The CDN wire format documented here does not use those fields.

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

### Header (15 bytes)

```c
struct SizeManifestHeader {
    char     magic[2];           // "DS" (0x44, 0x53)
    uint8_t  version;            // Version (1)
    uint8_t  ekey_size;          // Encoding key length in bytes (typically 9)
    uint32_t num_files;          // Number of entries (big-endian)
    uint16_t num_tags;           // Number of tags (big-endian)
    uint8_t  total_size[5];      // Total size as 40-bit big-endian integer
};
// Total header size: 15 bytes (0x0F)
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 2 | `magic` | "DS" (0x44 0x53) |
| 2 | 1 | `version` | Format version (1) |
| 3 | 1 | `ekey_size` | Encoding key length in **bytes** (typically 9; valid range 1-16) |
| 4 | 4 | `num_files` | Number of entries (big-endian) |
| 8 | 2 | `num_tags` | Number of tags after header (big-endian) |
| 10 | 5 | `total_size` | Sum of all entry esize values, 40-bit big-endian (max ~1 TB) |

Only version 1 has been observed in the wild across all WoW Classic builds
(1.13.2.31650 through 3.4.3.64272).

### Minimum Size Validation

The parser requires at least **15 bytes** (0x0F) to read the header.

If the data is too small: "Truncated data: expected {expected} bytes, got
{actual} bytes"

### Tags

Tags appear between the header and entries. The tag count is specified by the
`num_tags` field in the header. Tags use the same binary format as install
manifest tags (`InstallTag`), consisting of:

- A null-terminated name string
- A 2-byte type field (big-endian)
- A bitmask indicating which entries the tag applies to

Tags are used for platform and architecture filtering (e.g., "Windows",
"x86_64"), allowing the client to select entries relevant to the target system.

When `num_tags` is 0, no tags are present and entries follow the header
directly.

### Entry Format

Entries are stored sequentially after the tags:

```c
struct SizeManifestEntry {
    uint8_t  ekey[ekey_size];    // Partial encoding key (ekey_size bytes from header)
    uint32_t esize;              // Estimated file size (big-endian)
};
```

| Field | Size | Description |
|-------|------|-------------|
| `ekey` | `ekey_size` bytes | Partial encoding key (raw bytes) |
| `esize` | 4 bytes BE | Estimated file size (big-endian u32) |

The entry stride is `ekey_size + 4`. Entries are sorted descending by `esize`.

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
- Version must be non-zero
- `ekey_size` must be 1-16
- Tag count matches the header's `num_tags` field
- Entry count matches the header's `num_files` field
- Sum of all entry esize values matches the header's `total_size` field
- Each entry's key length matches `ekey_size`

## Error Messages

| Condition | Error |
|-----------|-------|
| Bad magic | `InvalidMagic` -- "Invalid magic: expected 'DS', got {bytes}" |
| Bad version | `UnsupportedVersion` -- "Unsupported version: {version}" |
| Truncated data | `TruncatedData` -- "Truncated data: expected {expected} bytes, got {actual} bytes" |
| Bad ekey_size | `InvalidEKeySize` -- "Invalid ekey_size {n}: must be 1-16" |
| Tag count mismatch | `TagCountMismatch` -- "Tag count mismatch: header says {expected}, found {actual}" |
| Entry count mismatch | `EntryCountMismatch` -- "Entry count mismatch: header says {expected}, found {actual}" |
| Total size mismatch | `TotalSizeMismatch` -- "Total size mismatch: header says {expected}, sum of esizes is {actual}" |

## Agent Internal Format (Reference)

The Blizzard Agent uses a different internal representation of this data with:

- `key_size_bits` as a u16 at offset 8 (key width in bits, not bytes)
- u64 `total_size` for V1 (8 bytes instead of 40-bit)
- `esize_bytes` field controlling variable-width entry sizes (1-8 bytes)
- Null-terminated key + 2-byte `key_hash` per entry (0x0000 and 0xFFFF reserved)

This internal format is documented in the management repository. The CDN wire
format above is what clients download and what cascette-rs implements.

## Implementation Status

Implemented in `cascette-formats` crate (`crates/cascette-formats/src/size/`).

The implementation provides:

- Parser and builder for the wire format
- Manual `BinRead`/`BinWrite` implementations for headers and entries
- Tag support using the same `InstallTag` format as install/download manifests
- Builder pattern with tag construction via `add_tag()` and `tag_file()`
- `CascFormat` trait implementation for round-trip support

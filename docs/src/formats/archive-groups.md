# Archive-Groups

Archive-groups are **locally generated mega-indices** that combine multiple CDN
archive indices into a single unified lookup structure. They are created
client-side by merging downloaded archive index files, never downloaded directly
from the CDN. They are essential for Battle.net client compatibility and enable
efficient content resolution.

## Format Specification

Archive-groups use the same binary format as regular CDN archive indices with
one critical difference:

| Field | Regular Index | Archive-Group |
|-------|--------------|---------------|
| Encoding Key | Variable (9-16 bytes) | 16 bytes |
| **Offset** | **4 bytes** | **6 bytes** |
| Size | 4 bytes | 4 bytes |

The 6-byte offset field contains:

- Bytes 0-1: Archive index (big-endian u16)
- Bytes 2-5: Offset within archive (big-endian u32)

## Critical Findings - SOLVED

### Archive Index Mapping Uses Hash-Based Assignment

**CONFIRMED**: ALL archive-groups use the full u16 range (0-65535) for archive
indices:

```text
archive_index = hash(encoding_key) % 65536
```

This explains why:

- All archive-groups use indices 0-65535 despite only ~606 CDN archives existing
- Archive 0 consistently receives 6-8% of entries (hash distribution)
- The pattern is universal across all Battle.net installations
- Archive-groups are generated locally using this deterministic hash-based
  assignment algorithm

### CDN Configuration

Archive-groups are referenced in CDN config files by their hash:

```text
archive-group = 6d08c5f69f6a2cf70a50cd40efdcd2fb
patch-archive-group = a5fb3ed088333348d93983d7e8693956
```

These hashes identify the locally generated archive-group files stored in
`Data/indices/`. The client generates these files locally and stores them using
the computed hash as the filename.

### Size Characteristics

Archive-groups are significantly larger than regular indices:

- Regular CDN indices: 4KB - 2MB
- Archive-groups: 50MB - 150MB
- Entry count: 2-5 million entries

Growth over time (WoW Classic):

- Version 1.13.2: 54MB, 2.1M entries
- Version 1.14.0: 73MB, 2.8M entries
- Version 1.15.2: 126MB, 5.0M entries

### Archive Index Distribution

Due to hash-based assignment, archive indices follow a predictable distribution:

1. Archive 0: ~6-8% of entries (150K-350K entries)
2. Archive 1: ~0.6% of entries (13K entries)
3. Archive 2-65535: Distributed based on hash function

This distribution is consistent across all Battle.net installations.

## Implementation Requirements

### Detection

```rust
fn is_archive_group(data: &[u8]) -> bool {
    if data.len() < 28 {
        return false;
    }
    // Check offset_bytes field at position -16 from end
    data[data.len() - 16] == 6
}
```

### Parsing

```rust
// For archive-groups with 6-byte offsets
let archive_index = u16::from_be_bytes([data[pos], data[pos + 1]]);
let offset = u32::from_be_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]]);
```

### Content Resolution

When resolving content in a Battle.net-compatible installation:

1. Look up encoding key in archive-group
2. Extract 2-byte archive index from entry
3. Map archive index to actual CDN archive (requires mapping table)
4. Read content from archive at specified offset

## Implementation Strategy for Cascette

To achieve binary-identical Battle.net installations:

### Required Actions

1. **Generate Archive-Groups Locally**
   - Parse CDN config to find all individual archive index hashes
   - Download all individual `.index` files from CDN
   - Merge them locally into unified archive-group structures
   - Store generated archive-groups in `Data/indices/` using computed hash as filename

2. **Implement Hash-Based Archive Assignment**
   - Use deterministic algorithm: `archive_index = hash(encoding_key) % 65536`
   - Ensure identical results to Battle.net client generation
   - Apply to all entries during archive-group creation

3. **Implement Archive Index Mapping**
   - Create mapping table: `archive_group_index -> actual_cdn_archive_hash`
   - The 65536 virtual indices map to ~606 actual CDN archives
   - Use for content resolution when accessing actual archive data

4. **Support Both Types**
   - Generate regular archive-group for main content from base archive indices
   - Generate patch-archive-group for patch content from patch archive indices
   - Both use same local generation process with 6-byte offsets

## Why Binary-Identical Matters

For cascette to be a trustworthy Battle.net replacement:

1. **Trust**: Users need confidence we produce EXACTLY what Battle.net would
2. **Compatibility**: Some third-party tools may depend on exact format
3. **Verification**: Binary matching allows easy validation
4. **Completeness**: Understanding the full algorithm proves our reverse
   engineering

## Footer Structure

Archive-groups are identified by the `offset_bytes` field in the footer:

```text
Footer (28 bytes):
  [0:8]   TOC hash (first 8 bytes of MD5)
  [8]     Version (always 1)
  [9:11]  Reserved
  [11]    Page size in KB
  [12]    Offset bytes (4 for regular, 6 for archive-group)
  [13]    Size bytes (always 4)
  [14]    Key bytes (16 for archive-groups)
  [15]    Footer hash bytes
  [16:20] Entry count (little-endian u32)
  [20:28] Footer hash
```

## Example Archive-Group Entry

```text
Entry from 6d08c5f69f6a2cf70a50cd40efdcd2fb.index:
  Key: 000003bafc39011c91accae47b94fb2d (16 bytes)
  Archive: 0 (from first 2 bytes of offset field)
  Offset: 0x5dfd00d7 (from last 4 bytes of offset field)
  Size: 92,211,754 bytes
```

This entry indicates:

- Content is in archive index 0
- Starts at offset 0x5dfd00d7 in that archive
- Compressed size is 92,211,754 bytes

## Validation

Archive-groups contain entries for all game content:

- Every encoding key should be findable
- Archive indices use full u16 range (0-65535)
- Entries are sorted by encoding key for binary search
- Total entries match the entry_count in footer

## Battle.net Client Behavior

The Battle.net client:

1. Downloads individual archive index files during installation
2. Generates archive-group locally by merging multiple archive indices
3. Stores generated archive-group in `Data/indices/{hash}.index`
4. Uses hash-based assignment algorithm for consistent archive index mapping
5. Uses archive-group for all subsequent content lookups

## Common Issues

### Incorrect Detection

- Checking file size alone is insufficient
- Must verify `offset_bytes == 6` in footer
- Some patch archives are large but not archive-groups

### Index Mapping Confusion

- Archive index in archive-group ≠ CDN archive position
- Indices 0-65535 map to ~600 actual archives
- Mapping requires modulo or lookup table

### Parser Assumptions

- Never hardcode 9-byte keys for archive-groups
- Archive-groups always use 16-byte keys
- Respect the `key_bytes` field in footer

## References

- Analysis of WoW Classic installations (1.13.2 through 1.15.2)
- [wowdev.wiki Archive documentation](https://wowdev.wiki/CASC#Archives)
- Battle.net client reverse engineering
- Empirical testing with cascette-py parser

# CASC Local Storage

Local CASC storage is the on-disk format used by the Battle.net client to store
game data. Unlike CDN archives which are content-addressed, local storage uses
optimized indices for fast file lookups.

## Directory Structure

A typical CASC installation has the following structure:

```
<install-dir>/
├── .build.info           # Build configuration
├── Data/
│   ├── data/
│   │   ├── data.000      # Combined archive data
│   │   ├── data.001
│   │   └── ...
│   ├── indices/
│   │   ├── 0000.idx      # Bucket indices (16 files)
│   │   ├── 0001.idx
│   │   └── ...
│   └── shmem             # Shared memory file
└── Cache/
    └── ADB/              # Hotfix database cache
        └── *.bin
```

## Index Files (.idx)

Local indices use a different format than CDN indices:

- **Key size**: 9 bytes (truncated from full 16-byte keys)
- **Offset size**: 4 bytes
- **Bucket distribution**: Files distributed across 16 index buckets

The 9-byte key truncation saves space while maintaining sufficient uniqueness
for local lookups.

### Bucket Assignment

Files are assigned to index buckets based on the first byte of their key:

```
bucket = key[0] & 0x0F
```

## Data Files (.data.xxx)

Data files contain BLTE-encoded content copied from CDN archives. Each entry
includes:

- Entry header with key and size information
- BLTE-encoded payload
- Checksum for verification

## Shared Memory (shmem)

The shmem file provides memory-mapped access to frequently used metadata:

- Data file locations
- Version information
- Free space tracking

## .build.info

The `.build.info` file contains installation metadata in BPSV format:

- Product code and region
- Active build configuration hash
- CDN configuration hash
- Installation tags and flags

## Differences from CDN Storage

| Aspect | CDN | Local |
|--------|-----|-------|
| Key size | 16 bytes | 9 bytes |
| Organization | Per-archive indices | Bucket-based indices |
| Compression | Already BLTE | Already BLTE |
| Mutability | Immutable | Updated during patches |

## References

- [Archives](../formats/archives.md)
- [Archive Groups](../formats/archive-groups.md)
- [BLTE Container](../compression/blte.md)

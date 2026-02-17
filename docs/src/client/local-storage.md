# CASC Local Storage

Local CASC storage is the on-disk format used by the Battle.net client to store
game data. Unlike CDN archives which are content-addressed, local storage uses
optimized indices for fast file lookups.

## Directory Structure

A typical CASC installation has the following structure:

```text
<install-dir>/
├── .build.info           # Build configuration (BPSV format)
├── Data/
│   ├── data/
│   │   ├── 0000000001.idx  # Local index files (16 buckets)
│   │   ├── 0100000001.idx
│   │   ├── ...
│   │   ├── 0f00000001.idx
│   │   ├── data.000        # Combined archive data
│   │   ├── data.001
│   │   └── ...
│   ├── indices/
│   │   └── ...             # CDN index files (not local storage)
│   └── shmem               # Shared memory control file
└── Cache/
    └── ADB/                # Hotfix database cache
        └── *.bin
```

Local `.idx` index files and `.data` archive files both reside in `Data/data/`.
The `Data/indices/` directory holds CDN index files, which are a separate
concern from local storage.

## Container Types

CASC manages four container types for local storage:

| Type | Size | Purpose |
|------|------|---------|
| Dynamic | 0x3c bytes | Read/write CASC archives (.data files) |
| Static | -- | Read-only archives (shared installations) |
| Residency | 0x30 bytes | File state tracking (.residency tokens) |
| Hard Link | 0x30 bytes | Filesystem hard links (trie directory) |

The Dynamic container is the primary read-write storage. It manages archive
segments, key state tracking, and shared memory coordination. Access modes:
0=none, 1=read-only, 2=read-write, 3=exclusive.

## Index Files (.idx)

Local indices use IDX Journal v7 format with little-endian headers (unlike most
NGDP formats which use big-endian).

- **Key size**: 9 bytes (truncated encoding keys)
- **Location size**: 5 bytes (1 byte archive high + 4 bytes packed)
- **Entry size**: 18 bytes (9 key + 5 location + 4 size)
- **Bucket distribution**: 16 index buckets (0x00-0x0F)

The 9-byte key truncation saves space while maintaining sufficient uniqueness
for local lookups. Keys are encoding keys, not content keys.

### Index File Format

Each `.idx` file contains guarded blocks with Jenkins hash validation:

```text
[GuardedBlockHeader]  (8 bytes: size + Jenkins hash)
[IndexHeaderV2]       (16 bytes: version, bucket, field sizes, segment_size)
[padding]             (8 bytes: hash/alignment)
[GuardedBlockHeader]  (8 bytes: entry block size + Jenkins hash)
[IndexEntry[]]        (N * 18 bytes: sorted by key)
```

### Index Filename Format

```text
{bucket:02x}{version:08x}.idx
```

Example: `0a00000003.idx` = bucket 0x0A, version 3. Total filename length is
14 characters (10 hex digits + `.idx`).

### Bucket Assignment

Files are assigned to index buckets using the XOR-fold algorithm on the first
9 bytes of the encoding key:

```text
hash = key[0] ^ key[1] ^ key[2] ^ key[3] ^ key[4] ^ key[5] ^ key[6] ^ key[7] ^ key[8]
bucket = (hash & 0x0F) ^ (hash >> 4)
```

Agent uses a flush-and-bind pattern with 3-retry atomic commits when writing
index files.

### Key Mapping Table (KMT)

Below the index files, CASC maintains a Key Mapping Table (KMT) as the
primary on-disk structure for key-to-location resolution:

- Two-tier LSM-tree: sorted section (0x12-byte entries) + update section
  (0x200-byte pages)
- Jenkins lookup3 hashes for bucket distribution
- 9-byte EKey prefix binary search within sorted sections
- KMT v8 (revision >= 8): sorted section uses 0x20-byte buckets, update
  section uses 0x400-byte pages with 0x19 entries per page (minimum 0x7800
  bytes)

## Data Files (.data.xxx)

Data files contain BLTE-encoded content. Each entry has a 30-byte (0x1E) local
header before the BLTE data:

```text
Offset  Size  Field
0x00    16    Encoding key (reversed byte order)
0x10    4     Size including header (big-endian)
0x14    2     Flags
0x16    4     ChecksumA
0x1A    4     ChecksumB
0x1E    ...   BLTE data
```

### Archive Location Packing

The 5-byte archive location in index entries encodes both archive ID and offset:

```text
Byte 0:      archive_id >> 2 (high 8 bits)
Bytes 1-4:   (archive_id_low << 30) | (offset & 0x3FFFFFFF) (big-endian)
```

This gives 10-bit archive IDs (max 1023) and 30-bit offsets (max ~1 GiB).

### Container Index

Agent maintains a ContainerIndex with 16 segments and supports frozen/thawed
archive management:

- Segments can be frozen (read-only) or thawed (writable)
- 0x1E-byte reconstruction headers per archive entry
- Segment limit configurable up to 0x3FF (1023)
- Per-segment tracking: 0x40 (64) bytes per segment in compactor state

## Shared Memory (shmem)

The shmem file provides memory-mapped coordination between the Agent process
and game clients:

- Protocol versions 4 (base) and 5 (exclusive access flag at +0x54)
- Free space table at offset 0x42, size 0x2AB8 bytes
- PID tracking: slot array with "PID : name : mode" formatting
- Writer lock: named global mutex with `Global\` prefix
- DACL: `D:(A;;GA;;;WD)(A;;GA;;;AN)` (grant all to Everyone + Anonymous)
- Retry logic: 10 attempts with `Sleep(0)` between failures
- `.lock` file with 10-second backoff for coordination

### LRU Cache

Agent maintains an LRU cache in shared memory:

- Linked-list table structure
- Generation-based checkpoints for eviction
- 20-character hex filenames with `.lru` extension

## .build.info

The `.build.info` file contains installation metadata in BPSV format:

- Product code and region
- Active build configuration hash
- CDN configuration hash
- Installation tags and flags

## Residency Tracking

The Residency container tracks which content keys are fully downloaded:

- `.residency` token files mark valid containers
- Byte-span tracking for partial downloads (header and data residency)
- Reserve, mark-resident, remove, query operations
- Scanner API for enumeration
- Drive type check prevents unsupported storage media

## Hard Link Storage

The Hard Link container uses a TrieDirectory for content sharing:

- Hard links allow multiple keys to reference the same physical file
- 32-character hex filename validation
- Unlinked key collection (link count <= 1)
- Recursive compaction
- LRU file descriptor cache with two open modes (handle vs async IO)
- 3-retry delete before hard link creation
- Falls back to residency when hard links are unsupported

## Maintenance Operations

### Compaction

Two-phase process: archive merge then extract-compact.

- Defrag algorithm: removes gaps between files, reorganizes positions
- Fillholes algorithm: estimates free space without moving data
- Merge threshold: float in [0.0, 0.4]
- Async read/write pipeline with 128 KB minimum buffer
- Per-segment span validation with overlap detection

### Garbage Collection

4-stage pipeline:

1. Remove unreferenced keys from dynamic container
2. Remove obsolete config files
3. Remove CDN index files
4. Clean up empty directories recursively

### Build Repair

Multi-stage pipeline using marker files for crash recovery:

- `RepairMarker.psv` (pipe-separated, writable keys)
- `CASCRepair.mrk` (V2 marker format)
- Stages: read config, init CDN index, repair containers (data/ecache/hardlink
  sequentially), data repair, post-repair cleanup

## Differences from CDN Storage

| Aspect | CDN | Local |
|--------|-----|-------|
| Key size | 16 bytes | 9 bytes (truncated) |
| Key type | Content keys | Encoding keys |
| Organization | Per-archive indices | 16-bucket index files |
| Entry header | None | 30-byte local header |
| Index format | CDN index footer | IDX Journal v7 with guarded blocks |
| Mutability | Immutable | Updated during patches |
| Containers | Single type | 4 types (dynamic/static/residency/hardlink) |

## References

- [Archives](../formats/archives.md)
- [Archive Groups](../formats/archive-groups.md)
- [BLTE Container](../compression/blte.md)
- [Agent Comparison](../development/agent-comparison.md)

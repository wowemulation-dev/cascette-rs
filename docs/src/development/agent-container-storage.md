# Agent.exe Container Storage

Reverse engineering notes from Agent.exe (TACT 3.13.3, CASC 1.5.9).
Source: BinaryNinja decompilation of `casc::ContainerIndex::*`,
`casc::Dynamic::*`, `casc::Residency::*`, `TrieDirectory::*`,
`casc::Lru::*`, and `ShmemControlBlock::*` functions.

## Container Index

The ContainerIndex manages 16 KMT buckets and coordinates segment
lifecycle, storage allocation, and key mapping.

Source: `casc::ContainerIndex::Open` (0x72a8fd),
`casc::ContainerIndex::CreateSegment` (0x72a303),
`casc::ContainerIndex::AllocateKeyMapping` (0x72868e),
`casc::ContainerIndex::UpdateKeyMapping` (0x72abe7).

### Object Layout

Key offsets within the ContainerIndex object (dword-indexed from
decompilation, multiply by 4 for byte offsets):

| Byte Offset | Field |
|-------------|-------|
| 0x0C | Flags (bit 0: inline path) |
| 0x10 | Path pointer (or inline string) |
| 0x2C | Version (4 or 5) |
| 0x30 | Current segment index |
| 0x34 | Max segment index |
| 0x44 | Allocation SRW lock |
| 0x48 | Shared memory control block pointer |
| 0x80 | Writer lock |
| 0xA4 | IndexTables base |
| 0x130 | KMT bindings array (16 x 0x50 bytes) |
| 0x630 | Loose storage allocator |
| 0x6CC | Per-bucket SRW lock array (16 x 4 bytes) |

### Open Sequence

`ContainerIndex::Open`:

1. Create index directory on disk
2. Bind shared memory with retry logic (up to 3 attempts, 20-second
   sleep between retries). Logs "Retrying shared memory bind" on retry
   and "Another process is interfering with our ability to bind shared
   memory" after 3 failures.
3. Initialize writer lock at offset 0x80
4. Initialize IndexTables at offset 0xA4
5. Bind 16 KMT mapping tables (loop 0..16), each at offset
   `0x130 + bucket * 0x50`
6. Copy segment info from IndexTables to ContainerIndex fields
7. If version != 4: call `Rebuild` to reconstruct state
8. Initialize span validation

### Segment Lifecycle

Segments are created on demand when storage allocation runs out of
space in the current segment.

`ContainerIndex::CreateSegment`:

1. Generate 16 segment header keys via `GenerateSegmentHeaders`
   (one per bucket). Key generation uses path hash + bucket targeting
   as documented in MEMORY.md.
2. For each of 16 buckets:
   - Acquire per-bucket SRW lock at offset `0x6CC + bucket * 4`
   - Validate bucket state
   - Verify generated key does not already exist (logs telemetry
     "dynamic_container.index.segment_header.key_exists" if collision)
3. Allocate space for the 480-byte segment header (16 x 30-byte
   `LocalHeader` entries)
4. Write segment headers to storage via `WriteSegmentHeaders`
5. Call storage callback to persist raw header data

### Storage Allocation

`ContainerIndex::AllocateKeyMapping`:

1. Check state is not frozen (state 2) or read-only (state 4) --
   returns ERROR_ACCESS_DENIED
2. If loose storage is available (offset 0x09 != 0): try loose
   allocation first at offset 0x630
3. Acquire allocation SRW lock at offset 0x44
4. Try segment allocator for the requested size
5. If ERROR_CURRENT_DIRECTORY (out of space in current segment):
   - Extract segment index from high bits of offset:
     `*offset >> 30 | offset_high << 2`
   - Call `CreateSegment` for the new segment index
   - Retry allocation
6. If still fails: map ERROR_CURRENT_DIRECTORY to ERROR_OUTOFMEMORY

Segment limit: index 0x3FF (1023). Segment size: 1 GiB (0x40000000).

### Key Mapping Update

`ContainerIndex::UpdateKeyMapping`:

1. Access per-bucket KMT at `bucket * 0x50 + this + 0x130`
2. Call `KeyMappingTable::InsertEntryLocked` with key, status, span
3. If ERROR_INVALID_DRIVE (update section full):
   - Flush the table via `FlushTable`
   - Retry insert
4. Delete entries (status 3) that return ERROR_FILE_NOT_FOUND are
   treated as success (key already absent)
5. On failure: logs "Failed to update key mapping for key '%s' to
   update type %u"

### Flush

`ContainerIndex::FlushTable` (0x7291f5):

1. Calls `IndexTables::FlushAndBindLoose` with the KMT binding
   at `0xA4` (IndexTables) and `0x130 + bucket * 0x50` (KMT binding)
2. On success: stores the new file handle in the shared memory control
   block at offset `0x110 + bucket * 4`

## Truncation Tracking

Agent tracks truncated reads and missing archives as part of the
read path. This is how partially-downloaded content gets marked as
non-resident.

Source: `casc::Dynamic::Read` (0x71a564),
`casc::Dynamic::HandleTruncatedRead` (0x718c0d),
`casc::Dynamic::HandleMissingArchive` (0x718b84),
`casc::Dynamic::UpdateResidency` (0x71a09f).

### Read Flow

`Dynamic::Read` calls `DynamicStorage::ReadFromArchive` and handles
two error cases:

**Truncated read** (success but `bytes_read < expected_size`):

1. Bump telemetry counter: `"dynamic_container.read_truncated.count"`
2. Adjust storage offset by +30 bytes (0x1E) to account for the
   30-byte segment local header
3. Call `HandleTruncatedRead`

**Missing archive** (error code 2):

1. Call `HandleMissingArchive`

### HandleTruncatedRead

1. Log: "Truncated read for key '%s'. Requested: %u Read: %u."
2. Call `GetAllocatedSpan(key, 0, &span)` to retrieve the full
   allocation for this key
3. If span retrieval fails: log "Failed to get allocated span after
   truncated read." and return
4. Compute the non-resident range:
   - Start: `original_offset + actual_bytes_read`
   - Size: `allocated_size - start`
5. Call core residency update with the non-resident span and
   `is_header=0` (data type)
6. The core update function (`sub_72ace3`) converts this to status
   byte 7 (DATA_NON_RESIDENT) via `(0 ^ 1) + 6 = 7`
7. If residency update fails: log "Failed to update residency after
   truncated read."

### HandleMissingArchive

1. Log: "Missing archive detected for offset '%llu'"
2. Delete the key from the container index via
   `DeleteKey(residency, key, 1, 0)`
3. If delete fails: log "Failed to delete key %s after missing
   archive detected."

Missing archives are unrecoverable -- the key is removed entirely,
unlike truncated reads which only degrade residency.

### Core Residency Update

The core residency update function (`sub_72ace3`):

1. Check state != 2 (ERROR_BAD_FORMAT if in bad state)
2. Acquire writer lock at offset 0x80
3. Compute bucket hash via `sub_72b457(key, section)`
4. Acquire per-bucket SRW lock at `0x6CC + (bucket << 2)`
5. Load/validate bucket data
6. Compute status byte: `(is_header ^ 1) + 6`
   - `is_header=0` (data) -> byte 7
   - `is_header=1` (header) -> byte 6
7. Call `ContainerIndex::UpdateKeyMapping(index, bucket, key_data,
   status, span, null)`
8. Release locks in reverse order

### UpdateResidency Wrapper

`Dynamic::UpdateResidency` (0x71a09f):

1. Check residency container is valid (non-null and has bound
   container at offset 0x48)
2. Adjust span offset by +0x1E (30 bytes for segment header)
3. Copy span data (offset_low, offset_high, encoded_size)
4. Call core residency update with `is_header=1`

### Error Logging

Two dedicated logging functions:

- `LogDataResidencyError` (0x719693): "Failed to update data
  residency for handle key '%s'. Error (%u): %s" (line 0x62)
- `LogHeaderResidencyError` (0x7196ea): "Failed to update header
  residency for handle key '%s'. Error (%u): %s" (line 0x59)

## Residency Container

The Residency container tracks which content keys are fully or
partially downloaded. It uses a KMT V8 index (40-byte entries) for
persistence and a MurmurHash3-based hash table for fast lookups.

Source: `casc::Residency::Open` (0x7218d5),
`casc::Residency::CheckResidency` (0x720632),
`casc::Residency::UpdateResidency` (0x721734),
`casc::Residency::ScanKeys` (0x7214a0),
`casc::Residency::DeleteKeys` (0x721c33),
`casc::Residency::BindContainerIndex` (0x721561).

### Object Layout

Residency object size: 0x6C0 (1728) bytes, alignment 8.

| Offset | Field |
|--------|-------|
| 0x08 | Path (SSO string, inline or heap) |
| 0x23 | Key size (set to 0x1B = 27) |
| 0x24 | Access mode (1=create, 2=readonly, 3=readwrite) |
| 0x40 | LooseIndex sub-object (KMT V8 index) |
| 0x48 | Initialized flag (non-null = valid) |
| 0x70 | WriterLock (SRW lock) |
| 0x74 | Reader SRW lock (for CheckResidency) |

### Access Modes

| Mode | Behavior |
|------|----------|
| 1 | Create: cleans directory if exists, creates `.residency` token |
| 2 | ReadOnly: requires `.residency` token file |
| 3 | ReadWrite: requires `.residency` token file |
| 4 | Invalid: returns error 5 immediately |
| 5 | Shared memory path |

### Open Sequence

`Residency::Open`:

1. Reject mode 4 (returns 5)
2. Check `ShouldUseSharedMemory(path, mode)` for shared memory path
3. For modes 2/3: verify `.residency` token file exists (error 0x12 if
   missing), verify directory exists (error 0x12 if missing)
4. Check drive type via `GetDriveTypeForPath` (error 0x0D for
   unsupported drives)
5. For mode 1: clean directory if exists via `CleanDirectory`
6. Allocate 0x6C0-byte object, set key size to 0x1B (27)
7. Call `BindContainerIndex` to initialize the LooseIndex
8. Create `.residency` token file (non-readonly modes)

### Token File

A `.residency` file in the container directory serves as a lock/marker.
Presence indicates a valid residency container exists. Created during
`Open` for writable modes, checked during `Open` for readonly/readwrite
modes.

### KMT V8 Storage

The residency data is stored in KMT V8 format (distinct from the V7
format used for `.idx` files):

- 16 buckets, each with its own SRW lock
- Bucket tables at offset 0x120 from LooseIndex base, each 0x50 bytes
- Per-bucket SRW locks at offset 0x620 from LooseIndex base
- Entry size: 0x28 (40) bytes

**KMT V8 entry format (40 bytes)**:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 4 | Hash/flags (XOR hash of key, bit 31 set = valid) |
| 0x04 | 16 | EKey (full 16-byte encoding key) |
| 0x14 | 16 | Residency span (4 x int32: offset_lo, offset_hi, size_lo, size_hi) |
| 0x24 | 1 | Update type byte |

Pages are 1024 bytes with 25 entries per page. Flushed to disk via
`FlushViewOfFile` with 0x1000-byte granularity every 4th bucket page
(`bucket & 3 == 3`).

### Bucket Hash (V8)

Bucket selection uses SSE instructions: XOR all 16 key bytes into 4x
int32 via `_mm_unpacklo_epi8` / `_mm_unpacklo_epi16`, fold to 32 bits,
then `(result >> 4 ^ result) & 0xF`. Same final mask as V7 bucket hash.

### CheckResidency

`Residency::CheckResidency`:

1. Validate object via initialized flag
2. Acquire reader SRW lock at offset 0x74
3. Hash key via MurmurHash3 finalizer (constants:
   `0xff51afd7_ed558ccd` and `0xc4ceb9fe_1a85ec53`)
4. Mask hash by `(bucket_count - 1)` for slot index
5. Walk linked-list chain comparing keys
6. Release SRW lock, return true/false

### Update Types

| Type | Meaning | Behavior |
|------|---------|----------|
| 0 | Invalid | Returns error 1 |
| 1-2 | Set/Create | Error 0xC if entry exists |
| 3 | Delete/Tombstone | Success if entry exists, error 2 if not |
| 6 | Mark resident | Must exist, validates span, sets flag=1 |
| 7 | Mark non-resident | Must exist, validates span, sets flag=0 |

### ScanKeys

`Residency::ScanKeys`:

1. First pass: count all keys across 16 buckets
2. Resize output arrays to fit
3. Second pass: populate key and residency value arrays
4. If `actual_count < count`: "Short scan due to another process"
   (another process modified index during scan), resize to actual

### DeleteKeys

`Residency::DeleteKeys`:

1. Validate, check not read-only (error 5 if mode 2)
2. If count < 10,000: normal path with WriterLock + per-bucket locks
3. If count >= 10,000: batch delete path
4. For each key: compute bucket, acquire SRW lock, issue update type 3

### Byte-Span Tracking

Partial downloads are tracked via byte spans stored in the KMT V8
update entries using status bytes 6 (header non-resident) and 7
(data non-resident). The span encodes the non-resident range within
the key's storage allocation. The residency value is a 64-bit packed
bitfield (two big-endian 32-bit integers) with variable-width fields
for file offset, header size, data size, total size, and flags.

## Hard Link Container / TrieDirectory

The TrieDirectory is an on-disk content-addressable storage system
that maps EKeys to files on the filesystem. It uses a two-level
directory trie derived from the hex-encoded EKey, with NTFS hard
links for deduplication. An LRU-based file descriptor cache
(`TrieDirFdCache`) manages open file handles.

Source: `TrieDirectory::Open` (0x7224f0),
`TrieDirectory::WriteFileData` (0x722266),
`TrieDirectory::DeleteKeys` (0x7227a4),
`TrieDirFdCache::CreateHardLink` (0x742579),
`TrieDirUtil::ResolvePath` (0x743341),
`TrieDirectoryCompactor::CompactDirectory` (0x7311d7).

### Object Layout

TrieDirectory object size: 0xB60 bytes, alignment 8.

| Offset | Field |
|--------|-------|
| 0x04 | String flags (bit 0 = inline SSO) |
| 0x08 | Root path (inline or pointer) |
| 0x24 | Open mode (1=create, 2=readonly, 3=readwrite) |
| 0x40 | LooseIndex / container index sub-object |
| 0x6C0 | TrieDirectoryStorage sub-object |

### Disk Layout

```text
<container_root>/
  .trie_directory          # Sentinel token file
  *.idx                    # Index files (preserved during clean)
  shmem*                   # Shared memory files (preserved during clean)
  XX/                      # First hex byte of EKey (00-ff)
    YY/                    # Second hex byte of EKey (00-ff)
      ZZZZ...ZZZZ          # Remaining hex-encoded EKey (32 chars)
```

### Path Resolution

`TrieDirUtil::ResolvePath`:

1. Compute required length via `MultiByteToWideChar` (codepage 65001,
   UTF-8)
2. Validate total length < 0x30C (780 bytes)
3. Reserve 0x26 (38) bytes for trie path suffix
4. Normalize path: lowercase, resolve `.` and `..`, normalize
   separators

`FormatContentKeyPath` formats EKey as `XX/YY/remaining_hex`:
- `XX` = hex of byte 0 (2 chars)
- `YY` = hex of byte 1 (2 chars)
- Remaining = hex of bytes 2+ (32 chars)
- Total suffix: 38 characters (0x26)

Path buffer size: 780 bytes (0x30C) throughout the codebase.

### Open Sequence

`TrieDirectory::Open`:

1. Reject mode 4 (returns 8)
2. Check `ShouldUseSharedMemory` for drive support (error 0x0D for
   unsupported drives)
3. For modes 2/3: verify `.trie_directory` token file (error 0x12)
4. For mode 1: clean directory if exists
5. Allocate 0xB60-byte object
6. Call `InitStorage` (initializes TrieDirectoryStorage at offset
   0x6C0)
7. Call `BindContainerIndex` (initializes LooseIndex at offset 0x40)
8. Create `.trie_directory` token file (create mode)

### CleanDirectory

Static method that removes all files and subdirectories except:
- Files ending with `.idx` (preserved)
- Files starting with `shmem` (preserved)

Subdirectories are removed recursively. Returns 0 on success, 8 on
error.

### WriteFileData

1. If data size is 0, returns immediately
2. Delegates to `TrieDirectoryStorage::WriteFileData` at offset 0x6C0
3. On success with residency update flag: updates container index at
   offset 0x40 with the storage span

### DeleteKeys

Two-phase operation:

1. `CollectUnlinkedKeys`: For each key, check hard link count
   - Link count <= 1: add to "unlinked" collection (safe to delete)
   - Link count > 1: file has other references, only unlink
2. Remove unlinked keys from container index
3. Remove actual files from storage for unlinked keys

### Hard Links

Uses `CreateHardLinkA` (Win32 API) for deduplication.

`TrieDirFdCache::CreateHardLink`:

1. Resolve EKey to filesystem path
2. If file already exists, remove it (retry up to 3 times)
3. Create parent directories if needed
4. Call `CreateHardLinkA(target, source)`

Error mapping: `FILE_NOT_FOUND`/`PATH_NOT_FOUND` -> 8,
`ACCESS_DENIED` -> 0x14, `DISK_FULL` -> 7, others -> 5.

### FD Cache

The `TrieDirFdCache` is an LRU cache for open file descriptors:

| Offset | Field |
|--------|-------|
| +0x04 | Max entry count (capacity) |
| +0x08 | Hash map (EKey -> cache entry) |
| +0x1C | Current entry count |
| +0x24 | LRU linked list head/sentinel |
| +0x2C | SRW lock |
| +0x30 | Root path (resolved) |
| +0x33D | First char of root path (quick comparison) |

Cache entry (`TrieDirectoryFile`, 0x30 bytes): VTable, reference count,
async handle, sync handle, and doubly-linked list node at +0x28.

Eviction: when full, removes LRU tail entry (standard unlink from
doubly-linked list), closes file handle, removes from hash map.

### TrieDirectoryCompactor

`CompactDirectory` validates the trie structure at each depth:

- Depth 0: expects 256 two-character hex directories (`00`-`ff`)
- Depth 1: expects 256 two-character hex directories
- Depth 2: expects files with 32-character hex names

Unknown files or directories at any depth are removed. Files at leaf
level are validated against the container index; orphaned files are
deleted. Empty directories are removed after enumeration.

## LRU Manager

The LRU manager tracks access recency for eviction decisions using a
flat-file doubly-linked list with generation-based filenames and MD5
checksums.

Source: `casc::Lru::Run` (0x73060f),
`casc::Lru::EvictNext` (0x730dd6),
`casc::Lru::CheckpointToDisk` (0x730b18),
`casc::Lru::FlushToDisk` (0x731056),
`casc::Lru::LoadTable` (0x741919),
`casc::Lru::ForEachEntry` (0x730088),
`casc::Lru::Shutdown` (0x730f96),
`casc::Dynamic::InitLru` (0x7195f3).

### Object Layout

Key offsets within the Lru object:

| Offset | Field |
|--------|-------|
| +0x40 | Generation (u64, low then high) |
| +0x48 | Previous generation (u64) |
| +0x58 | Eviction size counter (u64) |
| +0x68 | Cumulative encoded size (u64) |
| +0x70 | Lock (critical section / mutex) |
| +0x7A | Shutdown flag |

### File Format

Filename: `{generation:016X}.lru` (16 uppercase hex digits, big-endian
byte-swap encoding of 64-bit generation). Total length: 20 characters.

**Header (28 bytes)**:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 2 | Version (0 or 1 accepted) |
| 0x02 | 2 | Reserved (zeroed for hash) |
| 0x04 | 16 | MD5 hash (computed with field zeroed) |
| 0x14 | 4 | MRU head index (0xFFFFFFFF = empty) |
| 0x18 | 4 | LRU tail index (0xFFFFFFFF = empty) |

**Entry (20 bytes)**:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 4 | Prev index (toward LRU tail) |
| 0x04 | 4 | Next index (toward MRU head) |
| 0x08 | 9 | Encoding key (truncated) |
| 0x11 | 1 | Flags |
| 0x12 | 2 | Padding |

Sentinel value: 0xFFFFFFFF for empty prev/next links.

### Validation

From `LoadTable` / `sub_741cf7`:

- File size >= 0x1C (28 bytes)
- `(file_size - 0x1C) % 0x14 == 0` (entries are 20-byte aligned)
- `header.version <= 1`
- MD5 matches (computed with hash field zeroed, compared as 4 x u32)
- Entry prev/next indices within bounds (< entry_count or sentinel)
- MRU head entry must have `next == 0xFFFFFFFF`
- LRU tail entry must have `prev == 0xFFFFFFFF`
- No duplicate keys (checked via hash map)

### Default Capacity

`ResetTable` allocates 0xCCCC (52,428) entries.
Total buffer: 52,428 * 20 + 28 = 1,048,588 bytes (~1 MiB).

### Generation Counter

- Generation 0 is reserved (never used)
- Wraps from `u64::MAX` to 1
- `BumpGeneration` saves current to previous, then increments

### Run Sequence

`Lru::Run`:

1. Acquire lock at +0x70
2. Load table from disk via `LoadTable`
3. Eviction loop: call `EvictNext` repeatedly until it returns false
4. Scan directory via `ScanDirectory` (cleans stale `.lru` files)
5. Call `ForEachEntry` to accumulate total encoded size (excluding
   30-byte segment header entries)
6. Release lock

### Two-Phase Disk Write

1. **FlushToDisk**: Writes entire table with MD5 hash field zeroed.
   Registers `CheckpointToDisk` as async completion callback.
2. **CheckpointToDisk**: Computes MD5 over written data, opens file,
   seeks to offset 4, writes 16-byte hash, flushes, deletes previous
   generation file.

### Eviction

`EvictNext`:

1. Call `OpenTableAndGetSize` to find current `.lru` file
2. If no file exists (return 3), stop eviction
3. Compute buffer with ~1MB slack (`file_size + 0xFFFF0`)
4. Issue async delete via shared memory IO subsystem
5. Track cumulative evicted bytes

### Shutdown

1. Bump generation
2. Flush to disk
3. Wait for async write completion via `WaitForSingleObject`

### InitLru Requirements

`Dynamic::InitLru`:

- If LRU size limit is 0: returns null (LRU not configured)
- Rejects access modes 2 (readonly) and 4 (maintenance)
- Requires PID table (shared memory process tracking)
- Sets LRU enabled flag, creates manager object
- If initial setup mode: calls `Lru::Run` immediately

### Shared Memory Integration

The LRU subsystem uses the shared memory async IO layer for file
operations. The PID table (shared memory process tracking) is required
for initialization. `ForEachEntry` excludes entries with
`encoded_size == 30` (0x1E) because these are segment headers, not
cached data.

### IncrementalCompact

`LruManager::IncrementalCompact` bridges LRU eviction with archive
compaction:

1. Compute `size_to_free = current_size - target_size`
2. Call `Compactor::InitExtractCompact` with the free target
3. Call `Compactor::FinalizeArchiveMerge` to complete
4. Copy 0x50 bytes of stats to output buffer

## Shared Memory Protocol

Agent uses shared memory to coordinate between processes accessing
the same CASC storage. Protocol versions 4 and 5 are supported.

Source: `casc::ShmemControlBlock::OpenOrCreate` (0x736bcf),
`casc::ShmemControlBlock::Initialize` (0x736f7a),
`casc::ShmemControlBlock::BuildName` (0x736ebc),
`casc::ShmemControlBlock::Unmap` (0x73714d),
`casc::ShmemControlBlock::LogExclusiveAccessError` (0x736ae3),
`casc::ShmemControl::CreateFile` (0x747507),
`casc::ShmemControl::OpenAndMapFile` (0x747911),
`casc::ShmemControl::AcquireLockFile` (0x7476c4).

### Control Block Layout

| Offset | Size | Field |
|--------|------|-------|
| 0x000 | 4 | Protocol version (4 or 5) |
| 0x008 | 1 | Initialization flag (must be nonzero) |
| 0x108 | 4 | Free space table size (must be 0x2AB8) |
| 0x10C | 4 | Total mapped size |
| 0x150 | 4 | V5: exclusive access flags (bit 0=exclusive, bit 1=PID tracking) |
| 0x154 | 4 | V5: PID tracking presence bitmask |
| 0x158+ | varies | V5: PID tracking structure offsets |
| 0x1D8+ | varies | V5: PID tracking structure sizes |

### Version Differences

| Property | V4 | V5 |
|----------|----|----|
| Header size | 0x150 (336 bytes) | 0x154 (340) or 0x258 (600 with PID tracking) |
| Alignment | 16 bytes | 4096 bytes (page-aligned) |
| PID tracking | No | Optional (0x228 = 552 bytes) |
| Exclusive access | No | Yes (bit 0 of offset 0x150) |

Free space table size: 0x2AB8 (10,936 bytes) for both versions.

### Naming Convention

| Object | Name Pattern |
|--------|-------------|
| Shared memory name | `Global\<normalized_path>/shmem` |
| Shmem control file | `<container_path>/shmem` (on-disk) |
| Lock file | `<container_path>/shmem.lock` |
| Writer lock mutex | Wide-string of lock name |

Path normalization: backslashes to forward slashes, drive letter
lowercased, `.` and `..` resolved. Maximum path: 248 bytes (255 minus
7-byte `Global\` prefix).

### OpenOrCreate

`ShmemControlBlock::OpenOrCreate`:

1. Build shmem path: join container path with `"shmem"` subfolder
2. Dispatch on version: 4 -> V4 path, 5 -> V5 path, else error 0x0A
3. Allocate and zero the control block header
4. Call `BuildName` to generate the `Global\` prefixed name
5. Compute aligned total size (V4: 16-byte, V5: 4096-byte alignment)
6. Call `OpenAndMapFile` with the total size

### Initialize

`ShmemControlBlock::Initialize`:

1. Check drive type: network drives return error 0x0D
2. Call `OpenOrCreate`
3. Validate protocol version (4 or 5, else error 5)
4. Validate free space table format (`0x2AB8`, else error 5)
5. V5: check exclusive access flag (bit 0 at offset 0x150)
   - If set: log `LogExclusiveAccessError`, return 0x0B
6. Validate initialization (total mapped size and init flag nonzero)

### File Operations

`ShmemControl::CreateFile`:

1. Convert path to wide string (UTF-8 codepage 65001)
2. Delete any stale file
3. Retry loop (up to 10 attempts):
   - Try `CreateFileW` with `CREATE_NEW` + `FILE_ATTRIBUTE_TEMPORARY`
   - On failure: try `OPEN_EXISTING`
   - On `ERROR_DISK_FULL`: return 7
   - Otherwise: `Sleep(0)` and retry

`ShmemControl::OpenAndMapFile`:

1. Acquire lock file via `AcquireLockFile`
2. Create/open shmem file
3. For exclusive modes: verify this process created the file
4. If new: resize file and zero contents
5. Map file: `CreateFileMappingW` + `MapViewOfFile`
6. If new: copy control block header, zero remaining space

### Lock File

`ShmemControl::AcquireLockFile`:

- Lock file path: `<shmem_path>.lock`
- Uses `FILE_FLAG_DELETE_ON_CLOSE | FILE_ATTRIBUTE_TEMPORARY`
- `FILE_SHARE_NONE` for exclusive access
- Retry with 100-second timeout
- On timeout: error 0x0B

### Writer Lock

The writer lock uses a named global mutex (separate from shmem):

- SDDL: `D:(A;;GA;;;WD)(A;;GA;;;AN)S:(ML;;NW;;;ME)`
  - Allow Generic All to Everyone and Anonymous Logon
  - Mandatory Label, No-Write-Up, Medium integrity
- Creates via `CreateMutexW` with security descriptor
- Fallback: `OpenMutexW` with `SYNCHRONIZE` access
- Up to 5 retries on `ERROR_ACCESS_DENIED`

### Network Drive Detection

`LooseIndex::ShouldUseSharedMemory`:

- NULL path: returns true (use shared memory)
- Mode 2: returns true (forces shared memory)
- Network drive (`DRIVE_REMOTE`): returns false
- Unknown drive type: returns true (default)
- Local/removable/RAM: returns true

Network drives do not support cross-process memory-mapped file
semantics via SMB/CIFS.

### Error Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Path conversion failure |
| 5 | Unsupported version or format |
| 7 | Disk full |
| 8 | Generic error |
| 0x0A | Unsupported version (not 4 or 5) |
| 0x0B | Exclusive access conflict or timeout |
| 0x0D | Network drive (not supported) |
| 0x13 | Path too long |
| 0x14 | Access denied |

# Agent.exe Comparison

Comparison of cascette-rs against the Battle.net Agent binary
(TACT 3.13.3, CASC 1.5.9) based on reverse engineering of
Agent.exe. Issues are organized by severity and category.

Based on ~829 named functions across 113 TACT and 44 CASC source files.

## Bugs

No open bugs from Agent.exe comparison. The following were fixed:

PR [#31](https://github.com/wowemulation-dev/cascette-rs/pull/31):

- EKey page end-of-page detection: now uses `espec_index == 0xFFFFFFFF`
  sentinel matching Agent.exe, with zero-fill fallback
- Root V4 content flags: widened to `u64`, V4 block parsing split out
  to use `ContentFlags::read_v4()`/`write_v4()` for 5-byte flags
- Root version detection heuristic: tightened to `matches!(value2, 2..=4)`
  instead of `value2 < 10`
- EKey entry proptest: fixed size assertion (25 bytes), added missing
  `#[test]` annotations so proptest macro tests execute

PR [#32](https://github.com/wowemulation-dev/cascette-rs/pull/32):

- Encoding header: `EncodingHeader::validate()` checks all 8 fields
  (version, unk_11, ckey/ekey hash sizes, page counts, espec block size)
- ESpec table: rejects empty strings (consecutive nulls) and
  unterminated data (non-null-terminated blocks)
- Install manifest V2: supports version 2 with per-entry `file_type`
  byte
- CDN index footer: `validate_file_size()` checks expected vs actual
  file size
- Patch archive flags: `is_plain_data()` and `has_extended_header()`
  flag methods; rejects unsupported extended header flag during parsing
- TVFS header: `parse()` and `load_from_blte()` call
  `header.validate()` after reading

PR [#33](https://github.com/wowemulation-dev/cascette-rs/pull/33):

- BuildConfig: typed accessors for `size`, `vfs-root`,
  `build-playtime-url`, `build-product-espec`, `build-partial-priority`,
  and numbered `vfs-N` entries
- CdnConfig: typed accessors for `patch-file-index` and
  `patch-file-index-size`
- New `PartialPriority` type for comma-separated `key:priority` parsing

PR [#34](https://github.com/wowemulation-dev/cascette-rs/pull/34):

- TVFS VFS entry size: uses `VFS_ENTRY_DISK_SIZE` constant (22 bytes)
  instead of `size_of::<VfsEntry>()` (24 bytes due to alignment padding)
- TVFS builder path table size: uses exact `varint_size()` instead of
  hardcoded 5-byte approximation
- Root empty block parsing: skips empty intermediate blocks instead of
  breaking, EOF check handles termination
- Root TSFM endianness: stores `RootMagic` in `RootHeader`, writes
  little-endian fields for TSFM headers on round-trip
- Root module docs: added WoW-specific note

PR [#35](https://github.com/wowemulation-dev/cascette-rs/pull/35):

- Encoding table lookup: `find_encoding()`, `find_all_encodings()`,
  and `find_espec()` now use binary search on the page index for
  O(log p + e) instead of linear scan O(p * e)
- Archive group TOC hash: `ArchiveGroupBuilder` computes TOC hash
  from first keys of each data chunk instead of writing zeros

PR [#29](https://github.com/wowemulation-dev/cascette-rs/pull/29):

- Encoding entry parsing uses dynamic key sizes from header
  (`ckey_hash_size`, `ekey_hash_size`) instead of hardcoded 16
- Batch encoding lookups: `batch_find_encodings()`,
  `batch_find_all_encodings()`, `batch_find_especs()` using
  sort-and-merge algorithm matching Agent's batch functions
- Archive index builder uses configurable `key_size`,
  `offset_bytes`, `size_bytes` instead of hardcoded 16/4/4
- TOC hash corrected to `MD5(toc_keys || block_hashes)[:hash_bytes]`
  with per-block MD5 hashes, matching Agent's `BuildMergedIndex`
- Archive group builder uses last key per block for TOC (was first)
- TVFS container table reads `ekey_size` bytes from header instead
  of hardcoded 9. `ContainerEntry.ekey` changed to `Vec<u8>`
- TVFS EST (Encoding Spec Table) parsed when
  `TVFS_FLAG_ENCODING_SPEC` flag is set

Branch fix/formats-agent-comparison:

- CDN server selection: `FailoverManager` uses exponential decay
  scoring (`0.9^total_failure_weight`) with per-error-code weights
  and weighted-random selection, matching Agent.exe. Removed
  permanent server exclusion (`ServerHealth::Failed`). Per-status
  weights match `tact::HandleHttpResponse`: 500/502/503/504=5.0,
  401/416=2.5, other 5xx=1.0, 4xx/1xx/3xx=0.5, 429=0.0
- Streaming `max_redirects`: configurable (default 5), applied to
  both reqwest builder paths
- Connection parameter limitations documented in `StreamingConfig`
  and `HttpConfig` (low speed limit, receive buffer, DNS cache TTL,
  total connection pool cap)
- CDN Index Merge: `build_merged()` implements k-way merge via
  `BinaryHeap` for O(N log K) merging of pre-sorted archive indices,
  matching Agent.exe `tact::CdnIndex::BuildMergedIndex`. Also fixed
  entry field write order in `ArchiveGroupBuilder::build()` (was
  key/offset/size, corrected to key/size/offset)

## Format Parsing Issues

No open format parsing issues from Agent.exe comparison.

## Performance Issues

No open performance issues from Agent.exe comparison.

## Protocol Issues

### Connection Parameters

| Parameter | Agent | cascette-rs (non-streaming) | cascette-rs (streaming) |
|-----------|-------|----------------------------|------------------------|
| Connect timeout | 60s | 10s (intentional) | 10s |
| Request timeout | -- | 45s | 30s |
| Max connections/host | 3 | 10 (intentional) | 8 |
| Total connections | 12 | Unlimited | 100 (not a hard cap) |
| Max redirects | 5 | 5 (configurable) | 5 (configurable) |
| Low speed limit | 100 bps / 60s | Not set (reqwest limitation) | Not set (reqwest limitation) |
| Receive buffer | 256KB | Default (reqwest limitation) | Default (reqwest limitation) |
| DNS cache TTL | 300s | Default (reqwest limitation) | Default (reqwest limitation) |
| HTTP version | Forced 1.1 | 1.1 + HTTP/2 adaptive (intentional) | 1.1 + HTTP/2 |

Agent forces HTTP/1.1 for CDN downloads. cascette-rs enables
HTTP/2 by default with adaptive window sizing. `HttpConfig`
documents all intentional differences from Agent defaults.

The following Agent.exe parameters cannot be configured through
reqwest and are documented in `StreamingConfig` and `HttpConfig`:

- **Low speed limit** (100 bps / 60s): reqwest has no stall
  detection. Application-layer throughput monitoring would be
  needed.
- **Receive buffer** (256KB `SO_RCVBUF`): reqwest does not
  expose socket options.
- **DNS cache TTL** (300s): reqwest uses the system resolver.
- **Total connection pool cap** (12): reqwest only exposes
  per-host idle limits, not total active connections.

## Root File Issues

No open root file issues from Agent.exe comparison.

## TVFS Issues

No open TVFS issues from Agent.exe comparison.

## Local Storage Issues (cascette-client-storage)

The `cascette-client-storage` crate provides local CASC storage
support. Issues identified by comparing against Agent.exe (CASC 1.5.9)
reverse engineering. 11 of 19 issues resolved; 8 remain partial.

### ~~Write Path Missing Local Header~~ (Fixed)

Fixed in `881ab60`. The write path now prepends a 30-byte local header
before each BLTE entry, with reversed encoding key, size-with-header,
flags, and checksums. Both read and write paths handle the header.

### ~~Encoding Key Derivation on Write~~ (Fixed)

Fixed in `881ab60`. Encoding key is now `MD5(blte_encoded_data)`
matching Agent.exe behavior. The key is a property of the encoded
content, not the storage location.

### ~~Index Write Format Incorrect~~ (Fixed)

Fixed in `cc96203`. `save_index()` now writes IDX Journal v7 format
with guarded block headers (size + Jenkins hash), `IndexHeaderV2`,
and a second guarded block for entries.

### ~~No Jenkins Hash Validation~~ (Fixed)

Fixed in `cc96203`. Jenkins `hashlittle()` from cascette-crypto is
used for both read validation and write computation of guarded block
hashes.

### ~~No Atomic Index Commits~~ (Fixed)

Fixed in `cc96203`. Index writes use temp file + fsync + rename
with 3 retries, matching Agent's flush-and-bind pattern.

### ~~KMT Entry Size Endianness~~ (Fixed)

Fixed in `498cb2c`. The `IndexEntry.size` field was incorrectly
serialized as little-endian. Agent.exe and CascLib both use big-endian
for all 18-byte entry fields (verified via `ConvertBytesToInteger_BE`
in CascLib and BinaryNinja decompilation of `BinarySearchEKey` at
0x73aef9).

### ~~Incorrect KMT Entry Format~~ (Fixed)

Fixed in `498cb2c`. The `KmtEntry` struct was a fabricated 16-byte
LE format that did not match any Agent.exe structure. Replaced with
a re-export of `IndexEntry` (18 bytes), since the KMT and IDX are
the same file format. Documented the KMT = IDX equivalence.

### ~~Missing Segment Header Support~~ (Fixed)

Fixed in `498cb2c`. Added segment reconstruction header parsing:
480 bytes = 16 x 30-byte `LocalHeader` entries at the start of each
`.data` file. Added key generation with bucket hash targeting
(verified against `sub_72b457` and `GenerateSegmentHeaders` at
0x7293c6 via BinaryNinja).

### Missing KMT Update Section

Agent uses a two-tier LSM-tree Key Mapping Table (KMT) as the primary
on-disk structure. The KMT file header version is 7 (verified via
BinaryNinja decompilation of `casc::KeyMappingTable::WriteHeader` at
0x73a35c). The .idx files ARE the KMT files.

The sorted section is implemented (`IndexManager` read/write with
guarded blocks, Jenkins hashes, binary search). The update section
(append-only log in 512-byte pages for recent changes) is not yet
implemented. Compaction (merging update into sorted) is also pending.

A historical V5 format exists (`data.i##` filenames, 36-byte flat
header, no guarded blocks) used by Heroes of the Storm build 29049.
Agent.exe does not support V5. CascLib supports both.

#### Implementation Spec

The update section starts at a 64KB-aligned boundary after the sorted
section. It consists of 512-byte pages, each holding up to 21 entries.

**Update entry format (24 bytes)**:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 4 | Hash guard: `hashlittle(bytes[4..23], 0) \| 0x80000000` |
| 0x04 | 9 | EKey (big-endian) |
| 0x0D | 5 | StorageOffset (big-endian) |
| 0x12 | 4 | EncodedSize (big-endian) |
| 0x16 | 1 | Status byte (0=normal, 3=delete, 6=hdr-nonres, 7=data-nonres) |
| 0x17 | 1 | Padding |

An empty page has its first 4 bytes set to zero. Parsing stops at the
first empty page. Minimum section size: 0x7800 bytes (30,720 bytes).

**Search**: `SearchBothSections` binary-searches the sorted section,
then linearly scans update pages. Results are merged with update
entries taking precedence.

**Insert**: Appends to the next empty slot in the current page.
Every 8th page (when `page_index & 7 == 7`) triggers a 4KB sync.
When the update section is full, `FlushTable` merges update entries
into a new sorted section (atomic file replacement).

**Required changes**: Add `UpdateSection` struct to `IndexManager`
with page-based append, search, and flush-to-sorted compaction.

Reference: `agent-idx-file-format.md`

### Incomplete Container Index

Agent maintains a ContainerIndex with 16 buckets, supporting
frozen/thawed archive management with per-segment tracking (0x50
bytes per bucket binding). Archives can be frozen (read-only) or
thawed (writable).

cascette-rs has segment header parsing, key generation, bucket
hashing, frozen/thawed state tracking, and a working
`DynamicContainer` that coordinates `IndexManager` (KMT) with
`ArchiveManager` for read/write/remove/query operations. The
`ArchiveManager` does not yet use segment-based storage offsets
or enforce the segment limit (0x3FF = 1023).

#### Implementation Spec

**Object layout** (key byte offsets from `ContainerIndex::Open`):

| Offset | Field |
|--------|-------|
| 0x2C | Version (4 or 5) |
| 0x30 | Current segment index |
| 0x44 | Allocation SRW lock |
| 0x48 | Shared memory control block |
| 0x80 | Writer lock |
| 0xA4 | IndexTables base |
| 0x130 | KMT bindings (16 x 0x50 bytes) |
| 0x630 | Loose storage allocator |
| 0x6CC | Per-bucket SRW locks (16 x 4 bytes) |

**Open sequence**: Create directory, bind shared memory (3 retries,
20s sleep), init writer lock, init IndexTables, bind 16 KMT tables,
copy segment info, rebuild if version != 4.

**Allocation**: Try loose storage first (if available), then segment
allocator. On ERROR_CURRENT_DIRECTORY (full segment), create a new
segment and retry. Segment index extracted from offset high bits:
`offset >> 30 | offset_high << 2`.

**Segment creation**: Generate 16 header keys (one per bucket),
verify no collisions, allocate 480-byte header, write headers, call
storage callback.

**Key update**: Access per-bucket KMT at `0x130 + bucket * 0x50`.
Insert via `InsertEntryLocked`. If update section full
(ERROR_INVALID_DRIVE): flush and retry. Delete status 3 with
FILE_NOT_FOUND treated as success.

**Required changes**: Add segment-based offset encoding to
`ArchiveManager`, enforce segment limit 0x3FF, implement
`FlushTable` with shared memory handle update at `shmem + 0x110 +
bucket * 4`.

Reference: `agent-container-storage.md`

### Partial Residency Container

Agent tracks which content keys are fully downloaded via Residency
container (0x6C0 bytes): `.residency` token files, byte-span tracking
for partial downloads, reserve/mark-resident/remove/query operations,
and scanner API.

cascette-rs has in-memory residency tracking with `.residency` token
file creation. Byte-span tracking for partial downloads and
file-backed persistence are not yet implemented.

#### Implementation Spec

The Residency container uses a KMT V8 index (distinct from V7 used by
`.idx` files). Object size: 0x6C0 (1728) bytes.

**KMT V8 entry format (40 bytes)**:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 4 | Hash/flags (XOR hash, bit 31 set = valid) |
| 0x04 | 16 | EKey (full 16-byte encoding key) |
| 0x14 | 16 | Residency span (4 x int32) |
| 0x24 | 1 | Update type byte |

Pages: 1024 bytes, 25 entries per page (vs V7: 512-byte pages, 21
entries of 24 bytes). Flushed every 4th bucket page.

**Bucket hash (V8)**: SSE-based XOR of 16 key bytes, fold to 32 bits,
then `(result >> 4 ^ result) & 0xF`.

**CheckResidency**: MurmurHash3 finalizer (constants
`0xff51afd7_ed558ccd`, `0xc4ceb9fe_1a85ec53`) with reader SRW lock.
Hash masked by `(bucket_count - 1)` for slot, chain walk for match.

**Update types**: 0=invalid, 1-2=set/create, 3=delete/tombstone,
6=mark resident, 7=mark non-resident.

**ScanKeys**: Two-pass (count then populate) across 16 buckets.
Detects concurrent modification ("Short scan due to another process").

**DeleteKeys**: Threshold at 10,000 keys switches to batch path.
Normal path: WriterLock + per-bucket SRW locks, update type 3.

**Open sequence**: Validate mode, check drive type, verify token file
(modes 2/3), clean directory (mode 1), allocate 0x6C0 bytes, bind
container index, create token file.

**Required changes**: Replace in-memory HashMap with KMT V8 file-backed
storage. Add MurmurHash3 fast-path for `CheckResidency`. Implement
`ScanKeys` with two-pass iteration. Add batch delete threshold.

Reference: `agent-container-storage.md`

### Partial Hard Link Container

Agent uses a TrieDirectory with hard links for content sharing between
installations: 32-char hex filename validation, LRU file descriptor
cache, 3-retry delete before hard link creation, filesystem support
detection via `TestSupport`.

cascette-rs has filesystem hard link support detection via
`TestSupport`, link creation with 3-retry delete, and
`.trie_directory` token file. TrieDirectory-based metadata tracking
is not yet implemented.

#### Implementation Spec

TrieDirectory object size: 0xB60 bytes. Contains LooseIndex at +0x40
and TrieDirectoryStorage at +0x6C0.

**Trie disk layout**: `XX/YY/ZZZZ...ZZZZ` where XX/YY are hex of
EKey bytes 0-1, remaining 32 chars are the rest. Path suffix: 38 bytes
(0x26). Path buffer: 780 bytes (0x30C).

**FD cache** (`TrieDirFdCache`): LRU cache with hash map (EKey -> entry),
doubly-linked list, SRW lock. Entry size: 0x30 bytes (VTable, refcount,
async handle, sync handle, list node). Evicts LRU tail when full.

**DeleteKeys**: Two-phase: collect unlinked keys (link count <= 1),
then remove from index and storage. Keys with link count > 1 are only
unlinked (file remains for other references). Uses
`GetFileInformationByHandle` to read `nNumberOfLinks`.

**CleanDirectory**: Removes all files/directories except `.idx` and
`shmem*` files.

**CompactDirectory**: Validates trie at each depth (0=hex dirs,
1=hex dirs, 2=hex files). Orphaned files validated against container
index and deleted. Empty directories removed.

**Open sequence**: Reject mode 4, check drive type, verify
`.trie_directory` token (modes 2/3), clean directory (mode 1),
allocate 0xB60 bytes, init storage, bind container index, create
token file.

**Required changes**: Implement `TrieDirectoryStorage` with FD cache,
path resolution using `FormatContentKeyPath`, two-phase delete with
link count checking, and compactor integration.

Reference: `agent-container-storage.md`

### Partial Static Container

Agent supports read-only Static containers for shared installations
with batch key state lookups via `casc::Residency::GetFileState`.

cascette-rs has a StaticContainer with `IndexManager` and
`ArchiveManager` for read-only lookups, including batch
`state_lookup()` returning `KeyState { has_data, is_resident }`.

### Compaction is a Stub

`ArchiveManager::compact()` only truncates files to the write
position. Agent uses two-phase compaction: archive merge (plan merge
across segments, MurmurHash3 hash map) then extract-compact (per-segment
span validation, overlap detection, empty archive deletion). Two
algorithms: defrag (moves data to fill gaps) and fillholes (estimates
free space without moves). Async read/write pipeline with 128 KB
minimum buffer.

#### Implementation Spec

**Two modes**: Archive merge (flag=0, has external callback, async
dispatch) and extract-compact (flag=1, no callback, direct dispatch).

**Archive merge pipeline**:
1. `InitArchiveMerge`: copy defrag params, execute validation, async
   dispatch merge work
2. `OnArchiveMergeComplete`: on success -> `StartCompaction`
   (segment move phase), on cancel/error -> complete
3. `FinalizeArchiveMerge`: flush work, iterate segments for errors,
   validate container spans, record metrics

**Extract-compact pipeline**:
1. `InitExtractCompact`: set flag=1, execute, direct `StartCompaction`
2. `ProcessSegment`: validate size >= 480 bytes, validate entries >= 16,
   sort/validate spans (no overlaps), build move plan or truncate/delete
3. `DispatchWork`: dequeue segments, start file mover, handle errors

**Async pipeline** (`CompactionFileMover`):
- Buffer sizing: if total >= 128 KiB: `count = min(total >> 17, 16)`,
  per-buffer = total/count. Below 128 KiB: single buffer, log warning.
- Read/write loop: `QueueAsyncRead` -> `OnReadComplete` -> queue write
  -> `OnWriteComplete` -> loop or `ReconstructAndUpdateIndex`
- Segment offset: `segment_index * 0x40000000 + position`
- Per-move-item SRW locks for concurrency
- Single in-flight segment at a time

**Reconstruction**: After move completes, iterate entries (12-byte
stride), remove old index keys, insert with new offsets, update
residency spans. Partial span failures non-fatal (logged).

**Backup/recovery** (`ExtractorCompactorBackup`):
- File: `<data_dir>.extract_bu`, 4101 bytes, memory-mapped
- Header: version(1) + max_entries(1023) + count
- Entries: u32 segment indices, append-only, flush after every add
- On open: validate entries, remove invalid (index >= 0x3FF)

**Error sentinels**: 0=success, 0x8=truncated I/O, 0x9=validation
failure, 0xA=bad params, 0xF=cancelled (ERROR_INVALID_DRIVE).

**Required changes**: Replace truncation-only `compact()` with
two-phase pipeline. Implement `CompactionFileMover` with async
read/write loop. Add `ExtractorCompactorBackup` for crash recovery.
Add span validation and overlap detection.

Reference: `agent-maintenance-operations.md`

### Partial LRU Cache

LRU manager implemented with flat-array doubly-linked list and
hash map, matching Agent.exe's architecture. File format implemented
(28-byte header + 20-byte entries with MD5 checksum, generation-based
filenames). Persistence via checkpoint/load operations.

Not yet implemented:
- Shared memory integration (LRU table in shmem)
- Size-based eviction target (evict until under configured limit)
- Directory scanning to discover new files
- Integration with `DynamicContainer` and compactor

#### Implementation Spec

**Run sequence**: Load table -> eviction loop (`EvictNext` until false)
-> scan directory (clean stale `.lru` files) -> `ForEachEntry`
(accumulate total encoded size, excluding 30-byte segment headers).

**Two-phase disk write**:
1. `FlushToDisk`: write table with MD5 zeroed, set async callback
2. `CheckpointToDisk`: compute MD5, seek to offset 4, write 16-byte
   hash, flush, delete previous generation file

**Eviction**: `EvictNext` opens table, computes buffer with ~1MB slack
(`file_size + 0xFFFF0`), issues async delete via shmem IO. Tracks
cumulative evicted bytes at +0x58.

**IncrementalCompact**: Bridges LRU eviction to archive compaction.
Computes `size_to_free = current - target`, calls
`Compactor::InitExtractCompact`, then `FinalizeArchiveMerge`.

**InitLru requirements**: LRU size limit must be nonzero. Rejects
modes 2 (readonly) and 4 (maintenance). Requires PID table (shared
memory process tracking).

**Shutdown**: bump generation -> flush to disk -> wait for async write
completion via `WaitForSingleObject`.

**Default capacity**: `ResetTable` allocates 52,428 entries (~1 MiB).

**LoadTable validation**: File >= 28 bytes, entries 20-byte aligned,
version <= 1, MD5 match, prev/next indices valid. Second pass: remove
entries where callback returns false (key not in IDX/archive).

**Required changes**: Add shmem async IO integration for file
operations. Implement size-based eviction loop with target threshold.
Add `ScanDirectory` to clean stale generation files. Wire
`IncrementalCompact` to compactor. Add `ForEachEntry` size accounting
(skip entries with encoded_size == 30).

Reference: `agent-container-storage.md`

### Partial Shared Memory Protocol

Agent uses CASC shared memory protocol versions 4/5. The control
block layout has been implemented with BinaryNinja-verified
constants and offsets:

- V4 header (0x150 bytes, 16-byte alignment)
- V5 header (0x154/0x258 bytes, page alignment)
- Free space table format at offset 0x42 (0x2AB8 identifier)
- PID tracking with state machine (idle/modifying) and slot management
- Version validation, exclusive access checks, bind validation

Not yet implemented:

- Platform-specific shmem I/O (`shm_open`, `CreateFileMapping`)
- Writer lock via named global mutex (`Global\` prefix)
- DACL: `D:(A;;GA;;;WD)(A;;GA;;;AN)S:(ML;;NW;;;ME)`
- `.shmem.lock` file with retry logic
- Free space table read/write operations
- Network drive detection (`ShouldUseSharedMemory`)

#### Implementation Spec

**Naming**: Shmem name = `Global\<normalized_path>/shmem`. Lock file
= `<shmem_path>.lock`. Path normalization: lowercase drive, forward
slashes, resolve `.`/`..`. Max path: 248 bytes (255 - 7 prefix).

**OpenOrCreate**: Build shmem path (join with "shmem"), dispatch on
version (4 or 5, else error 0x0A), allocate/zero control block header,
call `BuildName`, compute aligned size, call `OpenAndMapFile`.

**Initialize**: Check drive type (network -> 0x0D), call OpenOrCreate,
validate version (4-5), validate free space table (0x2AB8), V5:
check exclusive access flag (bit 0 at offset 0x150, error 0x0B),
validate initialization (mapped size and init flag nonzero).

**File creation** (`CreateFile`): UTF-8 to wide conversion, delete
stale file, retry loop (10 attempts) with `CREATE_NEW` +
`FILE_ATTRIBUTE_TEMPORARY`, fallback to `OPEN_EXISTING`, `Sleep(0)`
between retries.

**File mapping** (`OpenAndMapFile`): Acquire lock file, create/open
shmem file, exclusive mode check (must be creator), resize if new,
`CreateFileMappingW` + `MapViewOfFile` with allocation granularity
alignment, `DuplicateHandle` for reference, copy init data if new.

**Lock file**: `FILE_FLAG_DELETE_ON_CLOSE | FILE_ATTRIBUTE_TEMPORARY`,
`FILE_SHARE_NONE`, 100-second timeout with timed retry, error 0x0B on
timeout.

**Writer lock mutex**: SDDL `D:(A;;GA;;;WD)(A;;GA;;;AN)S:(ML;;NW;;;ME)`.
`CreateMutexW` with security descriptor, fallback `OpenMutexW` with
`SYNCHRONIZE`, 5 retries on `ERROR_ACCESS_DENIED`.

**Network drive detection**: `GetVolumePathNameW` + `GetDriveTypeW`.
`DRIVE_REMOTE` -> disable shmem. NULL path or mode 2 -> force shmem.
Unknown drive -> default to shmem.

**Error codes**: 0=success, 5=bad version/format, 7=disk full,
8=generic, 0x0A=unsupported version, 0x0B=exclusive conflict/timeout,
0x0D=network drive, 0x13=path too long, 0x14=access denied.

**Required changes**: Implement platform-specific file mapping
(`CreateFileMappingW`/`MapViewOfFile` on Windows, `shm_open`/`mmap`
on Unix). Implement lock file with timeout retry. Implement writer
lock mutex with DACL. Add network drive detection. Wire free space
table operations.

Reference: `agent-container-storage.md`

### ~~Directory Structure~~ (Fixed)

Agent.exe creates five subdirectories under the storage root:
`data/` (dynamic container), `indices/` (CDN index cache),
`residency/` (residency tracking DB), `ecache/` (e-header cache),
`hardlink/` (hard link container trie).

cascette-rs now matches this layout. Build/CDN configs are stored
inside the dynamic container. Shared memory uses named kernel
objects + a temp file in `data/`. The incorrect `config/` and
`shmem/` directories have been removed.

### ~~Bucket Algorithm Documentation Error~~ (Fixed)

The `local-storage.md` doc previously stated `bucket = key[0] & 0x0F`.
The actual algorithm (correctly implemented in
`IndexManager::get_bucket_index`) is:

```text
hash = key[0] ^ key[1] ^ ... ^ key[8]
bucket = (hash & 0x0F) ^ (hash >> 4)
```

Corrected in the docs and verified against BinaryNinja decompilation
of `sub_72b457`.

### ~~No .build.info Handling~~ (Fixed)

Fixed in `fa741c6`. `BuildInfoFile` parses `.build.info` (BPSV format)
with typed accessors for product, branch, build key, CDN key, version,
CDN hosts, CDN servers, install key, tags, and armadillo. Async
`from_path()` reads from disk; `active_entry()` returns the first row
with `Active == 1`.

### Partial Truncation Tracking

Agent tracks truncated reads via key state and marks content as
non-resident when a read returns fewer bytes than expected (CASC
error 3 -> TACT error 7). `DynamicContainer::read()` detects
archive bounds errors and returns `StorageError::TruncatedRead`,
matching Agent's error code mapping.

Not yet implemented: automatic residency state update on truncated
reads (marking the key non-resident in the `ResidencyContainer`).

#### Implementation Spec

The truncation chain in Agent (`Dynamic::Read` at 0x71a564):

1. `ReadFromArchive` returns success but `bytes_read < expected`
2. Bump telemetry: `"dynamic_container.read_truncated.count"`
3. Adjust offset by +30 bytes (segment local header size)
4. `HandleTruncatedRead`:
   - Get full allocated span via `GetAllocatedSpan(key)`
   - Compute non-resident range: start = `offset + bytes_read`,
     size = `allocated_size - start`
   - Update KMT entry with status byte 7 (DATA_NON_RESIDENT)
   - Status encoding: `(is_header ^ 1) + 6` (data=7, header=6)

Missing archives (error 2) trigger key deletion instead:
`HandleMissingArchive` removes the key from the container index
entirely.

**Concurrency**: Core residency update acquires writer lock (0x80),
then per-bucket SRW lock at `0x6CC + (bucket << 2)`. Fine-grained
locking allows concurrent updates to different buckets.

**Required changes**: In `DynamicContainer::read()`, after detecting
`StorageError::TruncatedRead`, call `ResidencyContainer::mark_non_resident(key, non_resident_span)`. For missing archives, call
`IndexManager::remove(key)`.

Reference: `agent-container-storage.md`

## Not Implemented

These features exist in Agent.exe but have no cascette-rs
equivalent. They are documented in the reverse engineering
docs for future implementation.

### Containerless Mode

Agent stores game files individually on disk instead of packed
into CASC archives. This is the code path for fresh game
installations. 13 state machines, 17 source files, 15 RTTI
classes.

Required components:

- In-memory SQLite database with meta/tags/files tables
- Block mover for content transfer between file versions
- E-header cache for batch CDN downloads
- File identification via hash comparison

Reference: `agent-containerless-mode.md`

### Garbage Collection

4-stage pipeline:

1. BuildPreservationFilter: collect keys from active builds
2. GarbageCollectorFilter: mark unreferenced data
3. Compaction: defrag or fillholes algorithm
4. CleanupDirectory: remove empty archive files

Reference: `agent-maintenance-operations.md`

### Build Repair

5-stage pipeline using marker files (RepairMarker.psv) for
crash recovery:

1. ReadBuildConfig
2. InitCdnIndexSet
3. RepairContainers (data, ecache, hardlink sequentially)
4. RepairHardLinks
5. PostRepairCleanup

Reference: `agent-maintenance-operations.md`

### Build Update Orchestration

9-state machine (most complex in the binary):

1. ReadBaseBuildConfig
2. ClassifyArtifacts
3. ProcessPatchIndex
4. FilterLooseFiles
5. ClassifyLooseFiles
6. ParseDownloadManifest
7. FilterContainerKeys
8. FetchPatchHeaders
9. Finalize

File classification values: 0=current, 1=needs download,
2=needs patch, 5=special, 6=obsolete.

Reference: `agent-build-update-flow.md`

### Patch Operations

Three patch types not implemented:

- Block patching (Op 3): block-level differential updates
- Decryption patching (Op 4): key rotation patches
- Re-encode patching (Op 5): re-encode content after patch

Reference: `agent-async-state-machines.md`

### Download Telemetry

Agent sends structured telemetry to
`https://telemetry-in.battle.net/data` including server
performance metrics, download throughput, fallback events, and
build update status. cascette-rs collects internal metrics
(for library consumers) but does not transmit telemetry.

## Validated Correct

These cascette-rs implementations match Agent.exe behavior:

| Feature | Agent Function | cascette-rs |
|---------|---------------|-------------|
| Encoding header fields | `ParseHeader` | `EncodingHeader` field-for-field match |
| CKey page entry format | Page parser | `CKeyPageEntry` with key_count, 40-bit size |
| CKey end-of-page | `key_count == 0` sentinel | Same sentinel check |
| Page checksum verification | MD5 validation | Checksum verified per page |
| CDN index footer (7 of 7) | `CdnIndexFooterValidator` | Version, hash size, block size, element size, reserved, footer hash bytes, footer hash |
| CDN index entry format | Variable-length keys | 4/5/6-byte offset support |
| Archive group format | 6-byte offsets | Archive index + offset parsing |
| BLTE magic and header | E-header parser | "BLTE" magic, single-chunk mode |
| BLTE chunk table | Flags 0x0F, 24-bit count | Chunk entries with MD5 checksums |
| BLTE block codecs | N/Z/4/E/F dispatch | All 5 compression modes |
| LZ4 size prefix | 8-byte LE decompressed size | Matches exactly |
| Encryption format | Salsa20 + ARC4 | Key name, IV, type byte, tau constant, IV extension |
| ESpec grammar | n/z/c/e/b/g letters | All 6 letters with correct parameters |
| ESpec block sizes | *=NNNNK/M notation | Numeric, K, M multipliers, * notation |
| BCPack/GDeflate | Stubs in Agent | Parse-only in cascette-rs (correct) |
| Download manifest V1-V3 | DL magic, 40-bit sizes | All version differences handled |
| Size manifest V1-V2 | DS magic, V1 variable width | V1 configurable esize\_bytes, V2 fixed 4-byte |
| PSV format | Header/data/metadata lines | Pipe-delimited with type info |
| Config parsing | Build + CDN config | All fields stored in HashMap with typed accessors |
| CDN URL construction | host/path/type/XX/YY/hex | Path splitting matches |
| element\_count endianness | LE in BE format | Correctly handled |
| EKey end-of-page | `espec_index == 0xFFFFFFFF` sentinel | Same sentinel + zero-fill fallback |
| Root V4 content flags | 40-bit (5-byte) flags | `ContentFlags::read_v4()`/`write_v4()` |
| Root version detection | Header size + version field | `matches!(value2, 2..=4)` heuristic |
| Root file V1-V4 | MFST/TSFM magic | Interleaved and separated formats |
| TVFS header validation | `format_version`, key sizes | `TvfsHeader::validate()` called on parse |
| TVFS path table | Prefix tree with varints | LEB128-like encoding |
| TVFS container table | Variable EKey entries | `ekey_size` from header, `Vec<u8>` EKey + file\_size + optional CKey |
| Encoding header validation | 8-field check | `EncodingHeader::validate()` all fields |
| ESpec table validation | Null-terminated, no empty | Rejects empty strings and unterminated data |
| Install manifest V1+V2 | Version 1-2, file\_type byte | `InstallFileEntry::file_type` for V2 |
| CDN index file size | Expected vs actual size | `IndexFooter::validate_file_size()` |
| Patch archive flag bits | Bit 0/1 dispatch | `is_plain_data()`, `has_extended_header()` |
| Build config accessors | 22+ typed fields | `size()`, `vfs_root()`, `vfs_entries()`, `build_partial_priority()`, `build_playtime_url()`, `build_product_espec()` |
| CDN config patch indices | `patch-file-index` fields | `patch_file_index()`, `patch_file_index_size()`, `patch_file_indices()` |
| ZBSDIFF1 header endianness | Big-endian int64 sizes | `#[br(big)]` matches TACT conventions; "ZBSDIFF1" signature is big-endian 0x5A42534449464631 |
| TVFS VFS entry disk size | 22-byte entries | `VFS_ENTRY_DISK_SIZE` constant, not `size_of` (24 bytes with padding) |
| TVFS path table varint sizes | Variable-length encoding | `varint_size()` for exact calculation |
| Root MFST/TSFM endianness | Magic determines byte order | `RootMagic` stored and preserved on round-trip |
| Root empty block handling | Skip empty, continue parsing | EOF-based termination, empty blocks do not truncate |
| Root format scope | WoW-specific root format | Module docs note WoW-specific nature |
| Encoding page lookup | `PageBinarySearch` O(log p) | `partition_point` on page index `first_key` |
| Archive group TOC hash | `MD5(toc_keys \|\| block_hashes)[:hash_bytes]` | `calculate_toc_hash()` with per-block MD5 hashes, last key per block |
| Encoding dynamic key sizes | `ckey_hash_size`/`ekey_hash_size` from header | `CKeyPageEntry` and `EKeyPageEntry` use header sizes via BinRead Args |
| Encoding batch lookups | `BatchLookupCKeys`/`BatchLookupEKeys` | `batch_find_encodings()`, `batch_find_all_encodings()`, `batch_find_especs()` |
| Archive index builder config | Variable key/offset/size fields | `ArchiveIndexBuilder::with_config(key_size, offset_bytes, size_bytes)` |
| TVFS EST parsing | EST table when flag bit 1 set | `EstTable` with null-terminated strings, parsed from header offsets |
| CDN URL trailing slash | `cdnPath` trailing slash stripped | `normalize_cdn_path()` strips trailing slashes before URL construction |
| Retry-After header | 429 response reads `Retry-After` | `RateLimited { retry_after }` variant, `parse_retry_after()` in CDN client, `RetryPolicy` uses hint |
| CDN URL parameters | `ParseCdnServerUrl` parses `?fallback=1`, `?strict=1`, `?maxhosts=N` | `parse_cdn_server_url()` extracts params; `CdnEndpoint` and `CdnServer` store parsed fields |
| Max redirects | 5 redirect limit | `HttpConfig::max_redirects` (default 5), `StreamingConfig::max_redirects` (default 5), both reqwest builders use configured value |
| CDN server scoring | `0.9^total_failures` decay, weighted-random selection | `FailoverManager` uses `total_failure_weight` with per-error-code weights matching `tact::HandleHttpResponse` (500/502/503/504=5.0, 401/416=2.5, other 5xx=1.0, 4xx/1xx/3xx=0.5, 429=0.0), `0.9^weight` decay, cumulative-weight random selection. No permanent server exclusion |
| CDN index k-way merge | `BuildMergedIndex` with `HeapSiftDown`/`HeapSiftUp` | `build_merged()` using `BinaryHeap` for O(N log K) merge with deduplication |
| Archive group entry order | key, size, offset (standard CDN index format) | `build()` and `build_merged()` write key/size/offset matching `IndexEntry::to_bytes` |
| China region CDN | `.com.cn` domains for CN region | `Region` enum with `CN` and `SG` variants, `tact_https_url()`, `tact_http_url()`, and `ribbit_address()` return per-region domains |

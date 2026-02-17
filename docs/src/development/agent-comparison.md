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

The `cascette-client-storage` crate provides initial local CASC storage
support. The following issues were identified by comparing against
Agent.exe (CASC 1.5.9) reverse engineering.

### ~~Write Path Missing Local Header~~ (Fixed)

Fixed in `ebbf055`. The write path now prepends a 30-byte local header
before each BLTE entry, with reversed encoding key, size-with-header,
flags, and checksums. Both read and write paths handle the header.

### ~~Encoding Key Derivation on Write~~ (Fixed)

Fixed in `ebbf055`. Encoding key is now `MD5(blte_encoded_data)`
matching Agent.exe behavior. The key is a property of the encoded
content, not the storage location.

### ~~Index Write Format Incorrect~~ (Fixed)

Fixed in `07591c4`. `save_index()` now writes IDX Journal v7 format
with guarded block headers (size + Jenkins hash), `IndexHeaderV2`,
and a second guarded block for entries.

### ~~No Jenkins Hash Validation~~ (Fixed)

Fixed in `07591c4`. Jenkins `hashlittle()` from cascette-crypto is
used for both read validation and write computation of guarded block
hashes.

### ~~No Atomic Index Commits~~ (Fixed)

Fixed in `07591c4`. Index writes use temp file + fsync + rename
with 3 retries, matching Agent's flush-and-bind pattern.

### ~~KMT Entry Size Endianness~~ (Fixed)

Fixed in Phase 4+5. The `IndexEntry.size` field was incorrectly
serialized as little-endian. Agent.exe and CascLib both use big-endian
for all 18-byte entry fields (verified via `ConvertBytesToInteger_BE`
in CascLib and BinaryNinja decompilation of `BinarySearchEKey` at
0x73aef9).

### ~~Incorrect KMT Entry Format~~ (Fixed)

Fixed in Phase 4+5. The `KmtEntry` struct was a fabricated 16-byte
LE format that did not match any Agent.exe structure. Replaced with
a re-export of `IndexEntry` (18 bytes), since the KMT and IDX are
the same file format. Documented the KMT = IDX equivalence.

### ~~Missing Segment Header Support~~ (Fixed)

Fixed in Phase 4+5. Added segment reconstruction header parsing:
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
(append-only log in 0x1000-byte pages for recent changes) is not yet
implemented. Compaction (merging update into sorted) is also pending.

A historical V5 format exists (`data.i##` filenames, 36-byte flat
header, no guarded blocks) used by Heroes of the Storm build 29049.
Agent.exe does not support V5. CascLib supports both.

### Incomplete Container Index

Agent maintains a ContainerIndex with 16 segments, supporting
frozen/thawed archive management with per-segment tracking (0x40
bytes per segment). Archives can be frozen (read-only) or thawed
(writable).

cascette-rs has segment header parsing, key generation, bucket
hashing, frozen/thawed state tracking, and a working
`DynamicContainer` that coordinates `IndexManager` (KMT) with
`ArchiveManager` for read/write/remove/query operations. The
`ArchiveManager` does not yet use segment-based storage offsets
or enforce the segment limit (0x3FF = 1023).

### Partial Residency Container

Agent tracks which content keys are fully downloaded via Residency
container (0x30 bytes): `.residency` token files, byte-span tracking
for partial downloads, reserve/mark-resident/remove/query operations,
and scanner API.

cascette-rs has in-memory residency tracking with `.residency` token
file creation. Byte-span tracking for partial downloads and
file-backed persistence are not yet implemented.

### Partial Hard Link Container

Agent uses a TrieDirectory with hard links for content sharing between
installations: 32-char hex filename validation, LRU file descriptor
cache, 3-retry delete before hard link creation, filesystem support
detection via `TestSupport`.

cascette-rs has filesystem hard link support detection via
`TestSupport`, link creation with 3-retry delete, and
`.trie_directory` token file. TrieDirectory-based metadata tracking
is not yet implemented.

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

### Directory Structure Divergence

Agent stores both `.idx` and `.data` files in `Data/data/`. There is
no separate `indices/`, `config/`, or `shmem/` directory at the
storage root.

cascette-rs creates four directories (`indices/`, `data/`, `config/`,
`shmem/`). The `Installation` module correctly points both index and
archive managers at the `data/` directory, but the top-level `Storage`
creates the extra directories which do not match the official layout.

### Bucket Algorithm Documentation Error

The `local-storage.md` doc previously stated `bucket = key[0] & 0x0F`.
The actual algorithm (correctly implemented in
`IndexManager::get_bucket_index`) is:

```text
hash = key[0] ^ key[1] ^ ... ^ key[8]
bucket = (hash & 0x0F) ^ (hash >> 4)
```

This has been corrected in the docs.

### No .build.info Handling

Agent reads `.build.info` (BPSV format) for installation metadata
including product code, region, build config hash, and CDN config
hash. cascette-rs does not parse or generate this file.

### Content Read Missing Truncation Tracking

Agent tracks truncated reads via key state and marks content as
non-resident when a read returns fewer bytes than expected (CASC
error 3 → TACT error 7). cascette-rs returns an error on short
reads but does not update any residency state.

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

# Agent.exe Comparison

Comparison of cascette-rs against the Battle.net Agent binary
(TACT 3.13.3, CASC 1.5.9) based on reverse engineering of
Agent.exe. Issues are organized by severity and category.

Source: [management/docs/reverse-engineering/](https://github.com/wowemulation-dev/management/tree/main/docs/reverse-engineering)
covering ~829 named functions across 113 TACT and 44 CASC source files.

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

## Format Parsing Issues

### Encoding Table Hardcoded Key Size

cascette-rs hardcodes key size to 16 bytes in `IndexEntry`,
`CKeyPageEntry`, and `EKeyPageEntry`, ignoring the header's
`ckey_hash_size` and `ekey_hash_size` fields. This works for all
known CASC data but diverges from Agent's flexible key size
support. Making key size dynamic requires changing the crypto
types (`ContentKey`/`EncodingKey` are `[u8; 16]` wrappers),
threading sizes through BinRead Args, and updating all consumers.

## Performance Issues

### Encoding Table Batch Lookups

Agent provides 4 batch lookup functions
(`BatchLookupCKeys`, `BatchLookupCKeyPages`, `BatchLookupEKeys`,
`BatchLookupEKeysSorted`) that sort input keys and scan pages in
a single pass.

cascette-rs has no batch lookup API. Individual lookups use binary
search on the page index (O(log p + e)), but batch operations
would be more efficient for bulk resolution.

### CDN Index Merge

Agent implements k-way merge sort via binary min-heap
(`HeapSiftDown`/`HeapSiftUp`) for combining multiple CDN indices.
This is O(N log K) where K is the number of indices, with
per-block MD5 validation computed during the merge.

cascette-rs `ArchiveGroupBuilder` uses HashMap deduplication +
final sort: O(N log N). Per-block page hashes are not computed
during building. TOC hash and footer hash are computed correctly.

### Archive Index Builder Hardcoded Key Size

`ArchiveIndexBuilder::build()` hardcodes 16-byte keys and 4-byte
offsets. Cannot generate 9-byte truncated key indices used by
local CASC storage.

## Protocol Issues

### CDN Server Selection

Agent uses exponential decay scoring for server selection:

```text
weight = 0.9 ^ total_failures
selection = randomized linear interpolation across scored servers
```

Failed servers get progressively lower weights but are never fully
excluded. HTTP response codes carry different backoff weights:

| Code | Category | Backoff Weight |
|------|----------|---------------|
| 200-299 | Success | 0 |
| 429 | Rate limited | Reads Retry-After header |
| 503 | Unavailable | 5.0 |
| 404 | Not found | 2.5 |
| Other 4xx/5xx | Error | 1.0 |

cascette-rs has two CDN implementations:

- **Non-streaming** (`cdn/mod.rs`): Single configured host, no
  server scoring, plain exponential backoff retry.
- **Streaming** (`cdn_streaming/`): `FailoverManager` with
  per-server metrics (success rate, response time, bandwidth).
  Uses fixed unavailability windows (5 min for timeout, 15 min
  for 5xx) instead of decay scoring. Servers can be permanently
  marked `Failed`, unlike Agent which never fully excludes
  servers.

### Retry-After Header

Agent reads the HTTP `Retry-After` header on 429 responses and
waits the specified duration before retrying.

cascette-rs defines a `RateLimitExceeded` error with a
`retry_after_ms` field, but no code reads the `Retry-After` header
from HTTP responses. Both the non-streaming and streaming CDN
clients ignore this header.

### Connection Parameters

| Parameter | Agent | cascette-rs (non-streaming) | cascette-rs (streaming) |
|-----------|-------|----------------------------|------------------------|
| Connect timeout | 60s | 10s | 10s |
| Request timeout | -- | 45s | 30s |
| Max connections/host | 3 | 10 | 8 |
| Total connections | 12 | Unlimited | 100 |
| Max redirects | 5 | 3 | Default (10) |
| Low speed limit | 100 bps / 60s | Not set | Not set |
| Receive buffer | 256KB | Default | 64KB |
| DNS cache TTL | 300s | Default | Default |
| HTTP version | Forced 1.1 | 1.1 + HTTP/2 adaptive | 1.1 + HTTP/2 |

Agent forces HTTP/1.1 for CDN downloads. cascette-rs enables
HTTP/2 by default with adaptive window sizing.

### CDN URL Parameters

Agent `tact::ParseCdnServerUrl` (`0x6c9e4e`) parses `?fallback=1`,
`?strict=1`, `?maxhosts=10` query parameters from CDN server URLs
returned by version servers. These control fallback behavior,
strict mode, and host limits.

cascette-rs does not parse or honor these URL parameters.

### China Region CDN

Agent uses `.com.cn` domains for China:

```text
cn.patch.battlenet.com.cn
cn.patch.battlenet.com.cn:1119
https://cn.version.battlenet.com.cn
```

cascette-rs defaults to `.battle.net` domains. No `.com.cn` domain
handling or region-based domain switching exists.

### CDN URL Trailing Slash

Agent strips trailing slashes from `cdnPath` before constructing
URLs. cascette-rs does not normalize paths, so a CDN path ending
with `/` would produce double slashes in the URL.

## Root File Issues

No open root file issues from Agent.exe comparison.

## TVFS Issues

### Hardcoded 9-Byte EKey Size

`ContainerEntry` (`container_table.rs:19`) uses `[u8; 9]` for
EKey. The header has `ekey_size` and `pkey_size` fields that could
vary, but cascette-rs always reads 9 bytes. Matches Agent's
TACT 3.13.3 behavior but would break on future format changes.

### No Encoding Spec Table Parsing

The header supports optional EST (Encoding Spec Table) offset/size
fields (`header.rs:62-69`), but no code parses the EST data. The
`TVFS_FLAG_ENCODING_SPEC` flag is recognized but the table content
is ignored.

## Not Implemented

These features exist in Agent.exe but have no cascette-rs
equivalent. They are documented in the management repository
RE docs for future implementation.

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

### Local CASC Storage

Four container types manage local archive files:

| Type | Purpose |
|------|---------|
| Dynamic | Read/write CASC archives (.data files) |
| Static | Read-only archives (shared installations) |
| Residency | File state tracking (.residency tokens) |
| Hard Link | Filesystem hard links (trie directory) |

Key operations: open, read, write, remove, extract, compact.
Write path uses 0x1E-byte header offset. Read path uses LRU
caching with truncation tracking.

Reference: `agent-container-storage.md`

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
| CDN index footer (6 of 7) | `CdnIndexFooterValidator` | Version, hash size, block size, element size, reserved, footer hash |
| CDN index entry format | Variable-length keys | 4/5/6-byte offset support |
| Archive group format | 6-byte offsets | Archive index + offset parsing |
| BLTE magic and header | E-header parser | "BLTE" magic, single-chunk mode |
| BLTE chunk table | Flags 0x0F, 24-bit count | Chunk entries with MD5 checksums |
| BLTE block codecs | N/Z/4/E/F dispatch | All 5 compression modes |
| LZ4 size prefix | 8-byte LE decompressed size | Matches exactly |
| Encryption format | Salsa20 + ARC4 | Key name, IV, type byte, tau constant, IV extension |
| ESpec grammar | n/z/c/e/b/g letters | All 6 letters with correct parameters |
| ESpec block sizes | *=NNNNk/m notation | Numeric, K, M multipliers, * notation |
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
| TVFS container table | 9-byte EKey entries | EKey + file\_size + optional CKey |
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
| Archive group TOC hash | MD5 of first keys per chunk | `calculate_toc_hash()` on chunk first keys |

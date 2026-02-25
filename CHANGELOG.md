# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- cascette-formats: Download manifest CDN test fixture (Classic Era V1, truncated
  to 100 entries) with manifest.json metadata
- cascette-formats: Install manifest CDN test fixtures (Classic Era V1, Classic
  4.4.0 V1) with manifest.json metadata
- cascette-formats: Integration tests for download manifest parsing against real
  CDN data (7 tests covering V1 parsing, tags, tag types, round-trip, validation)
- cascette-formats: Integration tests for install manifest parsing against real
  CDN data (10 tests covering V1 parsing, tags, entries, round-trip, cross-product)
- cascette-formats: Root file CDN test fixtures (Classic Era V1, Retail V2
  extended header) downloaded via cascette-py, with manifest.json metadata
- cascette-formats: Integration tests for root file parsing against real CDN data
  (11 tests covering V1/V2 parsing, content/locale flags, records, lookups)
- cascette-formats: Suffix array-based bsdiff patch creation in `ZbsdiffBuilder`
  - `build()` / `build_optimized_patch()` using `divsufsort` for suffix array
    construction, implementing the Percival bsdiff algorithm
  - Produces near-optimal ZBSDIFF1 patches with three separate zlib-compressed
    blocks (control, diff, extra)
  - New `suffix` module with binary search on suffix array, forward/backward
    extension, and overlap resolution
  - CDN fixture round-trip tests: build patch from old->new, apply, verify
    output matches expected; parse CDN patch, rebuild bytes, verify byte-identical
    and application-correct
- cascette-formats: ZBSDIFF1 CDN test fixtures (5 triplets from WoW Classic)
  downloaded via cascette-py, with manifest.json metadata

### Fixed

- cascette-formats: Root content flags corrected to match CascLib/TACTSharp/wiki
  (10 of 12 values were wrong). Added X86_32 and X86_64. Removed NO_TOC_HASH.
- cascette-formats: Root locale flags corrected (PTBR, ITIT, RURU had wrong
  values). Added missing locales: enCN, enTW, esMX, ptPT.
- cascette-formats: Root default magic changed from MFST to TSFM to match real
  WoW output (CascLib `CASC_WOW_ROOT_SIGNATURE = 0x4D465354`)
- cascette-formats: Root extended header size handling fixed. Read/write now
  handle padding conditionally based on header_size field.
- cascette-formats: Root version detection accepts extended header version 1
  (matches CascLib and TACTSharp behavior). Maps to V2 block format.
- cascette-formats: Root name hash normalization corrected from forward slashes
  to backslashes, matching CascLib `NormalizeFileName_UpperBkSlash`
- cascette-formats: Root name hash word swap removed. Returns `(pc << 32) | pb`
  directly, matching CascLib `CalcNormNameHash`
- cascette-formats: Download `flag_size` validation added. Rejects flag_size > 4
  in both `DownloadHeader::validate()` and `DownloadManifestBuilder::with_flags()`,
  matching Agent.exe behavior
- cascette-formats: Install V2 extended header support. `InstallHeader` now reads
  and writes 6 additional bytes for V2 (content_key_size, entry_count_v2,
  v2_unknown). V1 computes content_key_size as `ckey_length + 4`
- cascette-formats: ZBSDIFF1 header endianness corrected from big-endian to
  little-endian, matching the original bsdiff format and verified against
  Agent.exe `tact::BsPatch::ParseHeader` at 0x6fbd1c
- cascette-formats: ZBSDIFF1 control block integer encoding corrected from
  two's complement (`i64::from_be_bytes`) to sign-magnitude (`offtin`/`offtout`),
  matching the bsdiff format where bit 63 is sign, bits 0-62 are magnitude in
  little-endian order

### Added

- cascette-client-storage: KMT update section for LSM-tree L0 writes
  - 24-byte `UpdateEntry` with hash guard, status byte, and delete markers
  - 512-byte `UpdatePage` (21 entries max) with 4KB sync every 8th page
  - `UpdateSection` with linear search (newest wins), merge-sort flush to
    sorted section via atomic file replacement
  - Lookup searches update section first, then sorted section
- cascette-client-storage: Residency container with KMT V8 file-backed storage
  - 40-byte `ResidencyEntry` with MurmurHash3 fast-path for `is_resident()`
  - Two-pass `scan_keys()` (count then populate) across 16 buckets
  - Batch delete with 10K threshold switching to batch path
  - `mark_span_non_resident(key, offset, length)` for truncation tracking
- cascette-client-storage: Hard link container with TrieDirectory and FD cache
  - `format_content_key_path(ekey)` -> `XX/YY/ZZZZ...ZZZZ` trie path layout
  - LRU FD cache with doubly-linked list eviction
  - Two-phase delete (collect entries with nlink <= 1, then remove)
  - `compact_directory()` validates trie at each depth, removes orphans
  - `query()` returns actual trie lookup result
- cascette-client-storage: Segment allocator for write path
  - Per-bucket `RwLock` array for concurrent KMT access
  - `allocate(size)` tries thawed segments first, creates new if full
  - Freeze/thaw state transitions with MAX_SEGMENTS (0x3FF) enforcement
  - `DynamicContainerBuilder` for configuring containers without breaking API
- cascette-client-storage: Two-phase compaction pipeline
  - Archive merge (between segments) and extract-compact (in-place) modes
  - `CompactionFileMover` with buffered I/O (`min(total >> 17, 16)` buffers)
  - `ExtractorCompactorBackup` crash recovery file (append-only segment list)
  - `validate_spans()` detects overlapping data ranges
  - `plan_archive_merge()` identifies low-utilization frozen segments
- cascette-client-storage: LRU cache shmem integration and eviction
  - `evict_to_target(target_bytes)` evicts from tail until target freed
  - `scan_directory()` removes stale `.lru` files from old generations
  - `for_each_entry(callback)` walks linked list for size accounting
  - `run_cycle()` and `shutdown()` matching Agent.exe `LRUManager::Run`
  - DynamicContainer touches LRU on `read()` and `write()`
- cascette-client-storage: Platform shared memory implementations
  - Unix: `shm_open`/`mmap` with owner-only permissions (0600), `flock`-based
    lock files with 100s timeout, `statfs` network drive detection
  - Windows: Type stubs with path normalization (lowercase, forward slashes,
    resolve `.`/`..`, max 248 bytes)
  - Control block `from_mapped()`/`to_mapped()` serialization for v4/v5
  - PID tracking serialization with slot management
- cascette-client-storage: Truncation tracking wired to residency and index
  - Truncated reads mark affected span non-resident via residency container
  - KMT entry status updated to DATA_NON_RESIDENT (7) via update section
  - Missing archive triggers key removal from container index
- docs: Agent container storage architecture documentation
- docs: Agent IDX/KMT file format documentation
- docs: Agent maintenance operations documentation

- cascette-client-storage crate: Local CASC storage for game installations
  - Bucket-based .idx index files with 18-byte entries, big-endian 9-byte
    truncated encoding keys, and archive location bit-packing
  - Memory-mapped .data archive files with BLTE compression (none, zlib, lz4)
    and compaction support
  - Content resolution pipeline: path/FileDataID -> ContentKey -> EncodingKey
    -> archive location, with DashMap-based caching
  - `Installation` async API for reading and writing files with 30-byte local
    archive header parsing
  - `Storage` multi-installation management with CASC directory structure
    validation (indices, data, config, shmem directories)
  - Shared memory IPC with binrw-serialized messages, platform-specific
    implementations (Windows `CreateFileMapping`, Unix `shm_open`), connection
    tracking, and CASC-compatible shmem file format
  - `BinaryFormatValidator` trait for round-trip testing with batch validation
    runner and property-based testing utilities
- Workspace dependencies: parking_lot 0.12, memmap2 0.9

- cascette-formats: Size manifest format parser and builder
  - Version 1 and 2 support with `SizeManifest`, `SizeManifestHeader`
    `SizeManifestEntry` types
  - Round-trip binary parsing and building via binrw
  - Version 2 adds 40-bit total size field per entry
  - Checksum verification using truncated MD5
  - docs: Added Size manifest documentation (`formats/size-manifest.md`)

- cascette-ribbit crate: Ribbit protocol server
  - HTTP server (axum) with `/{product}/versions`, `/{product}/cdns`
    `/{product}/bgdl` endpoints
  - TCP server with Ribbit v1 (MIME-wrapped with SHA-256 checksums) and v2
    (raw BPSV) protocol support
  - `v1/summary` endpoint listing all available products
  - JSON-based build database with MD5 hash validation
  - Multi-region BPSV response generation (us, eu, cn, kr, tw, sg, xx)
  - CDN configuration with per-region resolution
  - CLI binary (`cascette-ribbit`) with clap argument parsing and env var support
  - Optional TLS support behind `tls` feature flag
  - Contract tests verifying cascette-protocol client compatibility
  - Integration tests for HTTP, TCP v1, and TCP v2 protocols
  - Throughput benchmarks for BPSV generation
- docs: Added Ribbit Server documentation page (`protocols/ribbit-server.md`)
- Workspace dependencies: axum 0.8, tower-http 0.6, axum-server 0.7
  tokio-rustls 0.26, clap 4.5, anyhow 1.0, tracing-subscriber 0.3
- README: Added development section with mise tool version management
- docs: Added `wow_classic_titan` and `wow_anniversary` product codes with
  verified format details from live TACT data
- cascette-protocol crate: NGDP/CASC protocol implementation
  - Unified `RibbitTactClient` with automatic fallback (TACT HTTPS -> HTTP -> Ribbit TCP)
  - TACT client for HTTPS/HTTP queries to `us.version.battle.net`
  - Ribbit TCP client for direct protocol connections on port 1119
  - CDN client for content downloads with range requests and progress tracking
  - CDN streaming module behind `streaming` feature flag with BLTE
    decompression, concurrent chunk downloads, connection pooling with health
    checks, range request optimization, server failover with circuit breakers
    and Prometheus metrics
  - Protocol response caching with configurable TTLs
  - V1 MIME format support with PKCS#7 signature verification
  - Connection pooling and HTTP/2 support via reqwest with rustls
  - Retry policies with exponential backoff and jitter
  - Thread-local buffers and string interning for performance

- cascette-client-storage: Segment reconstruction header support
  - 480-byte header parsed as 16 x 30-byte `LocalHeader` entries (one per
    KMT bucket)
  - Segment header key generation with bucket hash targeting
  - `bucket_hash` function: XOR 9 bytes, `((xor>>4)^xor+seed) & 0xF`
  - Data file path generation (`data.NNN`) and filename parsing
  - `SegmentInfo` with frozen/thawed state tracking and space accounting
  - Segment size constant (1 GiB) and max segment count (1023)

- cascette-client-storage: `StaticContainer` read-only archive container
  - `IndexManager` + `ArchiveManager` for read-only key lookups and data reads
  - Batch `state_lookup` returning `KeyState { has_data, is_resident }` per key
  - Zero-key filtering

- cascette-client-storage: `ResidencyContainer` download tracking
  - In-memory key-level residency tracking (mark/unmark/query/scan)
  - `.residency` token file creation
  - Scanner API for iterating resident keys
  - Empty-product fallback

- cascette-client-storage: `HardLinkContainer` filesystem hard links
  - `test_support` filesystem probe using `casc_hard_link_test_file`
  - `create_link` with 3-retry delete
  - `.trie_directory` token file
  - Zero-key rejection
  - Silent success on file-not-found during remove

- cascette-client-storage: `DynamicContainer` read-write CASC archive container
  - Coordinates `IndexManager` (KMT) and `ArchiveManager` for read/write/remove/query
  - `open` async method loads indices and opens archive data files
  - Truncation detection converts archive bounds errors to `TruncatedRead`
  - `remove_span` with +0x1E offset adjustment
- cascette-client-storage: `.build.info` parser
  - BPSV file parser using `cascette-formats::bpsv` for installation metadata
  - Typed accessors for product, branch, build key, CDN key, version, CDN
    hosts/servers, install key, tags, and Armadillo key
  - Active entry detection via `Active` column

- cascette-client-storage: LRU cache with flat-file doubly-linked list
  - 28-byte header (version, MD5 hash, MRU head/LRU tail indices) and 20-byte
    entries (prev/next index, 9-byte ekey, flags)
  - O(1) touch/evict via hash map + doubly-linked list in flat array
  - Generation-based `.lru` file persistence with MD5 integrity check
  - File naming: 16 hex chars (big-endian 64-bit generation) + `.lru`
- cascette-client-storage: Shmem v4/v5 control block protocol
  - Version 4 (16-byte alignment, 0x150 header) and version 5 (page alignment
    0x154/0x258 header) layout constants
  - Free space table format identifier (0x2AB8) and data size validation
  - PID tracking with state machine (idle/modifying), slot management, recount
    recovery, and generation counter
  - `validate_for_bind` with protocol error strings
- cascette-formats: K-way merge for archive group building via `build_merged`
  - O(N log K) merge of pre-sorted archive indices using binary min-heap
  - Matches the CASC k-way merge algorithm
  - Deduplicates entries across archives, keeping first occurrence

### Fixed

- cascette-client-storage: Directory structure corrected to match CASC specification.
  CASC creates five subdirectories: `data/` (dynamic container with
  .idx, .data, shmem temp, LRU, KMT files), `indices/` (CDN index cache)
  `residency/` (residency tracking DB), `ecache/` (e-header cache), and
  `hardlink/` (hard link container trie). Removed incorrect `config/` and
  `shmem/` directories. Build/CDN configs are stored inside the dynamic
  container, not in a separate directory. Shared memory uses named kernel
  objects + a temp file in `data/`, not a `shmem/` directory.
- cascette-client-storage: `IndexEntry.size` field was serialized as
  little-endian but the format uses big-endian for all 18-byte
  entry fields (verified via CascLib `ConvertBytesToInteger_BE`)
- cascette-client-storage: `KmtEntry` was a fabricated 16-byte LE struct
  that did not match the format. The KMT file format is identical to IDX
  v7 (same 18-byte entries, same guarded blocks). `KmtEntry` is now a
  re-export of `IndexEntry`
- cascette-formats: Archive group builder wrote entry fields in wrong order
  (key, offset, size instead of key, size, offset), causing round-trip
  parse failures through `ArchiveGroup::parse`
- cascette-formats: Install tag bit mask used LSB-first bit ordering
  (`1 << offset`) instead of MSB-first (`0x80 >> offset`). This caused
  incorrect file-to-tag associations when parsing real install manifests.
  Verified against WoW Classic agent manifest data
- cascette-protocol: `ReqwestHttpClient::with_cdn_servers` missing
  `ensure_crypto_provider()` call, causing panic when creating reqwest
  client with rustls-no-provider configuration

### Changed

- cascette-formats: Renamed fields and constants based on Agent.exe verification
  - `EncodingHeader::unk_11` renamed to `flags` (verified at 0x6a23e6, must be 0)
  - `TVFS_FLAG_WRITE_SUPPORT` deprecated, use `TVFS_FLAG_ENCODING_SPEC` (0x02)
  - `TvfsHeader::has_write_support()` deprecated, use `has_encoding_spec()`
  - `constants::INDEX_VERSION` renamed to `LOCAL_IDX_VERSION` to distinguish
    local IDX format (v7) from CDN archive index footer version (0-1)
  - Added doc comments for BLTE `HeaderFlags::Extended` (Avowed-only),
    LZ4 compression format rationale, and `ZLibVariant` codec ID mapping
- **Breaking**: cascette-formats ESpec type model updated to match TACT behavior
  - Renamed `ZLibBits` to `ZLibVariant`, removed `Bits(u8)` variant
  - `ESpec::ZLib` fields changed from `bits: Option<ZLibBits>` to
    `variant: Option<ZLibVariant>` and `window_bits: Option<u8>`
  - `ESpec::BCPack::bcn` changed from `u8` to `Option<u8>` (bare `c` accepted)
  - `ESpec::GDeflate::level` changed from `u8` to `Option<u8>` (bare `g` accepted)
- axum-server uses `tls-rustls-no-provider` feature instead of `tls-rustls` to
  avoid pulling in aws-lc-rs/aws-lc-sys native C dependencies
- tokio-rustls uses `default-features = false` with `ring` feature to avoid
  aws-lc-rs
- deny.toml: Banned aws-lc-rs and aws-lc-sys crates to enforce pure-Rust
  crypto via ring
- deny.toml: Added advisory ignore for RUSTSEC-2025-0134 (rustls-pemfile
  deprecation, pulled in by axum-server 0.7 until upstream updates)
- Updated workspace dependencies:
  - reqwest 0.12 -> 0.13 (using `rustls-no-provider` with ring crypto provider)
  - tokio 1.47 -> 1.49
  - md5 0.7 -> 0.8
  - bytes 1.10 -> 1.11
  - asn1 0.22 -> 0.23
  - prometheus 0.13 -> 0.14
  - proptest 1.9 -> 1.10
  - lz4_flex 0.11 -> 0.12
- Removed unused workspace dependencies: `anyhow`, `mockall`
- Switched rustls crypto provider from aws-lc-rs to ring for WASM compatibility
  (removes aws-lc-sys C dependency from the production dependency tree)
- Renamed `mise.toml` to `.mise.toml` (dotfile convention)
- Updated markdownlint configuration with schema references, cli2 support, and
  relaxed rules for changelog format and table styles
- Updated wiremock dependency from 0.5 to 0.6 (removes unmaintained `instant` crate)
- cascette-cache: Added WASM support with browser storage backends
  - `LocalStorageCache` for small protocol data (~5-10MB browser limit)
  - `IndexedDbCache` for larger content (~50MB+ with user permission)
  - Cross-platform `CacheStats` using millisecond timestamps instead of `Instant`
  - Platform-specific conditional compilation for native-only features
  - Updated README with platform support documentation
- cascette-protocol: Full WASM support for browser-based applications
  - TCP Ribbit protocol conditionally compiled out on WASM (no raw sockets in browsers)
  - TACT HTTP/HTTPS client fully functional on WASM using browser Fetch API
  - CDN client with downloads and progress tracking (non-streaming on WASM)
  - Range request downloader with retry logic using gloo-timers for WASM sleep
  - Cache module uses localStorage on WASM for persistent protocol response caching
  - Platform-specific tokio and reqwest configurations
  - Transport module with WASM-compatible client builder (no pool/timeout settings)
  - Retry module with cross-platform sleep (tokio native, gloo-timers WASM)
  - Error handling adapted for reqwest WASM limitations (no is_connect check)
  - Certificate fetching native-only (requires TCP for Ribbit protocol)
  - Added `UnsupportedOnWasm` error variant for TCP-only endpoints
- Added `.cargo/config.toml` WASM target configuration for getrandom
- Added workspace dependencies: reqwest, url, sha2, digest, rsa, base64, mail-parser
  cms, der, asn1, x509-cert, wiremock
- Updated deny.toml to allow ISC, BSD-3-Clause, and CDLA-Permissive-2.0 licenses
  (used by ring, subtle, and webpki-roots respectively)

### Fixed

- cascette-formats: 6 format issues fixed based on CASC binary comparison
  - Encoding entry parsing now uses dynamic key sizes from header
    (`ckey_hash_size`, `ekey_hash_size`) instead of hardcoded 16 bytes
  - TOC hash algorithm corrected to `MD5(toc_keys || block_hashes)[:hash_bytes]`
    (was `MD5(toc_keys)[8:16]`). Per-block MD5 hashes now computed and written.
    Archive group builder fixed to use last key per block (was first)
  - Archive index builder uses configurable field sizes (`key_size`
    `offset_bytes`, `size_bytes`) instead of hardcoded 16/4/4. Binary search
    and TOC validation compute records-per-block from footer fields
  - TVFS container table reads `ekey_size` bytes from header instead of
    hardcoded 9. `ContainerEntry.ekey` changed from `[u8; 9]` to `Vec<u8>`
  - TVFS EST (Encoding Spec Table) now parsed when `TVFS_FLAG_ENCODING_SPEC`
    flag is set. Added `EstTable` type with null-terminated string parsing
  - Batch encoding lookups added: `batch_find_encodings`
    `batch_find_all_encodings`, `batch_find_especs` using sort-and-merge
    algorithm/`BatchLookupEKeys`
- cascette-formats: ESpec parser gaps fixed to match TACT behavior
  - GDeflate level range corrected from [1,9] to [1,12]
  - Window bits minimum lowered from 9 to 8
  - BCPack BCn range validation added ([1,7])
  - Added `InvalidBcn(u8)` error variant
  - ZLib 3-parameter syntax supported (`z:{level,variant,window_bits}`)
- docs: Removed binary verification sections from 13 documentation files
- cascette-formats: Corrected download manifest binary layout documentation
  - All versions use entries-then-tags order (not tags-then-entries for V2+)
  - Removed incorrect version-specific layout branching in parser/builder
  - Fixed header size calculation in tests (12 bytes for V2, not 11)
- docs: Fixed download manifest File Structure diagram to show correct order
- cascette-protocol: Fixed "No provider set" error in tests by adding rustls dev-dependency
  to provide crypto provider for reqwest with rustls feature
- Fixed reqwest feature from invalid rustls-tls to rustls in Cargo.toml
- cascette-protocol: Wired up `streaming` feature gate for CDN streaming module
  - Moved `cdn_streaming/` to `cdn/streaming/` for correct module path resolution
  - Added `streaming` feature with optional `binrw` dependency
  - Fixed `ArchiveIndex` API usage (`find_entry` instead of HashMap indexing
    `size` instead of `compressed_size`)
  - Fixed rand 0.10 API (`RngExt` trait instead of `Rng`)
  - Added crypto provider initialization in `ReqwestHttpClient::new`
  - Fixed CDN server count assertions in config tests
  - Removed ~208 redundant per-item `#[cfg(feature = "streaming")]` annotations
    (module-level gate is sufficient)
- cascette-formats: Fixed 4 format parser bugs found via CASC comparison
  - EKey page padding detection uses `espec_index == 0xFFFFFFFF` sentinel
    matching CASC, with zero-fill fallback for backward compatibility
  - Root V4 content flags widened to `u64` and parsed with
    `ContentFlags::read_v4`/`write_v4` for 5-byte (40-bit) flags instead
    of truncating to `u32`
  - Root version detection heuristic tightened to check version field against
    known values (2..=4) instead of `< 10`
  - EKey entry proptest size assertion corrected (36 -> 25 bytes), added
    missing `#[test]` annotations to 7 proptest macro functions
- cascette-formats: Added format validation matching CASC constraints
  - `EncodingHeader::validate` checks all 8 header fields (version, flags,
    ckey/ekey hash sizes, page counts, espec block size)
  - `ESpecTable::parse` rejects empty strings and unterminated data
  - Install manifest V2 support with per-entry `file_type` byte
  - `IndexFooter::validate_file_size` checks expected vs actual file size
  - `PatchArchiveHeader` flag bit interpretation with `is_plain_data` and
    `has_extended_header`; rejects unsupported extended header flag
  - `TvfsFile::parse` and `load_from_blte` call `header.validate`

### Infrastructure

- Added .config/nextest.toml with default, ci, and release profiles
- Added cargo aliases for nextest workflows in .cargo/config.toml
- Updated CI to use cargo nextest run --profile ci instead of cargo test
- Added .github/workflows/profiling.yml for flamegraph generation
- Added flamegraph and perf data paths to .gitignore
- Updated all documentation to use nextest instead of cargo test
- Added performance profiling section to testing documentation

## [0.2.0] - 2025-01-20

### Breaking Changes

- **MSRV increased from 1.86.0 to 1.92.0** - Required for new language features and
  clippy lints

### Changed

- Adopted Rust 1.92.0 language features:
  - Let-chains for cleaner conditional logic
  - `std::io::Error::other` for error construction
  - `.is_multiple_of` for divisibility checks
  - `usize::midpoint` for overflow-safe averaging
  - `#[default]` attribute on enum variants
- Updated code to satisfy new clippy lints in Rust 1.92.0

## [0.1.0] - 2025-01-15

### Added

- Rust 2024 workspace with MSRV 1.86.0
- cascette-crypto crate: MD5, Jenkins96, Salsa20, ARC4 implementations
- cascette-cache crate: Multi-layer caching for NGDP/CDN content
  - Memory cache with LRU eviction and size-based limits
  - Disk cache with fsync durability and atomic writes
  - Multi-layer cache combining L1 memory and L2 disk
  - NGDP-specific caches: resolution, content-addressed, BLTE block, archive range
  - Validation hooks for MD5, Jenkins96, and TACT key verification
  - Zero-copy data structures with reference counting
  - Streaming interfaces for large file handling
  - SIMD-optimized hash operations (SSE2, SSE4.1, AVX2, AVX-512)
  - Memory pooling optimized for NGDP file size classes
  - CDN integration with retry logic and range requests
- cascette-formats crate: Binary format parsers and builders for NGDP/CASC
  - BLTE: Block Table Encoded format with compression (ZLib, LZ4) and encryption
  - BPSV: Blizzard Pipe-Separated Values for version and config data
  - Archive: Archive index and data file operations for CDN content storage
  - Encoding: Content key to encoding key mappings
  - Root: Root file format mapping paths/FileDataIDs to content keys (V1-V4)
  - Install: Install manifest format for file tagging and selective installation
  - Download: Download manifest format for priority-based streaming (v1/v2/v3)
  - Config: Build and CDN configuration file formats
  - ESpec: Encoding specification format
  - TVFS: TACT Virtual File System manifest format
  - Patch Archive: Differential patch manifest format
  - ZBSDIFF1: Zlib-compressed binary differential patches
- TACT key management with TactKeyProvider trait for custom backends
- Workspace-level clippy lints for code quality
- Documentation framework using mdBook with Mermaid diagram support
- CI workflow with quality checks (fmt, clippy, test, doc, WASM)
- WASM compilation support for cascette-crypto (wasm32-unknown-unknown)
- Project introduction explaining wowemulation-dev goals and modern client focus
- Glossary of NGDP/CASC terminology with MPQ equivalents for newcomers
- Format documentation: encoding, root, install, download, archives, archive
  groups, TVFS, config formats, patches, BPSV, format transitions
- Compression documentation: BLTE container format, ESpec encoding specs
- Encryption documentation: Salsa20 stream cipher
- Protocol documentation: CDN architecture, Ribbit protocol
- Client documentation: Battle.net Agent, local CASC storage
- Operations documentation: CDN mirroring, reference implementations
- Community CDN mirrors list (Arctium, Wago, wow.tools)

### Changed

- Updated dependencies: tempfile 3.21→3.24, proptest 1.7→1.9, criterion 0.7→0.8
- Added workspace dependencies: bytes, dashmap, async-trait, futures, prometheus, tracing
- Removed keyring and file-store features from cascette-crypto for WASM compatibility
- Key loading functions now accept string content instead of file paths
- cascette-formats uses lz4_flex (pure Rust) instead of lz4 (C wrapper) for WASM compatibility

### Fixed

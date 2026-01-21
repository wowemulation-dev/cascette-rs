# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- cascette-protocol crate: NGDP/CASC protocol implementation
  - Unified `RibbitTactClient` with automatic fallback (TACT HTTPS -> HTTP -> Ribbit TCP)
  - TACT client for HTTPS/HTTP queries to `us.version.battle.net`
  - Ribbit TCP client for direct protocol connections on port 1119
  - CDN client for content downloads with range requests and progress tracking
  - CDN streaming with BLTE decompression and concurrent chunk downloads
  - Protocol response caching with configurable TTLs
  - V1 MIME format support with PKCS#7 signature verification
  - Connection pooling and HTTP/2 support via reqwest with rustls
  - Retry policies with exponential backoff and jitter
  - Thread-local buffers and string interning for performance

### Changed

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
- Added workspace dependencies: reqwest, url, sha2, digest, rsa, base64, mail-parser,
  cms, der, asn1, x509-cert, wiremock
- Updated deny.toml to allow ISC, BSD-3-Clause, and CDLA-Permissive-2.0 licenses
  (used by ring, subtle, and webpki-roots respectively)

### Fixed

- cascette-formats: Corrected download manifest binary layout documentation
  - All versions use entries-then-tags order (not tags-then-entries for V2+)
  - Removed incorrect version-specific layout branching in parser/builder
  - Fixed header size calculation in tests (12 bytes for V2, not 11)
- docs: Fixed download manifest File Structure diagram to show correct order

## [0.2.0] - 2025-01-20

### Breaking Changes

- **MSRV increased from 1.86.0 to 1.92.0** - Required for new language features and
  clippy lints

### Changed

- Adopted Rust 1.92.0 language features:
  - Let-chains for cleaner conditional logic
  - `std::io::Error::other()` for error construction
  - `.is_multiple_of()` for divisibility checks
  - `usize::midpoint()` for overflow-safe averaging
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

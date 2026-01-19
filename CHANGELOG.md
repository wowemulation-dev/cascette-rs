# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

<!-- Changes pending the 1.0.0 release go here -->

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

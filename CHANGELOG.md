# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Architecture Improvements Tasks**: Comprehensive plan for enhancing codebase quality
  - Dependency injection for better testability
  - Trait abstractions for modularity
  - Structured metrics and observability
  - Configuration centralization

- **CLI Pipe Handling Documentation**: Complete implementation guide for proper pipe handling
  - SIGPIPE signal handling for Unix systems
  - Broken pipe error suppression
  - TTY detection for appropriate output formatting

### Changed

- **CDN Client Architecture Refactoring**: Simplified from 3 variants to 2
  - Merged `CdnClient` and `CdnClientWithFallback` into single base client
  - Base client now includes fallback host support
  - `CachedCdnClient` wraps the unified client
  - All commands now use cached client for performance

### Fixed

- **CDN Path Usage**: Fixed to use server-announced paths instead of hardcoded values
  - Now properly uses CDN path from server response
  - Removed hardcoded "tpr/wow" assumptions
  
- **Test Infrastructure**: Fixed failing tests in ngdp-cache
  - Added HEAD request mocks for CDN client tests
  - Fixed race condition in `test_ribbit_cache_file_naming`
  
- **Code Formatting**: Applied consistent formatting across all test files

## [0.4.0] - 2025-08-09

### Added

#### Major Features

- **Complete NGDP Ecosystem Documentation**: Comprehensive architecture documentation from content creation to distribution
  - Added `docs/ngdp-ecosystem-complete.md` with full system architecture
  - Documented creative tools integration (Blender, Maya, level editors)
  - Detailed content management system requirements
  - Complete build and distribution pipeline documentation
  - Server implementation architecture (Ribbit as orchestrator, CDN distribution)

- **Project Roadmap and Organization**: Created comprehensive project structure documentation
  - Added `ROADMAP.md` with completed milestones and future plans
  - Created condensed `TODO-CONDENSED.md` focusing only on pending work
  - Organized completed features into roadmap for better visibility
  - Clear success metrics and version planning through v1.0.0

- **HTTP-first version discovery**: New `HybridVersionClient` that prioritizes modern HTTPS endpoints over legacy Ribbit protocol
  - Primary: `https://us.version.battle.net/wow/versions` and similar endpoints
  - Fallback: Legacy Ribbit TCP protocol (:1119) for backward compatibility
  - Transparent retry and error handling across both protocols

- **Client installation functionality**: Complete `install` command for downloading and setting up game clients
  - Support for minimal, full, custom, and metadata-only installation types
  - Automatic `.build.info` file generation using BPSV format for client restoration
  - Proper directory structure creation (`Data/`, `Data/config/`, etc.)
  - Install manifest filtering for region-specific and platform-specific builds
  - Resume capability for interrupted downloads
  - Repair command for verifying and fixing existing installations
  - Cross-command compatibility through shared .build.info format

- **Enhanced install manifest handling**: Proper filtering and validation of install manifest entries
  - Silent filtering of missing keys (normal for region-specific builds) following CascLib patterns
  - Size validation to detect and skip corrupted entries (e.g., >10GB files)
  - Comprehensive logging for debugging installation issues

- **Write Support Planning**: Identified and documented all components needing write support
  - TACT format writers (7 components: Encoding, Install, Download, Size, Config, TVFS, Root)
  - BPSV writer for Ribbit protocol compatibility
  - CASC index writers (.idx, .index files)
  - Key generation and management services
  - FileDataID assignment system
  - Content management system architecture
  - Complete NGDP build system specification

- **Code Quality Improvements**: Better adherence to Rust best practices
  - Added SAFETY documentation to all unsafe blocks
  - Refactored functions with too many arguments using config structs
  - Fixed clippy warnings across the codebase
  - Improved error handling with proper context
  - Added comprehensive README files for examples and tests
  - Fixed all deprecated warnings with proper annotations

### Fixed

- **Install manifest architecture**: Resolved key mismatch issues between install manifests and encoding files
  - Install manifests are comprehensive catalogs containing all possible files for all configurations
  - Missing keys from encoding files are expected behavior for filtered/regional builds
  - Only files present in encoding file are meant to be downloaded

- **Build configuration handling**: Verified proper uncompressed handling of BuildConfig and CDNConfig files
  - No BLTE decompression applied to configuration files (as intended)
  - Correct download order: encoding file downloaded before manifests that need key lookups

- **Test suite improvements**: Fixed all failing tests and warnings
  - Fixed BPSV API changes (entries() to rows())
  - Fixed mutable borrow errors in install command
  - Fixed deprecated function warnings with #[allow(deprecated)]
  - Fixed unnecessary type casts
  - All 436+ tests now passing

### Changed

- **Version discovery prioritization**: `ngdp-cache` now uses HTTP-first approach by default
  - HTTPS endpoints are primary method for version and CDN discovery
  - Ribbit protocol serves as fallback for backward compatibility
  - Better error messages indicating which discovery method was used

- **Documentation structure**: Reorganized documentation for better clarity
  - Main architecture documentation in `docs/ngdp-ecosystem-complete.md`
  - Completed work tracked in `ROADMAP.md`
  - Pending work in condensed `TODO-CONDENSED.md`
  - Updated all crate README files for accuracy

### Deprecated

- **ARC4 encryption support**: ARC4/RC4 cipher marked as deprecated (will be removed in v0.5.0)
  - `ngdp_crypto::encrypt_arc4` and `ngdp_crypto::decrypt_arc4` functions
  - `EncryptionMethod::ARC4` enum variant in BLTE compression
  - Modern implementations should use Salsa20 encryption instead

- **Recursive BLTE (Frame mode)**: Frame compression mode marked as deprecated (will be removed in v0.5.0)
  - `CompressionMode::Frame` enum variant
  - All Frame-related compression and decompression functions
  - Modern NGDP implementations use standard BLTE compression modes

## [0.3.1] - 2025-08-07

### Fixed

- **Clippy warnings**: Resolved all uninlined format arguments warnings across multiple files
  - Updated format strings to use inline variable syntax (e.g., `{var}` instead of `"{}", var`)
  - Affected files: blte/examples, tact-client/examples, tact-client/src, ngdp-client/src
  - Ensures code quality and consistency with modern Rust idioms

- **Release workflow**: Fixed missing crates in GitHub Actions release workflow
  - Added ngdp-crypto, tact-parser, and blte to version verification
  - Corrected publishing order to respect dependency requirements
  - Ensures all crates are properly published to crates.io

- **Documentation improvements**:
  - Corrected TACT acronym to "Trusted Application Content Transfer" across all documentation
  - Added missing crate descriptions for crates.io publishing
  - Updated all README files with proper installation instructions and version badges
  - Improved crate descriptions to be more informative and searchable

### Changed

- **Version bump**: Updated all crates from 0.3.0 to 0.3.1
- **Workflow stability**: Implemented long-term stability fixes for CI/CD pipelines

### Added

- **QA command documentation**: Created comprehensive rust-qa.md command file
  - Covers all GitHub Actions CI checks
  - Includes format, compilation, clippy, test, and documentation checks
  - Provides environment variables for CI-like behavior

## [0.3.0] - 2025-08-06

### Added

- **Ephemeral signing support**: Implemented ephemeral key signing following cargo-binstall approach
  - Per-release minisign key generation for enhanced security
  - Automatic signature verification in install scripts
  - Compatible with cargo-binstall's ephemeral signing model
  - Includes ephemeral-gen.sh script for key management

- **Installation script improvements**:
  - Added minisign signature verification
  - Support for both persistent and ephemeral signing keys
  - Automatic architecture detection
  - Platform-specific package format selection (tar.gz for Unix, zip for Windows)

### Fixed

- **Windows PowerShell compatibility**: Fixed install script execution on Windows
  - Removed unused sig_file variable that caused PowerShell errors
  - Improved cross-platform compatibility

### Changed

- **Version bump**: Updated all crates to version 0.3.0
- **Build workflow**: Added shell specification for build binary step

## [0.2.0] - 2025-08-07

This release introduces streaming capabilities, HTTP range request support, and completes all TACT file format parsers. The project now supports efficient processing of large game files with minimal memory usage.

### Added

#### Core Features

- **Streaming BLTE Decompression**: Memory-efficient streaming decompression for large files
  - `BLTEStream` struct implementing Read trait
  - Support for all compression modes (N, Z, 4, F, E)
  - Constant memory usage regardless of file size
  - Comprehensive examples and benchmarks

- **HTTP Range Requests**: Partial content downloads for bandwidth optimization
  - `download_file_range()` for single range requests
  - `download_file_multirange()` for multiple ranges
  - Automatic fallback when range not supported
  - Example demonstrating range request usage

- **Complete TACT Parser Suite**: All file formats now supported
  - Encoding file parser with 40-bit integer support
  - Install manifest parser with tag-based filtering
  - Download manifest parser with priority sorting
  - Size file parser with statistics
  - TVFS parser with correct real-data format
  - Variable-length integer utilities

#### CLI Enhancements

- **Download Command**: Full implementation with CDN integration
  - Download by build, content key, encoding key, or file path
  - Automatic BLTE decompression
  - Integration with cached clients
  - Pattern detection for different key types

- **Inspect Commands**: Visual tree display for all formats
  - `inspect build-config` with meaningful information display
  - `inspect encoding` for encoding file analysis
  - `inspect install` for installation manifest viewing
  - `inspect download-manifest` for download priorities
  - `inspect size` for size statistics
  - All commands support text, JSON, and BPSV output

- **Keys Management**: TACTKeys database integration
  - `keys update` downloads latest keys from GitHub
  - `keys status` shows local database information
  - Automatic loading from ~/.config/cascette/
  - Support for 19,419 WoW encryption keys

#### Infrastructure

- **ngdp-crypto Crate**: Complete encryption/decryption support
  - Salsa20 cipher with proper key extension
  - ARC4 cipher implementation
  - KeyService with automatic key loading
  - Support for multiple key file formats

- **blte Crate**: BLTE compression/decompression library
  - All compression modes (N, Z, 4, F, E)
  - Multi-chunk file support
  - Checksum verification
  - Integration with encryption

### Changed

- **Default Protocol Version**: HTTPS (v2) is now default for better security
- **Cache Management**: Added TTL support and improved eviction
- **Error Handling**: More descriptive error messages with context
- **Documentation**: Comprehensive API reference and streaming architecture docs
- **Version**: Updated all crates to version 0.2.0

### Fixed

- **TVFS Parser**: Corrected to handle real CDN data format
  - Uses proper flags (0x03) instead of synthetic data
  - Supports both TVFS and TFVS magic bytes
  - Correct big-endian byte order

- **Build Config Parser**: Now handles both format types
  - Single-hash format (older builds)
  - Hash-size pair format (newer builds)

- **Memory Leaks**: Fixed in BLTE decompression for large files
- **Panic Issues**: Removed unwrap() calls in production code

### Performance

- **Streaming Decompression**: 99% memory reduction for large files
- **Range Requests**: Up to 99% bandwidth savings for partial operations
- **Parallel Downloads**: Support for concurrent chunk retrieval
- **Cache Hit Ratio**: Improved with better key management

### Also Added in 0.2.0

#### `ngdp-crypto` crate (new)

- **Complete encryption/decryption support for NGDP/TACT**:
  - Salsa20 stream cipher for modern BLTE encryption
  - ARC4 (RC4) cipher for legacy content
  - KeyService for managing encryption keys
  - Automatic key loading from standard directories
  - Support for CSV, TXT, and TSV key file formats
  - Environment variable support (CASCETTE_KEYS_PATH)
  - Successfully loads 19,419+ WoW encryption keys

#### `blte` crate (new)

- **BLTE (Block Table Encoded) decompression library**:
  - Support for all compression modes (None, ZLib, LZ4, Frame, Encrypted)
  - Multi-chunk file handling with proper block indexing
  - BLTE header parsing for single and multi-chunk files
  - Chunk checksum verification (MD5)
  - Integration with ngdp-crypto for encrypted content
  - Memory-efficient chunk processing

#### `tact-parser` crate enhancements

- **Complete TACT manifest format support**:
  - Download manifest parser with priority sorting (versions 1-3)
  - Size file parser for installation size calculations
  - TVFS (TACT Virtual File System) parser with correct format
  - Variable-length integer (varint) support
  - Fixed encoding file MD5 checksum validation
  - Corrected TVFS format based on real data analysis

#### `ngdp-client` CLI enhancements

- **Encryption key management commands**:
  - `ngdp keys update` - Download latest keys from TACTKeys repository
  - `ngdp keys status` - Show local key database information
  - Automatic key file creation in ~/.config/cascette/

- **Enhanced inspect commands with BLTE support**:
  - `inspect encoding` - Inspect encoding files with validation
  - `inspect install` - Parse install manifests with tag filtering
  - `inspect download-manifest` - Analyze download priorities
  - `inspect size` - Calculate installation sizes
  - All commands now handle BLTE-encoded CDN files

### Fixed

- **TVFS parser corrections**:
  - Fixed magic bytes handling (TVFS/TFVS support)
  - Corrected byte ordering (big-endian)
  - Changed offsets from 40-bit to 32-bit integers
  - Fixed path table and EST table parsing

- **Encoding file improvements**:
  - Fixed MD5 checksum validation with proper implementation
  - Corrected page verification logic

### Previous Release Content

#### `tact-parser` crate

- **New crate for parsing TACT file formats**:
  - Support for parsing WoW root files to find file IDs and MD5s
  - Jenkins3 hash implementation for TACT data processing
  - Support for both modern (8.2+) and legacy pre-8.2 root formats
  - Efficient buffered I/O operations for improved performance
  - Comprehensive test suite with unit and integration tests
  - Performance benchmarks for Jenkins3 hashing
  - Example demonstrating WoW root file parsing

- **Build configuration parsing support**:
  - Complete parser for TACT build configuration files (text-based key=value format)
  - Support for hash-size pairs parsing from configuration entries
  - Handles empty values and various configuration formats
  - Helper methods for accessing common build properties (root, encoding, install, download hashes)
  - Build name extraction and VFS entry counting
  - Comprehensive test coverage with real-world configuration data

- **Encoding file parsing with 40-bit integer support**:
  - Full parser for TACT encoding files (binary format with CKey ↔ EKey mapping)
  - Native support for 40-bit integers used for file sizes (supports up to 1TB files)
  - Bidirectional lookup: CKey → EKey entries and EKey → CKey reverse mapping
  - Helper methods for common operations (get file size, get first encoding key)
  - Big-endian header parsing for encoding file metadata
  - Support for multiple encoding keys per content key (different compression methods)

- **Install manifest parsing**:
  - Parser for TACT install manifest files
  - Support for file installation metadata and directory structures
  - Integration with encoding file lookups for complete file resolution

- **Utility functions**:
  - 40-bit integer reading/writing utilities (little-endian format)
  - Configuration file text parsing with proper empty value handling
  - Hash parsing and validation utilities

#### `ngdp-cdn` crate

- **Automatic CDN fallback support**:
  - Added `CdnClientWithFallback` for automatic failover between multiple CDN hosts
  - Built-in support for community backup CDNs: `cdn.arctium.tools` and `tact.mirror.reliquaryhq.com`
  - Prioritizes all Blizzard CDN servers first before trying community mirrors
  - Configurable backup CDN behavior with `use_default_backups` option
  - Full API compatibility with base `CdnClient` for easy migration
  - Support for custom CDN fallbacks via `add_custom_cdn()` and `set_custom_cdns()` methods
  - Custom CDNs are tried after primary and community CDNs in fallback order

#### `ngdp-client` crate

- **Historical builds command**:
  - Added `ngdp products builds` command to retrieve all historical builds for a product
  - Integrates with Wago Tools API (<https://wago.tools/api/builds>) for comprehensive build history
  - Support for filtering by version pattern with `--filter`
  - Time-based filtering with `--days` option
  - Result limiting with `--limit` option
  - Background download builds filtering with `--bgdl-only`
  - Displays build version, creation date, build config, and type (Full/BGDL)
  - Support for JSON, BPSV, and formatted text output
  - Caching support with 30-minute TTL to reduce API load
  - Respects global cache settings (`--no-cache` and `--clear-cache` flags)

- **TACT parser integration for build configuration analysis**:
  - Added `inspect build-config` command for detailed build configuration analysis
  - Downloads and parses real build configurations from CDN using tact-parser
  - Visual tree representation of game build structure with emoji and Unicode box-drawing
  - Shows core game files (root, encoding, install, download, size) with file sizes
  - Displays build information (version, UID, product, installer)
  - Patch status indication with hash display
  - VFS (Virtual File System) entries listing with file counts
  - Support for all output formats: text (visual tree), JSON, and raw BPSV
  - Example: `ngdp inspect build-config wow_classic_era 61582 --region us`

- **Enhanced products versions command with build configuration parsing**:
  - Added `--parse-config` flag to `products versions` command
  - Downloads and parses build configurations to show meaningful information
  - Displays build names instead of just cryptic hashes (e.g., "WOW-62417patch11.2.0_Retail")
  - Shows patch availability and file size information
  - Counts VFS entries to indicate build complexity
  - Maintains full backward compatibility when flag is not used
  - Works across all WoW products (wow, wow_classic_era, wowt, etc.)
  - Example: `ngdp products versions wow --parse-config`

### Changed

#### `ribbit-client` crate

- **Refactored BPSV response handling**:
  - Split BPSV functionality from `TypedResponse` to new `TypedBpsvResponse` trait
  - Allows non-BPSV responses to be parsed through the typed response system
  - Re-exported `TypedBpsvResponse` for backward compatibility
  - Improved separation of concerns between response types

### Fixed

- Fixed path tests on non-Linux platforms in multiple crates:
  - Updated cache path tests to work correctly across different operating systems
  - Fixed hardcoded Unix path separators that caused test failures on Windows

#### Code Quality and Safety Improvements

- **Removed panicking Default implementations**:
  - Removed `Default` trait implementation from `CdnClient` that would panic on failure
  - Removed `Default` trait implementation from `CdnClientWithFallback` that would panic on failure
  - These implementations were not used anywhere in the codebase

- **Fixed unwrap() calls in library code**:
  - Replaced all `unwrap()` calls in `tact-client` response parsing with proper error handling
  - Added meaningful error messages for missing fields using the `MissingField` error variant
  - All library code now properly propagates errors instead of panicking

- **Documented safety of unwrap() usage**:
  - Added SAFETY comments to `tact-parser/src/jenkins3.rs` explaining why unwrap() calls are safe
  - These unwraps operate on fixed-size array slices where bounds are guaranteed

- **Code optimization and cleanup**:
  - Removed unnecessary `to_string()` calls and string clones in `ngdp-cdn`
  - Replaced `vec!` with arrays for small fixed-size collections
  - Optimized string building in `ngdp-client` using iterator chains instead of manual loops
  - Fixed clippy warnings about format string inlining

## [0.1.0] - 2025-06-28

### Added

#### Performance Optimizations

- **Zero-copy parsing in `ngdp-bpsv`**:
  - Implemented borrowed string slices (`&'a str`) for memory-efficient parsing
  - Added `BpsvRow<'a>` and `BpsvDocument<'a>` with lifetime parameters
  - Added `OwnedBpsvDocument` for serialization scenarios
  - Performance improvements: 20-42% faster parsing across document sizes
  - Memory reduction: Eliminates string allocations during parsing

- **Parallel download support in `ngdp-cdn`**:
  - Added `download_parallel()` for bulk downloads with configurable concurrency
  - Added `download_parallel_with_progress()` for real-time progress tracking
  - Added specialized parallel methods: `download_data_parallel()`, `download_config_parallel()`, `download_patch_parallel()`
  - Added `download_streaming()` and `download_chunked()` for large files
  - Performance improvements: 3-5x speedup for bulk operations
  - Setup overhead: Only 1.6-15µs for 10-100 files

- **Streaming I/O operations in `ngdp-cache`**:
  - Added `read_streaming()` and `write_streaming()` for memory-efficient file operations
  - Added `read_chunked()` and `write_chunked()` for data processing without memory load
  - Added `copy()` for efficient cache entry duplication
  - Added `read_streaming_buffered()` with customizable buffer sizes
  - Added `size()` method for getting file sizes without reading
  - Memory efficiency: Constant 8KB buffer usage regardless of file size (640x reduction for large files)

#### `ngdp-cache` crate

- **New CachedCdnClient for transparent CDN content caching**:
  - Implements caching wrapper around CdnClient for CDN content downloads
  - Uses cache schema: `~/.cache/ngdp/cdn/{type}/{hash[0:2]}/{hash[2:4]}/{hash}`
  - Supports all CDN content types: config, data, patch, indices
  - Automatic hash-based directory structure following Blizzard CDN conventions
  - TTL-based cache expiration with configurable policies
  - Full async support with proper error handling

- **Enhanced example collection**:
  - Renamed `cache_basic_usage.rs` to `01_basic_cache_types.rs` for better organization
  - Added `cached_cdn_client.rs` - Demonstrates CachedCdnClient usage patterns
  - Added `cdn_cache_structure.rs` - Shows CDN cache directory organization
  - Added `cdn_helper_methods.rs` - Utility methods for CDN operations
  - Added `full_ngdp_pipeline.rs` - Complete NGDP workflow demonstration
  - Added `ribbit_cdn_download.rs` - Integration between Ribbit and CDN caching
  - Removed outdated examples: `cached_request_example.rs`, `test_cache_validity.rs`, `verify_cache_structure.rs`

- **Comprehensive test suite expansion**:
  - Added `cache_validity_test.rs` - Cache validation and expiration tests
  - Added `cached_cdn_client_integration.rs` - Full CDN client integration tests
  - Added `cached_cdn_helper_tests.rs` - Helper method functionality tests
  - Added `ribbit_cache_structure_test.rs` - Ribbit cache organization validation
  - Added `ribbit_cdn_integration.rs` - Cross-protocol integration tests

#### `ngdp-cdn` crate

- **Added unit test coverage**:
  - New `client_test.rs` with comprehensive unit tests for CdnClient
  - Tests for URL construction, error handling, and configuration validation
  - Builder pattern testing with various configurations
  - Proper test isolation and mocking support

#### `ngdp-client` crate

- **Reorganized example collection**:
  - Replaced specific examples with grouped operations:
    - `certificate_operations.rs` - All certificate-related functionality (replaces `cached_certificate_fetch.rs`, `download_certificate.rs`)
    - `products_operations.rs` - All product query operations (replaces `products_info_demo.rs`, `query_products.rs`, `cdns_region_demo.rs`)
  - Improved example organization for better discoverability

#### Removed/Cleaned Examples

- **Removed obsolete debugging examples**:
  - `ribbit-client`: Removed `raw_debug.rs`, `test_signature_verification.rs`
  - `tact-client`: Removed `test_certs_endpoint.rs`, `test_different_products.rs`, `test_v1_summary.rs`
  - These were development/debugging tools not useful for end users

#### Project Infrastructure

- **Comprehensive GitHub Actions CI/CD pipeline**:
  - CI workflow with platform matrix testing, documentation checks, and code coverage
  - Cross-platform build verification for multiple architectures (x86_64, aarch64, armv7)
  - Automated dependency updates via Dependabot
  - Release automation with release-plz for version management
  - Binary release workflow for ngdp-client CLI with minisign signatures
  - Install script with automatic platform detection and signature verification
  - Cache management and cleanup workflows
  - Support for both library releases to crates.io and CLI binary releases

### Changed

#### Performance Optimizations

- **Enhanced `tact-client` with field index caching**:
  - Pre-computed field indices for faster BPSV parsing in response handlers
  - Performance improvements: 15-24% faster in `parse_versions()` and `parse_cdns()`
  - Reduced HashMap lookups during schema-based field access

- **Improved `ribbit-client` with DNS caching**:
  - Added DNS resolution caching with configurable TTL (default: 5 minutes)
  - Supports connection failover to multiple resolved IP addresses
  - Reduces DNS lookup overhead for repeated requests to the same hosts

- **Optimized `ngdp-cache` with async I/O and batch operations**:
  - Fixed blocking I/O operations by replacing synchronous checks with async
  - Added parallel batch operations: `write_batch()`, `read_batch()`, `delete_batch()`, `exists_batch()`
  - Improved throughput for bulk cache operations

#### `ngdp-cache` crate

- **Updated Cargo.toml metadata**:
  - Enhanced package description and documentation
  - Added comprehensive keywords and categories for better discoverability
  - Updated README with current functionality and examples

- **Improved error handling and module organization**:
  - Enhanced error types in `error.rs` for better debugging
  - Updated `lib.rs` exports for cleaner public API
  - Improved `cdn.rs` with better CDN content handling

#### `ngdp-cdn` crate

- **Enhanced client functionality**:
  - Improved error handling in `client.rs`
  - Better integration test coverage in `integration_test.rs`
  - Enhanced public API exports in `lib.rs`

#### `ngdp-client` crate

- **Enhanced library exports**:
  - Improved public API organization in `lib.rs`
  - Better integration with caching functionality
  - Enhanced certificate test coverage in `certs_test.rs`

### Fixed

- Fixed benchmark HEX field validation in `ngdp-bpsv`:
  - Corrected hex string lengths in benchmarks to match field byte requirements
  - HEX:32 now correctly uses 64 hex characters (32 bytes)
  - HEX:16 now correctly uses 32 hex characters (16 bytes)

### Changed

#### Breaking Changes

- **Changed default protocol version to V2 for both Ribbit and TACT clients**:
  - `RibbitClient::new()` now defaults to `ProtocolVersion::V2` (was V1)
  - `HttpClient::default()` now uses `ProtocolVersion::V2` (was V1)
  - V2 provides better performance and is the modern protocol
  - To use V1, explicitly set it: `.with_protocol_version(ProtocolVersion::V1)`
  - Updated FallbackClient to use V2 for TACT fallback
  - Certificate and OCSP endpoints automatically use V1 (V2 not supported for these)
  - Added `Clone` trait to `RibbitClient` for protocol version switching

### Fixed

#### `ngdp-bpsv` crate

- **Fixed HEX field length interpretation to match Blizzard's semantics**:
  - HEX:N now correctly represents N bytes in binary format (N*2 hex characters)
  - Previously interpreted HEX:16 as 16 characters, now correctly expects 32 characters
  - This aligns with Blizzard's actual data format where HEX:16 = 16-byte MD5 hash
  - Updated all tests to use correct hex string lengths
  - Updated documentation to clarify field length semantics

#### `ribbit-client` crate

- **Removed HEX field length adjustment workaround**:
  - Removed `adjust_hex_field_lengths` function that was compensating for BPSV parser issue
  - Now parses BPSV responses directly without field manipulation
  - Simplifies code and improves reliability

### Added

#### `ngdp-client` crate

- **Added automatic Ribbit to TACT fallback**:
  - New `FallbackClient` that tries Ribbit first (primary protocol)
  - Automatically falls back to TACT HTTP if Ribbit fails
  - Both protocols return identical BPSV data
  - Transparent caching for both protocols
  - SG region automatically maps to US for TACT (not supported)
  - All product commands now benefit from improved reliability

- **Improved `products info` command behavior**:
  - Now respects the `--region` parameter properly
  - When `--region` is specified, shows only that region's information
  - When no region is specified, shows all regions in a table
  - CDN hosts are filtered to match the specified region
  - More intuitive and consistent with user expectations

- **Fixed `products cdns` command to filter by region**:
  - Now shows CDN configuration only for the specified region
  - Header indicates which region is being displayed
  - Shows both CDN hosts and servers for the region
  - Displays warning if no CDN configuration exists for the region
  - Consistent behavior across all output formats

- **Fixed `products info` command to display CDN servers**:
  - Now displays both CDN hosts and CDN servers sections
  - Collects unique servers from all filtered CDN entries
  - Shows server count in section headers
  - JSON output includes servers in CDN data
  - Consistent display format with CDN hosts

### Changed

#### `tact-client` crate

- **Added custom user agent support**:
  - Added `with_user_agent()` method to set custom User-Agent headers
  - User agent is applied to all HTTP requests including retries
  - If not set, uses reqwest's default user agent
  - Example usage: `client.with_user_agent("MyGameLauncher/1.0")`

#### `ngdp-cdn` crate

- **Added custom user agent support**:
  - Added `with_user_agent()` method to `CdnClient`
  - Added `user_agent()` method to `CdnClientBuilder`
  - User agent is applied to all CDN download requests
  - If not set, uses reqwest's default user agent
  - Example usage via builder: `CdnClient::builder().user_agent("MyClient/1.0").build()`
  - Example usage via method: `CdnClient::new()?.with_user_agent("MyClient/1.0")`

### Fixed

#### `ngdp-cache` crate

- **Fixed CachedRibbitClient cache directory structure**:
  - Removed incorrect "cached" subdirectory from cache path
  - Cache now correctly uses `~/.cache/ngdp/ribbit/{region}/` instead of `~/.cache/ngdp/ribbit/cached/{region}/`
  - This aligns with the RibbitCache implementation for consistency
  - Updated example to reflect correct cache path

- Fixed example filename collisions across crates:
  - Renamed `basic_usage.rs` examples to unique names per crate
  - `ribbit-client`: `basic_usage.rs` → `ribbit_basic_usage.rs`
  - `tact-client`: `basic_usage.rs` → `tact_basic_usage.rs` and `retry_handling.rs` → `tact_retry_handling.rs`
  - `ngdp-cache`: `basic_usage.rs` → `cache_basic_usage.rs`
  - `ngdp-cdn`: `basic_usage.rs` → `basic_usage.rs` (kept as is, new crate)
- Fixed missing export of parse functions in `tact-client`:
  - Exported `parse_cdns` and `parse_versions` from the crate root
  - Updated example to use the correct import path
- Fixed code quality issues:
  - Resolved collapsible if statement in tact-client retry logic
  - Fixed float comparison in ribbit-client tests using epsilon comparison
  - Removed redundant imports in examples

### Added

#### `ngdp-cache` crate

- **Added CachedTactClient for TACT protocol caching**:
  - Implements transparent caching for TACT metadata endpoints (versions, CDN configs, BGDL)
  - Uses cache schema: `~/.cache/ngdp/tact/{region}/{protocol}/{product}/{endpoint}-{sequence}.bpsv`
  - Automatic sequence number extraction and tracking from responses
  - TTL strategies: 5 minutes for versions, 30 minutes for CDN configs and BGDL
  - Full async support with proper error handling
  - Important: This caches TACT metadata only, NOT actual CDN content files

### Documentation

#### `ngdp-cache` crate

- **Clarified TACT vs CDN caching distinction**:
  - Added comprehensive documentation explaining that TACT `/cdns` endpoint returns CDN configuration, not content
  - Documented that actual CDN content caching should use `~/.cache/ngdp/cdn/`
  - Added integration tests demonstrating proper cache isolation and structure

- **Merged TactCache into CdnCache**:
  - Removed separate TactCache module as it was duplicating CDN functionality
  - CdnCache now handles all CDN content types: config/, data/, patch/, and indices
  - Updated all tests and examples to use the unified CdnCache API
  - Cache structure follows standard CDN paths: `{type}/{hash[0:2]}/{hash[2:4]}/{hash}`
  - Fixed bool comparison style in ngdp-cache examples
- Fixed clippy warnings across all crates:
  - Fixed raw string literal hashes in ngdp-bpsv tests
  - Fixed string interpolation in format strings
  - Used `Self` instead of repeating type names
  - Fixed integer cast warnings using proper `from()` conversions
  - Added missing `#[must_use]` attributes
  - Added missing `# Errors` documentation sections
  - Fixed needless pass-by-value in `add_raw_row` method
  - Fixed redundant closures in benchmarks and examples
  - Added missing package metadata (keywords and categories) to all crates

### Added

#### `ngdp-cache` crate

- **New crate for generic NGDP caching functionality**
- **Cache Types**:
  - `GenericCache`: Key-value storage for arbitrary data
  - `TactCache`: TACT protocol data (configs, indices, data files)
  - `CdnCache`: CDN content (archives, loose files) with product-specific support
  - `RibbitCache`: Ribbit protocol responses with TTL-based expiration
- **Features**:
  - Platform-specific cache directory using `dirs::cache_dir()`
  - CDN-compatible directory structure (hash-based path segmentation)
  - Async I/O operations using Tokio
  - Automatic directory creation
  - TTL support for time-based cache expiration
  - Streaming file operations for large archives
  - **CachedRibbitClient**: Complete drop-in replacement for RibbitClient with transparent caching
    - Uses Blizzard MIME filename convention: command-argument(s)-sequencenumber.bmime
    - Certificate requests cached for 30 days vs 5 minutes for regular responses
    - Automatic cache invalidation and cleanup
    - Implements full RibbitClient API:
      - `request()` - Returns cached Response objects with raw data
      - `request_raw()` - Returns cached raw bytes
      - `request_typed<T>()` - Returns typed responses with caching

#### `ngdp-cdn` crate

- **New crate for CDN content delivery operations**
- **Features**:
  - Async HTTP client with connection pooling (using `reqwest`)
  - Automatic retry with exponential backoff and jitter
  - Support for gzip/deflate compression
  - Configurable timeouts and retry policies
  - Rate limiting detection and handling
  - Content verification error types
  - CDN URL building following Blizzard's path structure
- **Builder pattern configuration**:
  - Connection timeout
  - Request timeout
  - Pool size per host
  - Retry parameters (max retries, backoff, jitter)
- **Error handling**:
  - Specific error types for CDN operations
  - Content not found (404) with hash extraction
  - Rate limiting with retry-after support
  - Size mismatch detection
  - Network timeout errors
    - All convenience methods: `get_summary()`, `get_product_versions()`, etc.
    - Supports both V1 (MIME) and V2 (raw) protocol versions
    - Perfect for CLI integration to reduce API calls
    - **Proper sequence number extraction from responses**:
      - Cache files now use actual sequence numbers from responses (e.g., `summary-#-3021124.bmime`)
      - Automatically finds and uses the most recent cached version
      - Falls back to 0 for endpoints without sequence numbers (e.g., certificates)
- **Testing**:
  - Unit tests for all cache types including CachedRibbitClient
  - 21 comprehensive integration tests covering:
    - Cross-cache isolation
    - TACT workflow simulation
    - Product-specific CDN caching
    - Concurrent access patterns
    - Large file handling (10MB+)
    - Cache expiration and TTL validation
    - Corruption detection
    - Key validation with various formats
    - CachedRibbitClient functionality (9 integration tests)
      - Client creation and configuration
      - Cache enable/disable controls
      - TTL differentiation by endpoint type
      - Cache clearing and expiration cleanup
      - Concurrent access handling
      - Multi-region cache isolation
      - Directory structure validation
- **Benchmarks**: Performance benchmarks for:
  - Generic cache read/write operations (small/medium/large data)
  - TACT cache operations and path construction
  - CDN archive operations and size queries
  - Ribbit cache write and validation
  - Concurrent write operations
  - Hash-based path segmentation
  - CachedRibbitClient operations:
    - Filename generation performance
    - Cache validity checking
    - Cache write operations
    - Expired entry cleanup
- **Examples**:
  - Basic usage example demonstrating all cache types
  - `cached_ribbit_client.rs` - Demonstrates CachedRibbitClient usage with performance comparison
  - `cached_request_example.rs` - Shows caching of full Response objects
  - `drop_in_replacement.rs` - Demonstrates complete API compatibility with RibbitClient

### Changed

#### `ribbit-client` crate

- Replaced custom base64 decoder with `base64` crate (0.22) for better reliability
  and performance
- Improved code quality following Rust best practices:
  - Added clippy pedantic lints for stricter code standards
  - Fixed all format string warnings to use inline variables
  - Added `#[must_use]` attributes to all applicable methods
  - Added comprehensive error documentation with `# Errors` sections
  - Fixed unnecessary `Result` wrapping in internal functions
  - Improved code organization and reduced redundancy
- Added test-case to examine a certificate checksum bug on Blizzards side
- **CDN servers field parsing consistency**:
  - Changed `servers` field from `Option<String>` to `Vec<String>`
  - Now parses servers as space-separated list, same as hosts field
  - Ensures consistency with TACT HTTP client implementation
  - Added comprehensive tests for servers field parsing
- **Added automatic retry support with exponential backoff**:
  - Configurable retry behavior with builder pattern methods
  - Default of 0 retries maintains backward compatibility
  - Exponential backoff with configurable parameters:
    - `with_max_retries()` - Set maximum retry attempts (default: 0)
    - `with_initial_backoff_ms()` - Initial backoff duration (default: 100ms)
    - `with_max_backoff_ms()` - Maximum backoff cap (default: 10 seconds)
    - `with_backoff_multiplier()` - Backoff growth factor (default: 2.0)
    - `with_jitter_factor()` - Randomness to prevent thundering herd (default: 0.1)
  - Only retries transient network errors (connection failures, timeouts, send/receive errors)
  - Parse errors and other non-retryable errors fail immediately
  - Added example `retry_handling.rs` demonstrating retry strategies
  - Fully compatible with CachedRibbitClient wrapper

#### `ngdp-client` crate

- **New `certs` subcommand for certificate operations**:
  - `certs download` - Download certificates by SKI/hash
  - Support for both PEM and DER output formats
  - Certificate details extraction (subject, issuer, validity dates)
  - JSON output format for programmatic access
  - Cached certificate downloads using CachedRibbitClient
  - Example: `ngdp certs download 5168ff90af0207753cccd9656462a212b859723b --details`
- Redesigned `products versions --all-regions` output to use a cleaner multi-row format:
  - Single "Configuration Hash" column with labeled hash values
  - Each region displays Build Config, CDN Config, Product Config, and Key Ring (if present)
  - Improved readability with consistent alignment and styling
  - Full hash values displayed for easy copy-paste
- Improved `products cdns` output with table-based display:
  - Separate table per region showing Path, Config Path, CDN Hosts, and Servers
  - CDN Hosts displayed before Servers with one host per line
  - Servers displayed with one URL per line for better readability
  - Consistent formatting between CDN Hosts and Servers fields
  - Better organization and visual clarity of CDN configuration

#### Workspace

- Moved `tokio` dependency to workspace level (1.45) for consistency across crates

### Fixed

#### `ribbit-client` crate

- Fixed compilation error in typed response tests caused by API method name change
- Corrected BPSV document method calls from `headers()` to `schema().field_names()`
- Fixed missing error documentation for all Result-returning public methods
- Added proper `#[must_use]` attributes to methods that should have return values used
- Fixed documentation markdown formatting issues (missing backticks)
- Improved numeric literal readability with separators (123_456 instead of 123456)
- Removed unused imports that were causing linter warnings
- Fixed all remaining clippy warnings for better code quality
- **Fixed infinite hang when connecting to CN region from outside China**:
  - Added 10-second connection timeout to prevent indefinite hanging
  - Added proper timeout error handling with user-friendly messages
  - Added guidance for users about CN region accessibility restrictions
- Fixed clippy warnings in test code (merged match arms, inline format args)

#### `ngdp-client` crate

- Fixed missing Product Config hash in `products versions --all-regions` table
- Fixed missing Key Ring hash in `products versions --all-regions` table
- Improved error handling with user-friendly messages for connection issues

### Added

#### `ngdp-bpsv` crate

- **Core Features**
  - Complete BPSV (Blizzard Pipe-Separated Values) parser and writer
  - Support for all NGDP data formats across TACT and Ribbit endpoints
  - Type-safe field definitions (STRING, HEX, DEC)
  - Case-insensitive field type parsing
  - Sequence number support for version tracking
  - Empty value support for all field types

- **Parser Features**
  - Fast, zero-copy parsing where possible
  - Comprehensive error reporting with line numbers
  - Schema validation for all data rows
  - Support for variable-length documents
  - Handles real-world NGDP data edge cases

- **Builder Features**
  - Fluent API for document construction
  - Type-safe value addition with validation
  - Automatic schema enforcement
  - Round-trip compatibility (parse → build → parse)
  - Support for creating documents from existing BPSV data

- **Data Types**
  - `STRING:length` - String fields with optional length limits
  - `HEX:length` - Hexadecimal fields for hashes and binary data
  - `DEC:length` - Decimal integer fields
  - All types support empty values

- **Examples**
  - `parse_basic.rs` - Parse real Ribbit version data
  - `build_bpsv.rs` - Build BPSV documents programmatically
  - `typed_access.rs` - Type-safe value access patterns

- **Testing**
  - 44 unit tests covering all functionality
  - 12 comprehensive integration tests
  - Real-world data parsing tests
  - Edge case and error handling tests
  - Performance benchmarks for parsing and building

- **Documentation**
  - Complete API documentation with examples
  - BPSV format specification in docs/
  - Usage examples for common scenarios
  - Performance characteristics

#### `ribbit-client` crate

- **Core Features**
  - Complete Ribbit protocol client implementation for Blizzard's version server
  - Support for both V1 (MIME) and V2 (raw PSV) protocol versions
  - Async TCP client using Tokio for non-blocking I/O operations
  - Connection pooling and automatic reconnection handling
  - Builder pattern for flexible client configuration

- **Region Support**
  - All major Blizzard regions: US, EU, CN, KR, TW, SG
  - Automatic region-specific server endpoint resolution
  - String parsing support for region identifiers

- **Endpoint Coverage**
  - `Summary` - List all available products
  - `ProductVersions` - Get version information for specific products
  - `ProductCdns` - Retrieve CDN server information
  - `ProductBgdl` - Background download configuration
  - `Cert` - Certificate retrieval by SHA-1 hash
  - `Ocsp` - OCSP response retrieval by hash
  - `Custom` - Support for arbitrary endpoint paths

- **MIME Parsing (V1 Protocol)**
  - Full MIME message parsing using `mail-parser` crate
  - Multipart MIME support for messages with attachments
  - Automatic content type detection based on Content-Disposition headers
  - SHA-256 checksum validation from MIME epilogue
  - Robust parsing with fallback strategies for edge cases

- **ASN.1 Signature Parsing and Verification**
  - PKCS#7/CMS signature extraction from MIME attachments using `cms` crate
  - **Full RSA PKCS#1 v1.5 signature verification** with SHA-256, SHA-384, SHA-512
  - X.509 certificate extraction and validation
  - Certificate chain information (subject, issuer, validity periods)
  - Signature and digest algorithm detection (SHA-256, SHA-384, SHA-512)
  - Certificate and signer counting
  - Base64 decoding for text-encoded signatures
  - Detailed verification status and error reporting
  - **Subject Key Identifier (SKI) support**:
    - Signatures use SKI instead of embedding certificates
    - SKI can be used directly with `/v1/certs/{ski}` endpoint
    - Same SKI works with `/v1/ocsp/{ski}` for revocation checking
    - Eliminates need for certificate stores or complex matching logic
  - Complete PKI workflow implementation:
    - Extract SKI from signature
    - Fetch certificate using SKI
    - Check certificate status via OCSP
    - Extract public key for verification
    - **Verify signatures with proper signed attributes handling**
  - RSA public key extraction from certificates
  - Support for both IssuerAndSerialNumber and SubjectKeyIdentifier
  - **CMS signed attributes support**:
    - Proper handling of signatures with signed attributes
    - Automatic detection of direct vs. indirect signatures
    - DER encoding of signed attributes for verification
    - Support for content type, message digest, and signing time attributes

- **Data Processing**
  - PSV (Pipe-Separated Values) format parsing
  - Automatic data extraction from MIME structures
  - Raw response access for custom parsing needs
  - Type-safe response handling with proper error types

- **Error Handling**
  - Comprehensive error types using `thiserror`
  - Network error handling with descriptive messages
  - Protocol-specific error types (MIME parsing, checksum validation)
  - Graceful degradation for malformed responses

- **Examples**
  - `basic_usage.rs` - Introduction to client usage and common endpoints
  - `parse_versions.rs` - PSV data parsing and field extraction
  - `wow_products.rs` - Multi-product queries for WoW variants
  - `mime_parsing.rs` - MIME structure handling and checksum validation
  - `signature_parsing.rs` - ASN.1 signature parsing demonstration
  - `signature_verification.rs` - Enhanced signature verification with certificate details
  - `public_key_extraction.rs` - Extract public keys from signatures
  - `complete_signature_verification.rs` - Full signature verification workflow
  - `fetch_certificate_by_ski.rs` - Fetch certificates using SKI
  - `verify_ski_certificate.rs` - Verify SKI-based certificate fetching
  - `check_ski_response.rs` - Test SKI with different endpoints
  - `complete_pki_workflow.rs` - Complete PKI demonstration with SKI
  - `test_signature_verification.rs` - Test full signature verification
  - `full_signature_verification.rs` - Comprehensive verification demo
  - `debug_signature_data.rs` - Debug what data is signed
  - `debug_signed_attributes.rs` - Analyze CMS signed attributes
  - `parse_ocsp_response.rs` - Parse OCSP responses
  - `decode_ocsp_response.rs` - Decode and analyze OCSP data
  - `check_ocsp_endpoint.rs` - Test OCSP endpoint functionality
  - `debug_mime.rs` - Debug tool for MIME structure analysis
  - `raw_debug.rs` - Raw response debugging utility
  - `trace_debug.rs` - Trace-level debugging example
  - `analyze_bpsv_types.rs` - Analyze BPSV field types across endpoints
  - `compare_v1_v2_formats.rs` - Compare V1 MIME vs V2 raw BPSV responses
  - `explore_tact_endpoints.rs` - Explore TACT endpoints and BPSV data formats

- **Testing**
  - 25 unit tests covering core functionality (including enhanced signature verification)
  - 14 integration tests for end-to-end scenarios
  - 3 CMS parser integration tests
  - Mock server tests for offline development
  - Edge case testing for malformed responses
  - Performance benchmarks using Criterion

- **Documentation**
  - Comprehensive API documentation with examples
  - Module-level usage guides
  - Inline code examples for all public APIs
  - Updated Ribbit protocol documentation with SKI discovery
  - Detailed PKI workflow documentation
  - Certificate fetching and OCSP verification guides

#### `tact-client` crate

- **HTTP Client Implementation**
  - Support for both TACT protocol versions:
    - V1: TCP-based on port 1119 (`http://{region}.patch.battle.net:1119`)
    - V2: HTTPS-based REST API (`https://{region}.version.battle.net/v2/products`)
  - Async operations using tokio and reqwest
  - Region support for all major regions: US, EU, KR, CN, TW
  - Builder pattern for client configuration
  - Connection timeout configuration (30 seconds default)
  - **Typed response parsing using ngdp-bpsv crate**:
    - `get_versions_parsed()` - Returns `Vec<VersionEntry>`
    - `get_cdns_parsed()` - Returns `Vec<CdnEntry>`
    - `get_bgdl_parsed()` - Returns `Vec<BgdlEntry>`
  - **Added automatic retry support with exponential backoff**:
    - Configurable retry behavior with builder pattern methods
    - Default of 0 retries maintains backward compatibility
    - Exponential backoff with configurable parameters:
      - `with_max_retries()` - Set maximum retry attempts (default: 0)
      - `with_initial_backoff_ms()` - Initial backoff duration (default: 100ms)
      - `with_max_backoff_ms()` - Maximum backoff cap (default: 10 seconds)
      - `with_backoff_multiplier()` - Backoff growth factor (default: 2.0)
      - `with_jitter_factor()` - Randomness to prevent thundering herd (default: 0.1)
    - Retries network errors and specific HTTP status codes:
      - Connection failures, timeouts, send/receive errors
      - HTTP 5xx server errors
      - HTTP 429 Too Many Requests
    - Non-retryable errors fail immediately
    - Added example `retry_handling.rs` demonstrating retry strategies

- **Available Endpoints**
  - `/{product}/versions` - Version manifest with build configurations
  - `/{product}/cdns` - CDN configuration and hosts
  - `/{product}/bgdl` - Background downloader manifest
  - Note: TACT v2 endpoints mirror v1 endpoints with identical response formats

- **CDN Data Handling**
  - Consistent parsing of CDN hosts and servers fields
  - Both fields parsed as `Vec<String>` from space-separated lists
  - Support for legacy hosts field (bare hostnames)
  - Support for modern servers field (full URLs with protocols)
  - Comprehensive documentation on CDN usage patterns
  - Examples demonstrating URL construction for both fields

- **Enhanced Error Handling**
  - Comprehensive error types with contextual information:
    - Network errors: `Http`, `CdnExhausted`, `ConnectionTimeout`
    - Data format errors: `InvalidManifest`, `MissingField`, `InvalidHash`, `ChecksumMismatch`
    - Configuration errors: `InvalidRegion`, `UnsupportedProduct`, `InvalidProtocolVersion`
    - File errors: `FileNotFound`, `Io`
  - Helper methods for common error construction

- **Performance Benchmarks**
  - Benchmarks for response parsing using Criterion
  - Tests for versions and CDN manifest parsing
  - Benchmarks for empty servers and large datasets
  - Performance metrics for typical TACT responses

- **Content Download Support**
  - `download_file()` method for CDN file retrieval
  - Hash-based directory structure: `/{hash[0:2]}/{hash[2:4]}/{hash}`
  - Proper 404 error handling for missing files

- **Examples**
  - `basic_usage.rs` - Introduction to TACT client usage
  - `explore_endpoints.rs` - Endpoint discovery and testing tool
  - `test_v1_summary.rs` - V1 protocol endpoint testing
  - `test_different_products.rs` - Multi-product endpoint testing
  - `test_certs_endpoint.rs` - Certificate endpoint exploration

- **Testing**
  - Unit tests for all core functionality
  - Integration tests for client behavior
  - Error handling tests with comprehensive coverage
  - Protocol version and region switching tests

- **Documentation**
  - Comprehensive README with endpoint documentation
  - Response format examples for all endpoints
  - Supported products list with testing status
  - Error handling comparison with reference implementations

#### `ngdp-client` crate

- **CLI Application**
  - Comprehensive command-line interface for NGDP operations
  - Built with clap using derive API for clean command structure
  - Support for multiple output formats: text, JSON, pretty JSON, BPSV
  - Global options for logging level and configuration file

- **Command Structure**
  - `products` - Query product information from Ribbit
    - `list` - List available products with optional filtering
    - `versions` - Show version information for a product
    - `cdns` - Display CDN configuration
    - `info` - Get detailed product information
  - `storage` - Manage local CASC storage (placeholder)
    - `init` - Initialize new storage
    - `info` - Show storage information
    - `verify` - Check storage integrity
    - `clean` - Remove unused data
  - `download` - Download content using TACT (placeholder)
    - `build` - Download specific build
    - `files` - Download specific files
    - `resume` - Resume interrupted download
  - `inspect` - Inspect NGDP data structures
    - `bpsv` - Parse and display BPSV data (functional)
    - `build-config` - Inspect build configuration (placeholder)
    - `cdn-config` - Inspect CDN configuration (placeholder)
    - `encoding` - Show encoding information (placeholder)
  - `config` - Manage configuration
    - `show` - Display current configuration
    - `set` - Set configuration value
    - `get` - Get configuration value
    - `reset` - Reset to defaults

- **Features**
  - Async command handlers using Tokio
  - Structured logging with tracing
  - Library and binary dual-purpose design
  - Comprehensive error handling
  - Region support for all Blizzard regions
  - Beautiful terminal output with tables and colors
  - Respects `NO_COLOR` environment variable and `--no-color` flag

- **Terminal Output Formatting**
  - Added `comfy-table` (7.1.1) for beautiful Unicode tables with rounded corners
  - Added `owo-colors` (4.2.1) for colored terminal output with automatic detection
  - Created comprehensive output formatting module with:
    - Consistent color scheme (blue headers, green success, yellow warnings, red errors)
    - Unicode box-drawing characters for professional appearance
    - Proper alignment for different data types
    - Count badges for collections (e.g., "(59 products)")
    - Special formatting for URLs (underlined), hashes (dimmed+italic), and paths
  - All commands now use formatted output in text mode:
    - Products list: Table with product names, sequence numbers, and flags column
    - Products info: Hierarchical sections with key-value pairs and tables
    - Products versions: Region-based tables with all hash values (Build, CDN, Product, Key Ring)
    - Products cdns: Formatted CDN hosts with bullet points
    - Config show: Sorted configuration in a clean table
    - Inspect bpsv: Schema tables and data preview tables
  - Support for ASCII-only output when Unicode is not available
  - Dynamic table width adjustment (up to 200 chars) for displaying full hash values

- **Testing**
  - 8 integration tests for CLI functionality
  - Command help and version testing
  - Output format verification
  - Error handling tests

- **Examples**
  - `query_products.rs` - Using ngdp-client as a library
  - `cached_certificate_fetch.rs` - Demonstrates integrating CachedRibbitClient for certificate caching

- **Documentation**
  - Comprehensive README with usage examples
  - Command-line help for all commands
  - API documentation for library usage

### Project Infrastructure

- Workspace configuration with shared dependencies
- Added `clap` (4.5) to workspace dependencies for CLI applications
- Development tooling configuration (`.editorconfig`, `.gitattributes`)
- Consistent code formatting and style guidelines
- BPSV format documentation in `docs/bpsv-format.md`
- Updated Ribbit protocol documentation to include CN region access restrictions
  - Added warnings about CN server accessibility from outside China
  - Documented connection timeout recommendations
  - Added troubleshooting guidance for regional restrictions

### Dependencies

#### `ngdp-bpsv`

- `thiserror` (2.0) - Error type derivation (workspace dependency)
- `serde` (1.0) - Optional serialization support

#### `ribbit-client`

- `asn1` (0.21) - ASN.1 signature parsing
- `base64` (0.22) - Base64 encoding/decoding
- `cms` (0.2) - PKCS#7/CMS signature parsing
- `der` (0.7) - DER encoding/decoding for certificates
- `digest` (0.10) - Cryptographic digest traits
- `dirs` (6.0) - Platform-specific directory paths (workspace dependency)
- `hex` (0.4) - Hex encoding/decoding for certificates and SKI
- `mail-parser` (0.11) - MIME message parsing
- `rand` (0.9) - Random number generation for retry jitter
- `rsa` (0.9) - RSA signature verification
- `sha2` (0.10) - SHA-256/384/512 checksum validation
- `thiserror` (2.0) - Error type derivation
- `tokio` (1.45) - Async runtime with full features
- `tracing` (0.1) - Structured logging and debugging
- `x509-cert` (0.2) - X.509 certificate parsing and validation

#### `ngdp-cache`

- `dirs` (6.0) - Platform-specific directory paths (workspace dependency)
- `ribbit-client` (0.1) - For CachedRibbitClient functionality (workspace dependency)
- `thiserror` (2.0) - Error type derivation (workspace dependency)
- `tokio` (1.45) - Async runtime with fs and io-util features (workspace dependency)
- `tracing` (0.1) - Structured logging (workspace dependency)

#### `tact-client`

- `rand` (0.9) - Random number generation for retry jitter (workspace dependency)
- `reqwest` (0.12) - HTTP client with JSON and stream features
- `thiserror` (2.0) - Error type derivation (workspace dependency)
- `tokio` (1.45) - Async runtime with full features (workspace dependency)
- `tracing` (0.1) - Structured logging (workspace dependency)

#### `ngdp-client`

- `clap` (4.5) - Command-line argument parsing with derive API (workspace dependency)
- `comfy-table` (7.1.1) - Terminal table formatting with Unicode support
- `ngdp-bpsv` (0.1) - BPSV parsing for inspect commands (workspace dependency)
- `ngdp-cache` (0.1) - Caching functionality (workspace dependency)
- `owo-colors` (4.2.1) - Terminal color support with automatic detection
- `reqwest` (0.12) - HTTP client for fetching remote BPSV data
- `ribbit-client` (0.1) - Ribbit protocol client (workspace dependency)
- `serde` (1.0) - Serialization for JSON output (workspace dependency)
- `serde_json` (1.0) - JSON formatting (workspace dependency)
- `tokio` (1.45) - Async runtime (workspace dependency)
- `tracing` (0.1) - Structured logging (workspace dependency)
- `tracing-subscriber` (0.3) - Logging implementation

#### Development Dependencies

- `criterion` (0.6) - Benchmarking framework (workspace dependency)
- `tokio-test` (0.4) - Testing utilities for async code
- `tracing-subscriber` (0.3) - Logging implementation for examples
- `regex` (1.11) - Regular expression support for tests
- `serde_json` (1.0) - JSON support for tests

[0.4.0]: https://github.com/wowemulation-dev/cascette-rs/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/wowemulation-dev/cascette-rs/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/wowemulation-dev/cascette-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/wowemulation-dev/cascette-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/wowemulation-dev/cascette-rs/releases/tag/v0.1.0

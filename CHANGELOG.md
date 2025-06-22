# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/wowemulation-dev/cascette-rs/compare/main...HEAD

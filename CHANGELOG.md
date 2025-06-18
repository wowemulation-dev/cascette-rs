# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

#### `ribbit-client` crate

- Replaced custom base64 decoder with `base64` crate (0.22) for better reliability and performance
- Improved code quality following Rust best practices:
  - Added clippy pedantic lints for stricter code standards
  - Fixed all format string warnings to use inline variables
  - Added `#[must_use]` attributes to all applicable methods
  - Added comprehensive error documentation with `# Errors` sections
  - Fixed unnecessary `Result` wrapping in internal functions
  - Improved code organization and reduced redundancy

### Added

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

- **ASN.1 Signature Parsing**
  - PKCS#7/CMS signature extraction from MIME attachments
  - Basic ASN.1 structure validation
  - Signature algorithm detection (SHA-256)
  - Certificate and signer counting
  - Base64 decoding for text-encoded signatures

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
  - `debug_mime.rs` - Debug tool for MIME structure analysis
  - `raw_debug.rs` - Raw response debugging utility
  - `trace_debug.rs` - Trace-level debugging example

- **Testing**
  - 21 unit tests covering core functionality (including signature parsing)
  - 14 integration tests for end-to-end scenarios
  - Mock server tests for offline development
  - Edge case testing for malformed responses
  - Performance benchmarks using Criterion

- **Documentation**
  - Comprehensive API documentation with examples
  - Module-level usage guides
  - inline code examples for all public APIs

#### `tact-client` crate

- Initial crate setup for future TACT protocol implementation
- Placeholder library structure

### Project Infrastructure

- Workspace configuration with shared dependencies
- Development tooling configuration (`.editorconfig`, `.gitattributes`)
- Consistent code formatting and style guidelines

### Dependencies

#### `ribbit-client`

- `asn1` (0.21) - ASN.1 signature parsing
- `base64` (0.22) - Base64 encoding/decoding
- `mail-parser` (0.11) - MIME message parsing
- `sha2` (0.10) - SHA-256 checksum validation
- `thiserror` (2.0) - Error type derivation
- `tokio` (1.45) - Async runtime with full features
- `tracing` (0.1) - Structured logging and debugging

#### Development Dependencies

- `criterion` (0.6) - Benchmarking framework
- `tokio-test` - Testing utilities for async code
- `tracing-subscriber` - Logging implementation for examples

[Unreleased]: https://github.com/wowemulation-dev/cascette-rs/compare/main...HEAD

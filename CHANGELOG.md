# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

- Initial crate setup for future TACT protocol implementation
- Placeholder library structure

### Project Infrastructure

- Workspace configuration with shared dependencies
- Development tooling configuration (`.editorconfig`, `.gitattributes`)
- Consistent code formatting and style guidelines
- BPSV format documentation in `docs/bpsv-format.md`

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
- `rsa` (0.9) - RSA signature verification
- `sha2` (0.10) - SHA-256/384/512 checksum validation
- `thiserror` (2.0) - Error type derivation
- `tokio` (1.45) - Async runtime with full features
- `tracing` (0.1) - Structured logging and debugging
- `x509-cert` (0.2) - X.509 certificate parsing and validation

#### Development Dependencies

- `criterion` (0.6) - Benchmarking framework (workspace dependency)
- `tokio-test` (0.4) - Testing utilities for async code
- `tracing-subscriber` (0.3) - Logging implementation for examples
- `regex` (1.11) - Regular expression support for tests
- `serde_json` (1.0) - JSON support for tests

[Unreleased]: https://github.com/wowemulation-dev/cascette-rs/compare/main...HEAD

# ribbit-client Examples

This directory contains comprehensive examples demonstrating how to use the `ribbit-client` crate for accessing Blizzard's Ribbit version service.

## Basic Usage Examples

### `ribbit_basic_usage.rs`
Introduction to the Ribbit client with common operations:
- Creating a client for different regions
- Querying product summaries
- Getting product versions
- Basic error handling

### `simple_typed_usage.rs`  
Demonstrates the typed API for easy data access:
- Using typed response methods
- Accessing parsed data structures
- Working with version entries

### `raw_vs_typed.rs`
Compares raw response access vs typed API:
- Raw response handling
- Typed response benefits
- Performance considerations

## Protocol Examples

### `compare_v1_v2_formats.rs`
Shows differences between Ribbit V1 (MIME) and V2 (raw) protocols:
- Protocol version switching
- Response format differences
- Compatibility considerations

### `timeout_handling.rs`
Demonstrates connection timeout handling:
- Setting custom timeouts
- Handling region accessibility (especially CN)
- Error recovery strategies

### `retry_handling.rs`
Shows automatic retry configuration:
- Exponential backoff settings
- Retry strategies for different error types
- Custom retry parameters

## Data Processing Examples

### `parse_versions.rs`
Advanced parsing of version data:
- Extracting specific fields from responses
- Working with BPSV data structures
- Data validation and error handling

### `wow_products.rs`
Multi-product queries for WoW variants:
- Querying different WoW products (wow, wow_classic, etc.)
- Comparing versions across products
- Regional differences

### `analyze_bpsv_types.rs`
Analyzes BPSV field types across endpoints:
- Field type discovery
- Schema analysis
- Data format understanding

## MIME and Signature Examples

### `mime_parsing.rs`
MIME message structure handling:
- MIME multipart parsing
- Content extraction
- Checksum validation

### `signature_parsing.rs`
ASN.1 signature parsing demonstration:
- CMS/PKCS#7 signature extraction
- Certificate information
- Signature metadata

### `signature_verification.rs`
Basic signature verification:
- RSA signature verification
- Public key extraction
- Certificate validation

### `complete_signature_verification.rs`
Enhanced signature verification with full certificate details:
- Complete PKI workflow
- Certificate chain information
- Detailed verification status

## Certificate and PKI Examples

### `fetch_certificate_by_ski.rs`
Certificate retrieval using Subject Key Identifier:
- SKI-based certificate fetching
- Certificate details extraction
- Error handling for missing certificates

### `verify_ski_certificate.rs`
Verifies SKI-based certificate operations:
- SKI extraction from signatures
- Certificate validation
- Cross-reference verification

### `complete_pki_workflow.rs`
Complete PKI demonstration:
- End-to-end signature workflow
- Certificate fetching and validation
- OCSP response handling

### `public_key_extraction.rs`
Extract public keys from certificates:
- RSA public key extraction
- Key format conversion
- Usage in signature verification

## OCSP Examples

### `check_ocsp_endpoint.rs`
Tests OCSP endpoint functionality:
- OCSP request/response handling
- Certificate revocation checking
- Status interpretation

### `parse_ocsp_response.rs`
Parse and analyze OCSP responses:
- OCSP response structure
- Status extraction
- Certificate validation integration

### `decode_ocsp_response.rs`
Detailed OCSP response decoding:
- ASN.1 structure analysis
- Response data extraction
- Error handling

## Advanced Examples

### `cdn_consistency.rs`
Checks consistency between Ribbit and TACT data:
- Cross-protocol validation
- Data integrity checking
- Endpoint comparison

### `explore_tact_endpoints.rs`
Explores TACT endpoints and data formats:
- Endpoint discovery
- Response format analysis
- Protocol comparison

### `typed_api_showcase.rs`
Comprehensive typed API demonstration:
- All endpoint types
- Error handling patterns
- Best practices

## Debug and Analysis Examples

### `debug_mime.rs`
Debug tool for MIME structure analysis:
- MIME parsing deep dive
- Structure visualization
- Troubleshooting tools

### `debug_signature_data.rs`
Analyzes what data is actually signed:
- Signature payload extraction
- Data format analysis
- Verification debugging

### `debug_signed_attributes.rs`
CMS signed attributes analysis:
- Signed attributes structure
- Content type and digest analysis
- Timestamp information

### `trace_debug.rs`
Trace-level debugging example:
- Detailed logging setup
- Protocol trace analysis
- Performance debugging

### `debug_bpsv_format.rs`
BPSV format debugging and analysis:
- Field type analysis
- Data structure exploration
- Format validation

## Running Examples

To run any example:
```bash
cargo run --example <example_name> -p ribbit-client
```

For example:
```bash
cargo run --example ribbit_basic_usage -p ribbit-client
cargo run --example signature_verification -p ribbit-client
cargo run --example complete_pki_workflow -p ribbit-client
```

## Prerequisites

Some examples may require:
- Internet connection for Ribbit servers
- Specific certificates or signatures (examples handle missing data gracefully)
- Proper region access (CN region examples may timeout outside China)

Most examples include extensive error handling and will provide useful output even when encountering network issues or missing data.
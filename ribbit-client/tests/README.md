# ribbit-client Tests

This directory contains comprehensive tests for the `ribbit-client` crate.

## Test Categories

### Unit Tests (in source files)
Located in `src/` under `#[cfg(test)]` modules:

- **Client Tests** (`src/client.rs`) - Core client functionality
- **CMS Parser Tests** (`src/cms_parser.rs`) - PKCS#7/CMS signature parsing
- **DNS Cache Tests** (`src/dns_cache.rs`) - DNS resolution caching
- **Error Tests** (`src/error.rs`) - Error type functionality
- **Response Type Tests** (`src/response_types.rs`) - Typed response parsing
- **Signature Verification Tests** (`src/signature_verify.rs`) - RSA signature verification
- **Type Tests** (`src/types.rs`) - Protocol types and enums

### Integration Tests
Located in this `tests/` directory:

#### `integration_test.rs`
End-to-end testing with real Ribbit servers:
- Connection establishment
- Protocol version switching
- All endpoint types (summary, versions, CDNs, BGDL)
- Regional server testing
- Error condition handling
- Mixed valid/invalid request handling

#### `typed_response_test.rs`
Typed response API testing:
- Typed response parsing
- Field access methods
- Error handling for malformed data
- Response convenience methods

#### `signature_test.rs`
Signature parsing and verification:
- PKCS#7/CMS signature parsing
- RSA signature verification
- Certificate extraction
- Error handling for invalid signatures

## Test Coverage

### Protocol Features
- ✅ V1 (MIME) and V2 (raw) protocols
- ✅ All regions (US, EU, CN, KR, TW, SG)
- ✅ All endpoint types
- ✅ Retry logic and error handling
- ✅ DNS caching functionality

### MIME Processing
- ✅ Multipart MIME parsing
- ✅ Content extraction
- ✅ Checksum validation
- ✅ Signature attachment handling

### Cryptographic Features
- ✅ PKCS#7/CMS signature parsing
- ✅ RSA signature verification
- ✅ Certificate processing
- ✅ Subject Key Identifier (SKI) handling
- ✅ OCSP response parsing

### Data Processing
- ✅ BPSV format parsing
- ✅ Typed response generation
- ✅ Field validation and access
- ✅ Error condition handling

### Network Features
- ✅ Connection pooling
- ✅ Automatic retries
- ✅ DNS resolution caching
- ✅ Timeout handling
- ✅ Regional server failover

## Running Tests

```bash
# Run all ribbit-client tests
cargo test -p ribbit-client

# Run with output to see network requests
cargo test -p ribbit-client -- --nocapture

# Run specific test categories
cargo test -p ribbit-client client
cargo test -p ribbit-client integration
cargo test -p ribbit-client signature
cargo test -p ribbit-client typed_response

# Run tests with tracing enabled
RUST_LOG=debug cargo test -p ribbit-client
```

## Network Requirements

Integration tests require internet connectivity to reach Blizzard's Ribbit servers:
- `us.version.battle.net` (US region)
- `eu.version.battle.net` (EU region)  
- `kr.version.battle.net` (KR region)
- `tw.version.battle.net` (TW region)
- `cn.version.battle.net` (CN region - may timeout outside China)

Tests are designed to handle network failures gracefully and will skip or report connection issues rather than failing completely.

## Test Data

Tests use a combination of:
- **Live data** from Ribbit servers for integration testing
- **Synthetic data** for unit testing edge cases
- **Known signatures** for cryptographic verification testing
- **Mock responses** for error condition testing

This ensures comprehensive coverage while maintaining test reliability across different environments.
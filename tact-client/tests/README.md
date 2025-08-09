# tact-client Tests

This directory contains comprehensive tests for the `tact-client` crate.

## Test Categories

### Unit Tests (in source files)

Located in `src/` under `#[cfg(test)]` modules:

- **HTTP Client Tests** (`src/http.rs`) - Core HTTP functionality
- **Region Tests** (`src/region.rs`) - Region handling and validation
- **Response Type Tests** (`src/response_types.rs`) - Typed response parsing
- **Error Tests** (`src/error.rs`) - Error type functionality

### Integration Tests

Located in this `tests/` directory:

#### `integration_test.rs`

End-to-end testing with real TACT servers:

- Connection establishment for both V1 and V2 protocols
- All endpoint types (versions, CDNs, BGDL)
- Regional server testing (US, EU, KR, etc.)
- Error condition handling
- Protocol version switching
- Response parsing validation

#### `typed_response_test.rs`

Typed response API testing:

- Parsing version entries with all fields
- CDN configuration parsing
- BGDL manifest processing
- Error handling for malformed responses
- Field access methods

## Test Coverage

### Protocol Features

- ✅ V1 (HTTP port 1119) and V2 (HTTPS REST) protocols
- ✅ All regions (US, EU, CN, KR, TW, SG)
- ✅ All endpoint types (versions, cdns, bgdl)
- ✅ Retry logic with exponential backoff
- ✅ Connection timeout handling
- ✅ Custom user agent support

### Response Processing

- ✅ BPSV format parsing
- ✅ Typed response generation
- ✅ Field validation and access
- ✅ CDN hosts/servers field parsing
- ✅ Version entry processing

### Network Features

- ✅ Connection pooling
- ✅ Automatic retries for transient failures
- ✅ HTTP error handling (4xx, 5xx)
- ✅ Timeout configuration
- ✅ URL construction for different protocols

### Error Handling

- ✅ Network connectivity failures
- ✅ Invalid endpoint responses
- ✅ Malformed BPSV data
- ✅ Missing required fields
- ✅ Protocol version incompatibility

## Running Tests

```bash
# Run all tact-client tests
cargo test -p tact-client

# Run with output to see network requests
cargo test -p tact-client -- --nocapture

# Run specific test categories
cargo test -p tact-client http
cargo test -p tact-client integration
cargo test -p tact-client response_types
cargo test -p tact-client region

# Run tests with network debugging
RUST_LOG=debug cargo test -p tact-client
```

## Network Requirements

Integration tests require internet connectivity to reach Blizzard's TACT servers:

### V1 Protocol (HTTP)

- `us.patch.battle.net:1119`
- `eu.patch.battle.net:1119`
- `kr.patch.battle.net:1119`
- `cn.patch.battle.net:1119`

### V2 Protocol (HTTPS)

- `us.version.battle.net`
- `eu.version.battle.net`
- `kr.version.battle.net`
- `cn.version.battle.net`

Tests handle network failures gracefully and will skip or report connection issues rather than failing completely.

## Test Data

Tests use a combination of:

- **Live data** from TACT servers for integration testing
- **Synthetic BPSV** for unit testing edge cases
- **Known product responses** for validation testing
- **Mock responses** for error condition testing

## Performance Testing

Some tests include performance measurements:

- **Response parsing speed** for different data sizes
- **Connection establishment time** across protocols
- **Retry mechanism overhead** with various backoff strategies
- **Memory usage** for large response processing

Run performance tests:

```bash
cargo test -p tact-client --release -- --ignored
```

This ensures comprehensive coverage while maintaining test reliability across different network environments and server conditions.

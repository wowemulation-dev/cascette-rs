# ngdp-cdn Tests

This directory contains comprehensive tests for the `ngdp-cdn` crate.

## Test Categories

### Unit Tests (in source files)
Located in `src/` under `#[cfg(test)]` modules:

- **Client Tests** (`src/client.rs`) - Core client functionality and builder
- **Fallback Tests** (`src/fallback.rs`) - CDN fallback and backup server logic

### Integration Tests
Located in this `tests/` directory:

#### `integration_test.rs`
End-to-end testing with CDN operations:
- Real CDN server connections
- File download operations
- Error condition handling
- Performance measurement
- Content verification

#### `fallback_tests.rs`
CDN fallback mechanism testing:
- Primary CDN failure scenarios
- Automatic fallback to backup servers
- Community mirror integration
- Custom CDN configuration
- Load distribution testing

## Test Coverage

### Client Configuration
- ✅ Builder pattern configuration
- ✅ Connection timeout settings
- ✅ Request timeout configuration
- ✅ Connection pool sizing
- ✅ Custom user agent support

### Download Operations
- ✅ Single file downloads
- ✅ Parallel download operations
- ✅ Streaming downloads for large files
- ✅ Progress tracking and callbacks
- ✅ Content verification

### Retry Logic
- ✅ Exponential backoff implementation
- ✅ Jitter factor application
- ✅ Maximum retry limits
- ✅ Transient error handling
- ✅ Rate limit detection and handling

### CDN Infrastructure
- ✅ Primary CDN server connections
- ✅ Backup server fallback logic
- ✅ Community mirror integration
- ✅ Regional CDN handling
- ✅ Load balancing verification

### Error Handling
- ✅ Network connectivity failures
- ✅ HTTP error responses (4xx, 5xx)
- ✅ Content not found scenarios
- ✅ Timeout conditions
- ✅ Content verification failures

## Running Tests

```bash
# Run all ngdp-cdn tests
cargo test -p ngdp-cdn

# Run with output to see CDN requests
cargo test -p ngdp-cdn -- --nocapture

# Run specific test categories
cargo test -p ngdp-cdn client
cargo test -p ngdp-cdn fallback
cargo test -p ngdp-cdn integration

# Run performance tests
cargo test -p ngdp-cdn --release -- performance

# Run with network debugging
RUST_LOG=debug cargo test -p ngdp-cdn
```

## Network Requirements

Tests may require internet connectivity to reach:

### Primary CDN Servers
- `us.patch.battle.net`
- `eu.patch.battle.net`
- `kr.patch.battle.net`
- `cn.patch.battle.net`

### Backup CDN Mirrors
- `cdn.arctium.tools` (Community mirror)
- `tact.mirror.reliquaryhq.com` (Community mirror)

Tests handle network failures gracefully and will skip network-dependent tests when connectivity is unavailable.

## Performance Testing

Integration tests include performance measurements:
- **Download speed** testing with various file sizes
- **Parallel download** throughput measurement
- **Connection establishment** timing
- **Retry mechanism** overhead analysis
- **Memory usage** validation for streaming operations

### Performance Benchmarks

Expected performance characteristics:
- Single file downloads: >10 MB/s on good connections
- Parallel downloads: 3-5x speedup with 4-8 concurrent connections
- Memory usage: Constant for streaming operations regardless of file size
- Connection overhead: <100ms for connection establishment

## Test Data

Tests use a combination of:
- **Real CDN files** for integration testing (small files to minimize bandwidth)
- **Synthetic test data** for unit testing
- **Error injection** for failure scenario testing
- **Performance measurement data** for optimization validation

## Mock Testing

Some tests use mock servers to:
- Test error conditions reliably
- Simulate rate limiting scenarios
- Test retry logic with controlled failures
- Validate fallback mechanisms

## Content Types

Tests verify handling of all CDN content types:
- **Config files** (`.config` paths)
- **Data files** (`.data` paths)  
- **Patch files** (`.patch` paths)
- **Index files** (`.index` paths)

This ensures comprehensive functionality across Blizzard's entire CDN infrastructure.
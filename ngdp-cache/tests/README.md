# ngdp-cache Tests

This directory contains comprehensive tests for the `ngdp-cache` crate covering all caching functionality.

## Test Files

### Integration Tests
- `cache_validity_test.rs` - Cache validation and expiration testing
- `cached_cdn_client_integration.rs` - CDN client caching integration
- `cached_cdn_helper_tests.rs` - CDN helper method functionality
- `cached_ribbit_client_test.rs` - Ribbit client caching functionality
- `cached_tact_client_integration.rs` - TACT client caching integration
- `cached_tact_client_test.rs` - TACT client unit testing
- `integration_test.rs` - Cross-cache integration scenarios
- `ribbit_cache_structure_test.rs` - Ribbit cache organization validation
- `ribbit_cdn_integration.rs` - Cross-protocol integration testing

## Test Coverage

### Core Cache Types
- **GenericCache**: Key-value operations, TTL handling, concurrent access
- **RibbitCache**: Protocol-specific caching, sequence numbers, TTL strategies
- **CdnCache**: Hash-based paths, content types, product separation
- **TactCache**: Metadata caching, endpoint isolation, region separation

### Client Integration
- **CachedRibbitClient**: Drop-in replacement functionality, API compatibility
- **CachedCdnClient**: Content caching, streaming operations, statistics
- **CachedTactClient**: Metadata caching, sequence tracking, TTL management

### Advanced Features
- **Streaming I/O**: Memory-efficient large file operations
- **Batch operations**: Parallel read/write/delete operations
- **TTL management**: Expiration, cleanup, validation
- **Concurrent access**: Thread safety, race condition handling
- **Error handling**: Network failures, corruption detection, recovery

### Performance Testing
- **Large file handling**: >10MB file operations
- **Concurrent operations**: Multi-threaded access patterns
- **Cache statistics**: Hit/miss ratios, performance metrics
- **Memory usage**: Constant memory for streaming operations

## Cache Structure Testing

Tests verify correct cache organization:
```
~/.cache/ngdp/
├── ribbit/{region}/{protocol}/{endpoint}-{sequence}.bpsv
├── tact/{region}/{protocol}/{product}/{endpoint}-{sequence}.bpsv
├── cdn/{type}/{hash[0:2]}/{hash[2:4]}/{hash}
└── generic/{key}
```

### Directory Validation
- Platform-specific cache root directories
- Hash-based path construction
- Region and protocol isolation
- Product-specific organization
- Content type separation

### Cache Isolation Testing
- Cross-cache type isolation
- Region-based separation
- Protocol version isolation
- Product-specific boundaries
- Concurrent access safety

## Running Tests

```bash
# Run all ngdp-cache tests
cargo test -p ngdp-cache

# Run with output to see cache operations
cargo test -p ngdp-cache -- --nocapture

# Run specific test categories
cargo test -p ngdp-cache cache_validity
cargo test -p ngdp-cache cached_ribbit_client
cargo test -p ngdp-cache cached_cdn_client
cargo test -p ngdp-cache integration

# Run performance-intensive tests
cargo test -p ngdp-cache large_file_handling
cargo test -p ngdp-cache concurrent_cache_access

# Run with timing information
cargo test -p ngdp-cache --release
```

## Test Environment

Tests create temporary cache directories to avoid interfering with user data:
- Uses system temp directories for test isolation
- Cleans up test data automatically
- Supports parallel test execution
- Handles platform-specific path formats

### Performance Benchmarks

Some tests include performance measurements:
- **Streaming operations**: Memory usage validation
- **Batch operations**: Throughput measurement  
- **Large files**: Processing time analysis
- **Concurrent access**: Contention measurement

## Network Requirements

Integration tests may require network access for:
- Real Ribbit server responses (cached for subsequent runs)
- CDN content downloads (minimal, uses small test files)
- TACT endpoint testing (metadata only)

Tests handle network failures gracefully and use cached data when available to ensure reliability in offline environments.
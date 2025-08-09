# casc-storage Tests

This directory contains comprehensive tests for the `casc-storage` crate, covering CASC (Content Addressable Storage Container) functionality.

## Test Files

### Core Functionality
- `binary_search_test.rs` - Binary search algorithm tests for index lookups
- `lazy_loading_tests.rs` - Lazy loading and on-demand file access
- `zero_copy_test.rs` - Zero-copy operations and memory efficiency

### Performance and Scaling
- `large_archive_test.rs` - Handling of large CASC archives (>2GB)
- `parallel_loading.rs` - Concurrent file loading and thread safety
- `lockfree_cache_test.rs` - Lock-free cache implementation tests

### Progressive Loading
- `progressive_loading_test.rs` - Progressive/streaming file access
- `test_progressive_integration.rs` - Integration tests for progressive loading

### Real Data
- `real_data_test.rs` - Tests with actual CASC data files
- `tact_manifest_tests.rs` - TACT manifest parsing and integration

## Test Coverage

The tests verify:

### Storage Operations
- Opening and reading CASC storage
- Index file parsing and lookups
- Archive file access and extraction
- Memory-mapped file operations

### Performance Features
- Lazy loading of large files
- Progressive chunk loading
- Zero-copy data access
- Parallel file operations
- Lock-free caching

### Data Integrity
- Checksum verification
- File existence checks
- Proper error handling
- Edge case handling

### Integration
- TACT manifest integration
- Real game data compatibility
- Multi-archive support
- Cross-platform functionality

## Running Tests

```bash
# Run all casc-storage tests
cargo test -p casc-storage

# Run with output
cargo test -p casc-storage -- --nocapture

# Run specific test categories
cargo test -p casc-storage binary_search
cargo test -p casc-storage lazy_loading
cargo test -p casc-storage progressive
cargo test -p casc-storage parallel

# Run tests with real data (requires CASC storage)
cargo test -p casc-storage real_data -- --ignored
```

## Test Data Requirements

Some tests require:
- Sample CASC index files (.idx)
- Sample CASC archive files
- Valid TACT manifests
- Sufficient disk space for large file tests

Tests that require real CASC data are marked with `#[ignore]` and must be run explicitly.

## Performance Tests

Performance-critical tests measure:
- Index lookup speed
- Archive extraction throughput
- Memory usage patterns
- Concurrent access performance
- Cache hit rates

## Notes

- Tests use both synthetic and real CASC data
- Large archive tests may require significant memory
- Parallel tests verify thread safety
- Integration tests ensure compatibility with game clients
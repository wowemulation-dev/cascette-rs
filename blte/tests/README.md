# BLTE Tests

This directory contains comprehensive tests for the `blte` crate.

## Test Files

- `integration_tests.rs` - Integration tests covering all BLTE features

## Test Coverage

The tests cover:

### Compression Modes

- **None (N)** - Uncompressed data
- **ZLib (Z)** - Deflate compression
- **LZ4 (4)** - Fast compression
- **Frame (F)** - Nested BLTE frames
- **Encrypted (E)** - Salsa20/ARC4 encrypted blocks

### File Types

- Single-chunk files
- Multi-chunk files with multiple compression modes
- Large file simulation
- Encrypted content with both Salsa20 and ARC4

### Error Conditions

- Invalid headers
- Corrupted data
- Missing encryption keys
- Checksum verification failures

## Running Tests

```bash
# Run all BLTE tests
cargo test -p blte

# Run with output
cargo test -p blte -- --nocapture

# Run specific test
cargo test -p blte test_blte_file_structure
```

## Test Data

Tests use synthetic BLTE data generated in-memory to ensure consistent behavior across different environments. Real BLTE file testing is performed in higher-level integration tests in the `tact-parser` and `ngdp-client` crates.

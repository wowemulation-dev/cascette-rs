# tact-parser Tests

This directory contains comprehensive tests for the `tact-parser` crate covering all TACT file format parsers.

## Test Files

### Integration Tests

- `integration_test.rs` - Integration tests with real TACT file data
- `tvfs_tests.rs` - TVFS parser specific tests

### Unit Tests (in source files)

Located in `src/` under `#[cfg(test)]` modules for each parser

## Test Coverage by Parser

### Configuration Parser (`src/config.rs`)

- ✅ Build configuration parsing (key=value format)
- ✅ CDN configuration parsing
- ✅ Hash-size pair extraction
- ✅ Empty value handling
- ✅ Comment line processing
- ✅ Case-sensitive key handling

### Download Manifest Parser (`src/download.rs`)

- ✅ All versions (V1, V2, V3) support
- ✅ Priority-based file ordering
- ✅ Tag-based filtering
- ✅ Entry size validation
- ✅ Large file handling
- ✅ Download size calculations

### Encoding File Parser (`src/encoding.rs`)

- ✅ Big-endian header parsing
- ✅ 40-bit integer handling for file sizes
- ✅ CKey to EKey bidirectional mapping
- ✅ Multiple EKeys per CKey support
- ✅ MD5 checksum verification
- ✅ Large encoding file support

### Install Manifest Parser (`src/install.rs`)

- ✅ Tag system with bitmask support
- ✅ Platform-specific filtering
- ✅ Multi-platform installations
- ✅ Locale-specific files
- ✅ Complex tag combinations
- ✅ Large manifest handling

### Size File Parser (`src/size.rs`)

- ✅ Partial EKey to size mapping
- ✅ Tag-based size calculations
- ✅ Statistics generation (min/max/average)
- ✅ Largest files identification
- ✅ Installation space requirements
- ✅ Large file size handling

### TVFS Parser (`src/tvfs.rs`)

- ✅ TVFS/TFVS magic byte support
- ✅ Big-endian byte order handling
- ✅ Path table parsing
- ✅ VFS entry processing
- ✅ CFT table handling
- ✅ EST table support (currently ignored tests)

### Utilities (`src/utils.rs`)

- ✅ 40-bit integer reading/writing
- ✅ Variable-length integer (varint) support
- ✅ C-string parsing
- ✅ Boundary condition handling
- ✅ Overflow protection

## Test Data Categories

### Real-World Data Tests

- WoW build configurations from multiple versions
- PTR build configurations for testing
- Various product CDN configurations
- Known encoding file structures

### Synthetic Test Data

- Edge case file structures
- Boundary condition testing
- Error injection scenarios
- Performance stress testing

### Format Validation Tests

- Magic byte verification
- Header structure validation
- Field type checking
- Checksum verification

## Running Tests

```bash
# Run all tact-parser tests
cargo test -p tact-parser

# Run with output to see parsing details
cargo test -p tact-parser -- --nocapture

# Run specific parser tests
cargo test -p tact-parser encoding
cargo test -p tact-parser install
cargo test -p tact-parser download
cargo test -p tact-parser tvfs

# Run integration tests only
cargo test -p tact-parser integration

# Run ignored tests (require real data files)
cargo test -p tact-parser -- --ignored
```

## Test File Requirements

Some tests are marked `#[ignore]` and require real TACT files:

- Real encoding files for comprehensive testing
- Large manifest files for performance validation
- Encrypted file testing (requires proper keys)

These can be obtained from:

- WoW installation directories
- CDN downloads using other tools
- Community test data repositories

## Performance Testing

Tests include performance validation:

- **Parser speed** for different file sizes
- **Memory usage** for large files
- **Hash calculation** performance
- **40-bit integer** operation speed

Run performance tests:

```bash
cargo test -p tact-parser --release
```

## Error Condition Testing

Comprehensive error handling validation:

- **Truncated files** - Partial data scenarios
- **Invalid magic bytes** - Wrong file format detection
- **Corrupted headers** - Malformed structure handling
- **Invalid checksums** - Data integrity failures
- **Memory exhaustion** - Large file boundary testing

## Format Evolution Testing

Tests verify compatibility across format versions:

- Legacy vs modern root file formats
- Download manifest version differences (V1/V2/V3)
- TVFS vs TFVS magic byte handling
- Encoding file header variations

This ensures the parser remains compatible with content from different WoW expansions and patch versions.

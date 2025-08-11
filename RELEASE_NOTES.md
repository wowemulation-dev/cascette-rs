# Release Notes - v0.4.2

## Release Summary

cascette-rs v0.4.2 is a patch release that fixes critical issues with the v0.4.0 release workflow and Windows compilation. This release ensures all crates can be properly published to crates.io and builds successfully on all platforms.

## Key Fixes

- **Release Workflow**: Added missing casc-storage crate to publishing workflow
- **Windows Compilation**: Fixed unused variable warnings in casc-storage that caused build failures
- **Dependency Order**: Proper crates.io publishing order with casc-storage included
- **Test Utils**: Marked test-utils as non-publishable to fix crates.io dependency resolution

---

# Release Notes - v0.4.0

## Release Summary

cascette-rs v0.4.0 is a major release that achieves full TACT format specification compliance with significant performance improvements and enhanced safety. This release includes critical fixes for 40-bit integer parsing, complete ESpec EBNF grammar implementation, TVFS specification compliance, and major architectural improvements. The implementation has been validated against real World of Warcraft encoding files from Blizzard's CDN with comprehensive testing on both Rust 1.86 (MSRV) and 1.89 (stable).

## Key Highlights

### TACT Format Compliance ✅

- **40-bit Integer Support**: Complete implementation of 40-bit integers for TACT formats
  - Big-endian support for TVFS and encoding file headers
  - TACT encoding format (1 high byte + 4-byte big-endian u32) for file sizes
  - Successfully validated with real WoW encoding files up to 1TB
- **ESpec Parser**: Full EBNF grammar implementation for BLTE compression specifications
  - Complete parsing of all compression modes (n, z, e, b, c, g)
  - Support for complex block specifications and nested compression
  - 11 comprehensive tests covering all ESpec variants
- **TVFS Compliance**: Specification-compliant implementation
  - Fixed magic bytes to only accept 'TVFS' (0x53465654)
  - All offset/size fields use 40-bit big-endian integers
  - Header structure matches wowdev.wiki specification exactly

### Real Data Validation

- **Production Testing**: Successfully downloaded and parsed real WoW encoding files
  - 176MB decompressed encoding file with 124,062 content keys
  - Validated bidirectional CKey ↔ EKey lookups
  - Confirmed ESpec block detection and parsing
- **CDN Integration**: Proper use of CDN client for all downloads
  - No direct URL construction - uses official CDN endpoints
  - Supports community mirrors and fallback scenarios

### Performance & Safety Improvements

- **LRU Cache Optimization**: Reduced from O(n) to O(1) operations
  - Consolidated mutex-protected structures for better lock management
  - Significant performance gains in cache-heavy operations
- **Atomic Operations**: Replaced Arc<Mutex<u64>> with AtomicU64
  - Better concurrency with lock-free atomic counters
  - Reduced contention in multi-threaded scenarios
- **Memory Safety**: Eliminated all potential runtime panics
  - Fixed unsafe string slicing in ngdp-bpsv parser
  - Proper bounds checking in all parsers
  - Safe indexing operations throughout
- **Code Quality**: Major refactoring and deduplication
  - Created BpsvRowOps trait to eliminate duplicate logic
  - Consolidated HTTP retry logic in tact-client
  - Comprehensive QA with 520+ tests passing

### MSRV Compatibility

- **Rust 1.86 Support**: Full compatibility with minimum supported Rust version
  - Fixed 17 let-chain syntax instances across codebase
  - All unstable features removed
  - CI/CD validates both MSRV (1.86) and stable (1.89)

## Breaking Changes

None. This release maintains backward compatibility with all previous versions.

## Migration Guide

No migration required. Simply update your dependencies to version 0.4.0:

```toml
[dependencies]
ngdp-bpsv = "0.4.2"
ribbit-client = "0.4.2"
tact-client = "0.4.2"
tact-parser = "0.4.2"
ngdp-cdn = "0.4.2"
ngdp-cache = "0.4.2"
ngdp-crypto = "0.4.2"
blte = "0.4.2"
casc-storage = "0.4.2"
ngdp-client = "0.4.2"
```

## Installation

### Using the install script (Linux/macOS/Windows)

```bash
curl -fsSL https://raw.githubusercontent.com/wowemulation-dev/cascette-rs/main/install.sh | bash
```

### Using cargo-binstall

```bash
cargo binstall ngdp-client
```

### Using cargo

```bash
cargo install ngdp-client
```

### From Source

```bash
git clone https://github.com/wowemulation-dev/cascette-rs
cd cascette-rs
cargo build --release
```

## Changes in This Release

### Added

- **ESpec Parser** (`tact-parser/src/espec.rs`)
  - Complete EBNF grammar parser for BLTE compression specifications
  - Support for all compression modes: None, ZLib, Encrypted, BlockTable, BCPack, GDeflate
  - Complex block specification parsing with size expressions
  - Integration with BLTE decompression system

- **40-bit Integer Support** (`tact-parser/src/utils.rs`)
  - `read_uint40()` and `write_uint40()` for little-endian
  - `read_uint40_be()` and `write_uint40_be()` for big-endian
  - TACT encoding format support (1 high byte + 4-byte BE u32)
  - Reader functions for `std::io::Read` traits
  - Comprehensive test coverage with edge cases

- **Progressive Loading Infrastructure** (`casc-storage/src/progressive.rs`)
  - Chunk-based file loading with configurable sizes
  - Predictive prefetching based on access patterns
  - Memory-efficient streaming operations
  - Statistics tracking for cache efficiency

- **Test Infrastructure** (`test-utils` crate)
  - WoW data discovery utility with cross-platform support
  - Environment variable support for test data paths
  - CI-friendly test skipping when data unavailable
  - Serial test execution to prevent race conditions

- **Performance Improvements**
  - BpsvRowOps trait for code deduplication
  - Atomic operations for simple counters
  - Optimized LRU cache implementation

### Fixed

- **TVFS Implementation** (`tact-parser/src/tvfs.rs`)
  - Magic bytes fixed to only accept 'TVFS' (0x53465654)
  - All offset/size fields converted to 40-bit big-endian integers
  - Header structure updated to match specification exactly
  - Big-endian compliance for all multi-byte values

- **Encoding File Parser** (`tact-parser/src/encoding.rs`)
  - Correct 40-bit integer parsing for file sizes
  - TACT encoding format implementation (1 byte + 4-byte BE u32)
  - Validated against real WoW encoding files

- **MSRV Compatibility**
  - Fixed 17 let-chain syntax instances for Rust 1.86
  - Removed all unstable features
  - Ensured CI/CD validates both MSRV and stable

- **Memory Safety**
  - Fixed unsafe string slicing in ngdp-bpsv parser
  - Proper bounds checking in all parsers
  - Eliminated unsafe indexing operations

- **Code Quality Issues**
  - Fixed Clippy warnings (unnecessary casts, map_or simplifications)
  - Resolved deprecated function warnings with proper suppression
  - Fixed documentation URL formatting for rustdoc compliance
  - Fixed race conditions in cache tests

### Changed

- Updated all crates from version 0.3.1 to 0.4.0
- Enhanced documentation with specification compliance details
- Improved error handling and validation
- CDN client architecture simplified from 3 to 2 variants
- casc-storage version aligned with other crates (0.1.0 → 0.4.0)

## Technical Details

### 40-bit Integer Implementation

The TACT format uses a specific encoding for 40-bit integers in encoding files:
- **Header values**: Standard big-endian 40-bit integers
- **File sizes in pages**: 1 high byte (bits 32-39) + 4-byte big-endian u32 (bits 0-31)

This allows representing file sizes up to 1TB while maintaining compatibility with the game client.

### ESpec Grammar Support

Complete implementation of the EBNF grammar:
```
e-spec = ( 'n' ) | ( 'z' [...] ) | ( 'e' [...] ) | ( 'b' [...] ) | ( 'c' [...] ) | ( 'g' [...] )
```

Supports complex specifications like:
- `z`: ZLib compression
- `z:6`: ZLib with level 6
- `b:{164=z,16K*565=z,1656=z,140164=z}`: Block table with mixed compression

### TVFS Specification Compliance

All TVFS fields now use 40-bit big-endian integers:
- `path_table_offset` and `path_table_size`
- `vfs_table_offset` and `vfs_table_size` 
- `cft_table_offset` and `cft_table_size`

This ensures compatibility with modern game builds.

## All Crate Versions

All crates have been updated to version 0.4.2:

| Crate | crates.io |
|-------|-----------| 
| ngdp-bpsv | [![crates.io](https://img.shields.io/crates/v/ngdp-bpsv.svg)](https://crates.io/crates/ngdp-bpsv) |
| ribbit-client | [![crates.io](https://img.shields.io/crates/v/ribbit-client.svg)](https://crates.io/crates/ribbit-client) |
| tact-client | [![crates.io](https://img.shields.io/crates/v/tact-client.svg)](https://crates.io/crates/tact-client) |
| tact-parser | [![crates.io](https://img.shields.io/crates/v/tact-parser.svg)](https://crates.io/crates/tact-parser) |
| ngdp-cdn | [![crates.io](https://img.shields.io/crates/v/ngdp-cdn.svg)](https://crates.io/crates/ngdp-cdn) |
| ngdp-cache | [![crates.io](https://img.shields.io/crates/v/ngdp-cache.svg)](https://crates.io/crates/ngdp-cache) |
| ngdp-crypto | [![crates.io](https://img.shields.io/crates/v/ngdp-crypto.svg)](https://crates.io/crates/ngdp-crypto) |
| blte | [![crates.io](https://img.shields.io/crates/v/blte.svg)](https://crates.io/crates/blte) |
| ngdp-client | [![crates.io](https://img.shields.io/crates/v/ngdp-client.svg)](https://crates.io/crates/ngdp-client) |
| casc-storage | [![crates.io](https://img.shields.io/crates/v/casc-storage.svg)](https://crates.io/crates/casc-storage) |
| test-utils | Internal testing utility (not published) |

## Contributors

Thank you to all contributors who helped make this release possible!

## Support

For issues or questions:

- GitHub Issues: <https://github.com/wowemulation-dev/cascette-rs/issues>
- Documentation: <https://github.com/wowemulation-dev/cascette-rs/tree/main/docs>

## License

This project is dual-licensed under MIT OR Apache-2.0.
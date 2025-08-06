# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/wowemulation-dev/cascette-rs/compare/tact-parser-v0.1.0...tact-parser-v0.2.0) - 2025-08-06

### Added

- complete TACT parser implementation with encryption and BLTE support

### Fixed

- clippy/fmt

### Other

- ✨ feat: integrate tact-parser with build configuration analysis
- 📝 docs: update changelogs and add module documentation
- 🎨 style: optimize code and remove unnecessary allocations
- 🚨 fix: remove panicking Default impls and fix unwrap() calls
- Move tact-parser dependencies into workspace
- fmt
- tidy up old stuff
- Implement support for pre-8.2 roots
- Use bufread to improve performance
- Docs
- fmt
- Move all the tests into separate files like the rest of the packages
- Sort ReadInt implementations
- Initial wow TACT root parser
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Core TACT file parsing support**:
  - Initial release of tact-parser crate
  - Support for parsing WoW root files to find file IDs and MD5s
  - Jenkins3 hash implementation for TACT data processing
  - Support for both modern (8.2+) and legacy pre-8.2 root formats
  - Efficient buffered I/O operations for improved performance
  - Module-level documentation with usage examples

- **Build configuration parsing**:
  - Complete parser for TACT build configuration files (text-based key=value format)
  - Support for hash-size pairs parsing from configuration entries
  - Handles empty values and various configuration formats (key=value, key= for empty)
  - Helper methods for accessing common build properties:
    - `root_hash()`, `encoding_hash()`, `install_hash()`, `download_hash()`, `size_hash()`
    - `build_name()` for human-readable version strings
    - Size information retrieval for files with hash-size pairs
  - Configuration value and hash pair access methods
  - Comprehensive test coverage with real-world configuration data from CDN

- **Encoding file parsing with 40-bit integer support**:
  - Full parser for TACT encoding files (binary format with CKey ↔ EKey mapping)
  - Native support for 40-bit integers used for file sizes (supports up to 1TB files)
  - Bidirectional lookup capabilities:
    - `lookup_by_ckey()` - Content Key → Encoding entries mapping
    - `lookup_by_ekey()` - Encoding Key → Content Key reverse mapping
  - Helper methods for common operations:
    - `get_ekey_for_ckey()` - Get first encoding key for content
    - `get_file_size()` - Get file size for content key
    - `ckey_count()` and `ekey_count()` - Entry counting
  - Big-endian header parsing for encoding file metadata
  - Support for multiple encoding keys per content key (different compression methods)
  - Handles various 40-bit integer values from 0 to 1TB (0xFFFFFFFFFF)

- **Install manifest parsing**:
  - Parser for TACT install manifest files
  - Support for file installation metadata and directory structures
  - Integration with encoding file lookups for complete file resolution
  - Handles install file priorities and file system organization

- **Utility functions**:
  - 40-bit integer reading/writing utilities (little-endian format)
  - `read_uint40()` and `write_uint40()` for precise 5-byte integer handling
  - Configuration file text parsing with proper empty value handling
  - Hash parsing and validation utilities

- **Comprehensive testing**:
  - 49 total tests (24 unit tests, 25 integration tests)
  - Real-world data parsing tests with actual CDN configuration files
  - Edge case testing for empty values, large file sizes, and malformed data
  - Performance benchmarks for Jenkins3 hashing
  - Examples demonstrating all major functionality

### Fixed

- **Documented safety of unwrap() calls**:
  - Added SAFETY comments to Jenkins3 hash implementation
  - Clarified that unwraps on fixed-size array slices are guaranteed safe
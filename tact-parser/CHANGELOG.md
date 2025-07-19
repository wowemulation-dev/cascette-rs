# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial release of tact-parser crate
- Support for parsing WoW root files to find file IDs and MD5s
- Jenkins3 hash implementation for TACT data processing
- Support for both modern (8.2+) and legacy pre-8.2 root formats
- Efficient buffered I/O operations for improved performance
- Comprehensive test suite with unit and integration tests
- Performance benchmarks for Jenkins3 hashing
- Example demonstrating WoW root file parsing
- Module-level documentation with usage examples

### Fixed

- **Documented safety of unwrap() calls**:
  - Added SAFETY comments to Jenkins3 hash implementation
  - Clarified that unwraps on fixed-size array slices are guaranteed safe
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/wowemulation-dev/cascette-rs/compare/blte-v0.3.0...blte-v0.3.1) - 2025-08-07

### Fixed

- resolve clippy uninlined format args warnings

### Other

- update all README files and improve crate descriptions
# Changelog

All notable changes to the `blte` crate will be documented in this file.

## [Unreleased]

### Added
- Streaming decompression support with `BLTEStream` struct
- `create_streaming_reader()` convenience function
- Read trait implementation for memory-efficient processing
- Example demonstrating streaming decompression usage

## [0.1.0] - 2025-08-06

### Added
- Initial implementation of BLTE (Block Table Encoded) decompression
- Support for all compression modes:
  - Mode 'N' (0x4E): No compression
  - Mode 'Z' (0x5A): ZLib compression
  - Mode '4' (0x34): LZ4 compression  
  - Mode 'F' (0x46): Recursive BLTE (frame mode)
  - Mode 'E' (0x45): Encrypted blocks with Salsa20/ARC4
- Multi-chunk file support with proper block indexing
- BLTE header parsing for both single and multi-chunk files
- Chunk checksum verification (MD5)
- Integration with ngdp-crypto for encrypted content
- Comprehensive test coverage for all compression modes
- Memory-efficient chunk processing
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
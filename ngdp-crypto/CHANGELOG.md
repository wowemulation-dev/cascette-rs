# Changelog

All notable changes to the `ngdp-crypto` crate will be documented in this file.

## [0.4.0] - 2025-08-09

### Fixed

- **Code Quality Issues**:
  - Added proper suppression for deprecated ARC4 function warnings in test code
  - Fixed Clippy warnings and improved code documentation

### Changed

- Updated crate version from 0.1.0 to 0.4.0 for consistency with workspace

## [0.1.0] - 2025-08-06

### Added

- Initial implementation of NGDP encryption/decryption support
- Salsa20 stream cipher implementation for modern BLTE encryption
  - Proper key extension (16 bytes → 32 bytes by duplication)
  - IV extension (4 bytes → 8 bytes by duplication)
  - Block index XOR with IV for multi-chunk support
- ARC4 (RC4) cipher implementation for legacy encryption
  - Combined key+IV+block_index initialization
  - 32-byte padded key handling
- KeyService for managing TACT encryption keys
  - Automatic loading from standard directories (~/.config/cascette/, ~/.tactkeys/)
  - Support for multiple key file formats (CSV, TXT, TSV)
  - Environment variable support (CASCETTE_KEYS_PATH)
  - Built-in hardcoded keys for common WoW content
  - Successfully loads 19,419+ WoW encryption keys
- Comprehensive error handling with thiserror
- Full test coverage with round-trip encryption/decryption tests

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2025-08-06

### Added

- **Encryption key management commands**:
  - Added `ngdp keys` command group for managing TACT encryption keys
  - `ngdp keys update` - Downloads latest keys from TACTKeys repository
  - `ngdp keys status` - Shows local key database information
  - Automatic key file creation in `~/.config/cascette/WoW.txt`
  - Support for forced updates with `--force` flag
  - Custom output path support with `--output`

- **Enhanced inspect commands with BLTE decompression**:
  - `inspect encoding` - Inspect encoding files with checksum validation
  - `inspect install` - Parse install manifests with tag filtering
  - `inspect download-manifest` - Analyze download priorities
  - `inspect size` - Calculate installation sizes per platform/tag
  - All commands now handle BLTE-encoded CDN files automatically
  - Support for all output formats (text, JSON, BPSV)

- **Download files command structure**:
  - Added placeholder for `download files` command
  - Supports downloading by content key or encoding key
  - Includes automatic BLTE decompression
  - Full implementation pending API adjustments

### Fixed

- **Inspect command improvements**:
  - Fixed encoding file checksum validation
  - Corrected manifest decompression with proper BLTE handling
  - Added proper error handling for encrypted content
  - Improved memory efficiency for large files

### Dependencies

- Added `ngdp-crypto` (0.1.0) for encryption support
- Added `blte` (0.1.0) for BLTE decompression

## [0.2.1] - 2025-08-05

### Added

- **TACT parser integration for build configuration analysis**:
  - Added `inspect build-config` command for detailed build configuration analysis
  - Downloads and parses real build configurations from CDN using tact-parser crate
  - Visual tree representation of game build structure with emoji and Unicode box-drawing
  - Shows core game files (root, encoding, install, download, size) with file sizes
  - Displays build information (version, UID, product, installer)
  - Patch status indication with hash display
  - VFS (Virtual File System) entries listing with file counts
  - Support for all output formats: text (visual tree), JSON, and raw BPSV
  - Example: `ngdp inspect build-config wow_classic_era 61582 --region us`

- **Enhanced products versions command with build configuration parsing**:
  - Added `--parse-config` flag to `products versions` command
  - Downloads and parses build configurations to show meaningful information
  - Displays build names instead of just cryptic hashes (e.g., "WOW-62417patch11.2.0_Retail")
  - Shows patch availability and file size information  
  - Counts VFS entries to indicate build complexity
  - Maintains full backward compatibility when flag is not used
  - Works across all WoW products (wow, wow_classic_era, wowt, etc.)
  - Example: `ngdp products versions wow --parse-config`

- **Dependencies**:
  - Added `tact-parser` (0.1.0) dependency for TACT file format parsing
  - Added `ngdp-cdn` client integration for downloading build configurations

### Fixed

- **Code optimization**:
  - Optimized string building in certificate PEM to DER conversion using iterator chains
  - More efficient and idiomatic implementation of base64 extraction

## [0.2.0](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-client-v0.1.2...ngdp-client-v0.2.0) - 2025-07-15

### Other

- 🔧 fix: resolve clippy warnings and apply code formatting
- 📝 docs: synchronize individual crate changelogs with main changelog
- ✨ feat(ngdp-client): enhance config show to display all settings
- 🩹 fix(ngdp-client): resolve critical -o flag conflict in download commands
- 🩹 fix(ngdp-client): resolve conflicting short command-line flags
- ✨ feat(ngdp-client): add products builds command with Wago Tools API integration

### Added

- **Historical builds command**:
  - Added `ngdp products builds` command to retrieve all historical builds for a product
  - Integrates with Wago Tools API (https://wago.tools/api/builds) for comprehensive build history
  - Support for filtering by version pattern with `--filter`
  - Time-based filtering with `--days` option
  - Result limiting with `--limit` option
  - Background download builds filtering with `--bgdl-only`
  - Displays build version, creation date, build config, and type (Full/BGDL)
  - Support for JSON, BPSV, and formatted text output
  - Caching support with 30-minute TTL to reduce API load
  - Respects global cache settings (`--no-cache` and `--clear-cache` flags)

- **Custom CDN fallback configuration**:
  - New `custom_cdn_fallbacks` configuration option for user-defined CDN hosts
  - Custom CDNs are tried after Blizzard and community CDNs have been exhausted
  - Integration with `CdnClientWithFallback` through new `cdn_config` module
  - Custom CDNs can be configured as comma-separated list in settings

### Fixed

- **Conflicting short command-line flags**:
  - Removed `-l` short flag from `--limit` in `products builds` command (was conflicting with `-l` for `--log-level`)
  - Removed `-d` short flag from `--days` in `products builds` command (was conflicting with `-d` for `--details` in `certs download`)
  - Removed `-o` short flag from `--output` in `download build` and `download files` commands (was conflicting with global `-o` for `--format`)

- **Enhanced `config show` command**:
  - Now shows all available settings with their default values, not just the three basic ones
  - Added settings: `cache_enabled`, `cache_ttl`, `max_concurrent_downloads`, `user_agent`, `verify_certificates`, `proxy_url`, `ribbit_timeout`, `tact_timeout`, `retry_attempts`, `log_file`, `color_output`, `fallback_to_tact`, `use_community_cdn_fallbacks`, `custom_cdn_fallbacks`
  - All settings are now accessible via `config get` command

- **Code quality improvements**:
  - Fixed clippy warnings in examples (uninlined_format_args)
  - Applied consistent code formatting

## [0.1.2](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-client-v0.1.1...ngdp-client-v0.1.2) - 2025-07-05

### Other

- updated the following local packages: ngdp-cache

## [0.1.1](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-client-v0.1.0...ngdp-client-v0.1.1) - 2025-06-29

### Other

- 🔧 chore: replace OpenSSL with rustls for cross-platform builds

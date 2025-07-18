# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-client-v0.2.0...ngdp-client-v0.2.1) - 2025-07-18

### Other

- updated the following local packages: ribbit-client, ngdp-cache

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

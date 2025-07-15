# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Enhanced CDN fallback support**:
  - Support for custom CDN fallbacks via `add_custom_cdn()` and `set_custom_cdns()` methods
  - Custom CDNs are tried after primary and community CDNs in fallback order
  - New `custom_cdn_hosts` field in `CdnClientWithFallback` for user-defined hosts
  - Builder support for custom CDNs with `add_custom_cdn()` and `add_custom_cdns()` methods
  - `clear_cdns()` now clears custom CDN hosts as well
  - Full support for custom CDN configuration in builder pattern

## [0.2.0](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-cdn-v0.1.1...ngdp-cdn-v0.2.0) - 2025-07-05

### Other

- 🔧 chore: fix clippy warning and remove dependabot config
- ✨ feat(ngdp-cdn): add automatic CDN fallback support with community mirrors

## [0.1.1](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-cdn-v0.1.0...ngdp-cdn-v0.1.1) - 2025-06-29

### Other

- 🔧 chore: replace OpenSSL with rustls for cross-platform builds
- 🐛 fix: all the cargo checks we can find

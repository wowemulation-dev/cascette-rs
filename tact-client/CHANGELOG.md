# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3](https://github.com/wowemulation-dev/cascette-rs/compare/tact-client-v0.1.2...tact-client-v0.1.3) - 2025-08-06

### Other

- ✨ feat: integrate tact-parser with build configuration analysis
- 📝 docs: update changelogs and add module documentation
- 🚨 fix: remove panicking Default impls and fix unwrap() calls

### Fixed

- **Replaced unwrap() calls with proper error handling**:
  - All response parsing now uses `ok_or_else()` with meaningful error messages
  - Added proper error propagation for missing fields in BPSV responses
  - Prevents potential panics when parsing malformed responses

## [0.1.2](https://github.com/wowemulation-dev/cascette-rs/compare/tact-client-v0.1.1...tact-client-v0.1.2) - 2025-07-15

### Other

- updated the following local packages: ngdp-bpsv

## [0.1.1](https://github.com/wowemulation-dev/cascette-rs/compare/tact-client-v0.1.0...tact-client-v0.1.1) - 2025-06-29

### Other

- 🔧 chore: replace OpenSSL with rustls for cross-platform builds

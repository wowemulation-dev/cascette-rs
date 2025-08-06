# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-cache-v0.1.3...ngdp-cache-v0.1.4) - 2025-08-06

### Fixed

- path tests on non-linux platforms

### Other

- ✨ feat: integrate tact-parser with build configuration analysis
- 📝 docs: update changelogs and add module documentation
- Merge pull request #11 from micolous/refactor/bpsv-response
- Merge pull request #10 from micolous/fix/non-linux-paths

### Fixed

- Fixed path tests on non-Linux platforms:
  - Updated cache path tests to work correctly across different operating systems
  - Fixed hardcoded Unix path separators that caused test failures on Windows

## [0.1.3](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-cache-v0.1.2...ngdp-cache-v0.1.3) - 2025-07-15

### Other

- updated the following local packages: ngdp-cdn, ribbit-client, tact-client

## [0.1.2](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-cache-v0.1.1...ngdp-cache-v0.1.2) - 2025-07-05

### Other

- updated the following local packages: ngdp-cdn

## [0.1.1](https://github.com/wowemulation-dev/cascette-rs/compare/ngdp-cache-v0.1.0...ngdp-cache-v0.1.1) - 2025-06-29

### Other

- 🔧 chore: replace OpenSSL with rustls for cross-platform builds

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/wowemulation-dev/cascette-rs/releases/tag/ngdp-bpsv-v0.1.0) - 2025-06-28

### Added

- major caching and testing improvements
- add custom user agent support and fix cache directory structure
- add ngdp-cache crate for generic NGDP caching functionality
- add ngdp-bpsv crate for BPSV parsing and writing

### Fixed

- *(ngdp-bpsv)* correct HEX field lengths in benchmarks
- clippy warnings
- *(ngdp-bpsv)* correct HEX field length interpretation to match Blizzard's semantics
- resolve all clippy warnings and improve code quality

### Other

- 📝 docs: prepare for v0.1.0 release with comprehensive documentation updates
- ⚡️ perf: implement comprehensive performance optimizations across all crates
- clean up license sections
- update CONTRIBUTING.md and CONTRIBUTORS.md for cascette-rs

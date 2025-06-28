# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/wowemulation-dev/cascette-rs/releases/tag/ngdp-cache-v0.1.0) - 2025-06-28

### Added

- major caching and testing improvements
- [**breaking**] change default protocol to V2 for better performance
- *(ngdp-cache)* add CachedTactClient for TACT protocol metadata caching
- add custom user agent support and fix cache directory structure
- *(ngdp-cache)* make CachedRibbitClient a complete drop-in replacement for RibbitClient
- *(ngdp-cache)* add CachedRibbitClient for transparent request caching
- add ngdp-cache crate for generic NGDP caching functionality

### Fixed

- clippy warnings
- *(ngdp-bpsv)* correct HEX field length interpretation to match Blizzard's semantics
- resolve code quality issues and update CHANGELOG
- resolve example filename collisions and missing exports
- MIME parsing and sequence number extraction in CachedRibbitClient

### Other

- 🐛 fix: all the cargo checks we can find
- 📝 docs: prepare for v0.1.0 release with comprehensive documentation updates
- ⚡️ perf: implement comprehensive performance optimizations across all crates
- clean up license sections
- update CONTRIBUTING.md and CONTRIBUTORS.md for cascette-rs
- *(ngdp-cache)* add comprehensive tests and benchmarks for CachedRibbitClient

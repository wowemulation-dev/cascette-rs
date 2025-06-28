# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/wowemulation-dev/cascette-rs/releases/tag/ngdp-client-v0.1.0) - 2025-06-28

### Added

- major caching and testing improvements
- [**breaking**] change default protocol to V2 for better performance
- *(ngdp-client)* add automatic fallback and improve region filtering
- *(ngdp-client)* add certs download subcommand
- *(ngdp-cache)* add CachedRibbitClient for transparent request caching
- enhance ngdp-client terminal output with tables and colors
- add ngdp-client CLI application

### Fixed

- clippy warnings
- resolve all clippy warnings and improve code quality
- *(ribbit-client)* parse CDN servers field as Vec<String> for consistency
- *(ngdp-client)* fix JSON output for certs download command
- MIME parsing and sequence number extraction in CachedRibbitClient

### Other

- 🐛 fix: all the cargo checks we can find
- 📝 docs: prepare for v0.1.0 release with comprehensive documentation updates
- ⚡️ perf: implement comprehensive performance optimizations across all crates
- clean up license sections
- update CONTRIBUTING.md and CONTRIBUTORS.md for cascette-rs

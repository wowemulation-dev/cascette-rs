# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/wowemulation-dev/cascette-rs/releases/tag/tact-client-v0.1.0) - 2025-06-28

### Added

- major caching and testing improvements
- [**breaking**] change default protocol to V2 for better performance
- add custom user agent support and fix cache directory structure
- *(tact-client)* add automatic retry support with exponential backoff
- *(tact-client)* add performance benchmarks for response parsing
- *(tact-client)* [**breaking**] implement HTTP client for TACT protocol
- initial ribbit-client support is here

### Fixed

- resolve all clippy warnings and improve code quality
- resolve code quality issues and update CHANGELOG
- resolve example filename collisions and missing exports
- *(ribbit-client)* parse CDN servers field as Vec<String> for consistency

### Other

- 📝 docs: prepare for v0.1.0 release with comprehensive documentation updates
- ⚡️ perf: implement comprehensive performance optimizations across all crates
- clean up license sections
- update CONTRIBUTING.md and CONTRIBUTORS.md for cascette-rs
- moved tokio to workspace level
- bootstrap the project

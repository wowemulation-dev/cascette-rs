# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/wowemulation-dev/cascette-rs/releases/tag/ribbit-client-v0.1.0) - 2025-06-28

### Added

- major caching and testing improvements
- [**breaking**] change default protocol to V2 for better performance
- *(tact-client)* add automatic retry support with exponential backoff
- *(tact-client)* [**breaking**] implement HTTP client for TACT protocol
- enhance ngdp-client terminal output with tables and colors
- add ngdp-client CLI application
- add ngdp-bpsv crate for BPSV parsing and writing
- initial ribbit-client support is here

### Fixed

- clippy warnings
- *(ngdp-bpsv)* correct HEX field length interpretation to match Blizzard's semantics
- resolve all clippy warnings and improve code quality
- resolve code quality issues and update CHANGELOG
- resolve example filename collisions and missing exports
- *(ribbit-client)* resolve unused variable warning in debug_cert_checksum example
- *(ribbit-client)* parse CDN servers field as Vec<String> for consistency
- resolve linter warnings and improve code quality

### Other

- 🐛 fix: all the cargo checks we can find
- 📝 docs: prepare for v0.1.0 release with comprehensive documentation updates
- ⚡️ perf: implement comprehensive performance optimizations across all crates
- clean up license sections
- added test-case to examine a bug on Blizzards side
- moved tokio to workspace level
- cleanup friendly clippy reminders
- bootstrap the project

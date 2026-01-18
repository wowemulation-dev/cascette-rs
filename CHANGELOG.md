# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

<!-- Changes pending the 1.0.0 release go here -->

### Added

- Rust 2024 workspace with MSRV 1.86.0
- cascette-crypto crate: MD5, Jenkins96, Salsa20, ARC4 implementations
- TACT key management with TactKeyProvider trait for custom backends
- Workspace-level clippy lints for code quality
- Documentation framework using mdBook with Mermaid diagram support
- CI workflow with quality checks (fmt, clippy, test, doc, WASM)
- WASM compilation support for cascette-crypto (wasm32-unknown-unknown)
- Project introduction explaining wowemulation-dev goals and modern client focus
- Glossary of NGDP/CASC terminology with MPQ equivalents for newcomers
- Format documentation: encoding, root, install, download, archives, archive
  groups, TVFS, config formats, patches, BPSV, format transitions
- Compression documentation: BLTE container format, ESpec encoding specs
- Encryption documentation: Salsa20 stream cipher
- Protocol documentation: CDN architecture, Ribbit protocol
- Client documentation: Battle.net Agent, local CASC storage
- Operations documentation: CDN mirroring, reference implementations
- Community CDN mirrors list (Arctium, Wago, wow.tools)

### Changed

- Updated dependencies: tempfile 3.21→3.24, proptest 1.7→1.9, criterion 0.7→0.8
- Removed keyring and file-store features from cascette-crypto for WASM compatibility
- Key loading functions now accept string content instead of file paths

### Fixed

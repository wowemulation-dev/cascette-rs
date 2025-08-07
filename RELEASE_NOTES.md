# Release Notes - v0.3.1

## Release Summary

cascette-rs v0.3.1 is a patch release that includes important fixes for code quality, documentation improvements, and GitHub Actions workflow corrections. This release ensures all crates can be successfully published to crates.io and maintains high code quality standards.

## Key Highlights

### Bug Fixes
- **Resolved clippy warnings**: Fixed all uninlined format arguments across multiple files
- **Fixed release workflow**: Added missing crates to GitHub Actions publishing pipeline
- **Corrected publishing order**: Ensures proper dependency resolution during crate publication

### Documentation Improvements
- **TACT acronym correction**: Fixed to "Trusted Application Content Transfer"
- **Enhanced crate descriptions**: Improved discoverability on crates.io
- **Updated README files**: All crates now have proper installation instructions

### Developer Experience
- **QA command documentation**: Created comprehensive rust-qa.md for local CI checks
- **Workflow stability**: Implemented long-term fixes for CI/CD pipelines
- **Code quality**: Enforces modern Rust idioms and best practices

## Breaking Changes

None. This release maintains backward compatibility with all previous versions.

## Migration Guide

No migration required. Simply update your dependencies to version 0.3.1:

```toml
[dependencies]
ngdp-bpsv = "0.3.1"
ribbit-client = "0.3.1"
tact-client = "0.3.1"
tact-parser = "0.3.1"
ngdp-cdn = "0.3.1"
ngdp-cache = "0.3.1"
ngdp-crypto = "0.3.1"
blte = "0.3.1"
```

## Installation

### Using the install script (Linux/macOS/Windows)

```bash
curl -fsSL https://raw.githubusercontent.com/wowemulation-dev/cascette-rs/main/install.sh | bash
```

### Using cargo-binstall

```bash
cargo binstall ngdp-client
```

### Using cargo

```bash
cargo install ngdp-client
```

### From Source

```bash
git clone https://github.com/wowemulation-dev/cascette-rs
cd cascette-rs
cargo build --release
```

## Changes in This Release

### Fixed
- Resolved all clippy uninlined format arguments warnings
- Fixed missing crates in GitHub Actions release workflow
- Corrected TACT acronym to "Trusted Application Content Transfer"
- Added missing crate descriptions for crates.io publishing
- Fixed crate publishing order to respect dependencies

### Changed
- Updated all crates from version 0.3.0 to 0.3.1
- Improved crate descriptions for better discoverability
- Enhanced README files with installation instructions

### Added
- Comprehensive rust-qa.md command documentation
- QA checks matching GitHub Actions CI pipeline

## All Crate Versions

All crates have been updated to version 0.3.1:

| Crate | crates.io |
|-------|-----------|
| ngdp-bpsv | [![crates.io](https://img.shields.io/crates/v/ngdp-bpsv.svg)](https://crates.io/crates/ngdp-bpsv) |
| ribbit-client | [![crates.io](https://img.shields.io/crates/v/ribbit-client.svg)](https://crates.io/crates/ribbit-client) |
| tact-client | [![crates.io](https://img.shields.io/crates/v/tact-client.svg)](https://crates.io/crates/tact-client) |
| tact-parser | [![crates.io](https://img.shields.io/crates/v/tact-parser.svg)](https://crates.io/crates/tact-parser) |
| ngdp-cdn | [![crates.io](https://img.shields.io/crates/v/ngdp-cdn.svg)](https://crates.io/crates/ngdp-cdn) |
| ngdp-cache | [![crates.io](https://img.shields.io/crates/v/ngdp-cache.svg)](https://crates.io/crates/ngdp-cache) |
| ngdp-crypto | [![crates.io](https://img.shields.io/crates/v/ngdp-crypto.svg)](https://crates.io/crates/ngdp-crypto) |
| blte | [![crates.io](https://img.shields.io/crates/v/blte.svg)](https://crates.io/crates/blte) |
| ngdp-client | [![crates.io](https://img.shields.io/crates/v/ngdp-client.svg)](https://crates.io/crates/ngdp-client) |

## Contributors

Thank you to all contributors who helped make this release possible!

## Support

For issues or questions:
- GitHub Issues: https://github.com/wowemulation-dev/cascette-rs/issues
- Documentation: https://github.com/wowemulation-dev/cascette-rs/tree/main/docs

## License

This project is dual-licensed under MIT OR Apache-2.0.
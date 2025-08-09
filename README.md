# cascette-rs

Open-source Rust implementation of Blizzard's NGDP (Next Generation Distribution
Pipeline) for World of Warcraft emulation.

<div align="center">

[![Discord](https://img.shields.io/discord/1394228766414471219?logo=discord&style=flat-square)](https://discord.gg/QbXn7Vqb)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![CI Status](https://github.com/wowemulation-dev/cascette-rs/workflows/CI/badge.svg)](https://github.com/wowemulation-dev/cascette-rs/actions)
[![Crates.io Version](https://img.shields.io/crates/v/ngdp-client)](https://crates.io/crates/ngdp-client)
[![docs.rs](https://img.shields.io/docsrs/ngdp-client)](https://docs.rs/ngdp-client)

</div>

## 🎯 Project Status

**Current Version**: 0.3.1

### Core Components

| Component       | Version | Status      | Description                                        |
| --------------- | ------- | ----------- | -------------------------------------------------- |
| `ngdp-bpsv`     | 0.3.1   | ✅ Stable   | BPSV parser/writer for NGDP formats                |
| `ribbit-client` | 0.3.1   | ✅ Stable   | Ribbit protocol client with signature verification (legacy fallback) |
| `tact-client`   | 0.3.1   | ✅ Stable   | TACT HTTP client with modern HTTPS endpoints primary |
| `tact-parser`   | 0.3.1   | ✅ Stable   | TACT file format parser (encoding, install, etc.) |
| `ngdp-cdn`      | 0.3.1   | ✅ Stable   | CDN content delivery with parallel downloads       |
| `ngdp-cache`    | 0.3.1   | ✅ Stable   | Hybrid caching with HTTP-first version discovery  |
| `blte`          | 0.3.1   | ✅ Stable   | BLTE decompression (ARC4/Frame modes deprecated)  |
| `ngdp-crypto`   | 0.3.1   | ✅ Stable   | Modern encryption with Salsa20 (ARC4 deprecated)  |
| `ngdp-client`   | 0.3.1   | ✅ Stable   | CLI tool for NGDP operations                      |

### Implementation Progress

- ✅ **Modern Version Discovery**: HTTP-first approach with HTTPS endpoints, Ribbit fallback for compatibility
- ✅ **TACT Protocol**: Full HTTP/HTTPS clients for version and CDN queries with retry logic
- ✅ **BPSV Format**: Complete parser and builder with zero-copy optimizations
- ✅ **TACT Parsers**: Full support for encoding, install, download, size, build config, TVFS
- ✅ **BLTE Decompression**: Modern compression modes (ARC4/Frame deprecated in v0.4.0)
- ✅ **Encryption**: Modern Salsa20 cipher with key management (ARC4 deprecated)
- ✅ **CDN Operations**: Parallel downloads, streaming, retry logic, rate limiting
- ✅ **Hybrid Caching**: HTTP-first caching with transparent fallbacks and TTL support
- ✅ **Install Command**: Full client installation with .build.info generation for restoration
- ✅ **Build Config**: Proper uncompressed handling and download order (encoding before manifests)
- 🚧 **CASC Storage**: Local storage implementation (planned for future release)
- 🔄 **TVFS**: Basic parser implemented, needs real-world data testing

## 🚀 Quick Start

### Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
ribbit-client = "0.3"
ngdp-bpsv = "0.3"
tact-parser = "0.3"
blte = "0.3"
ngdp-crypto = "0.3"
```

Basic example (modern HTTP-first approach):

```rust
use ngdp_cache::hybrid_version_client::HybridVersionClient;
use ribbit_client::Region;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a hybrid client (HTTP primary, Ribbit fallback)
    let client = HybridVersionClient::new(Region::US).await?;

    // Request WoW versions with modern HTTPS endpoints
    let versions = client.get_product_versions("wow").await?;

    // Print version information
    for entry in &versions.entries {
        println!(
            "{}: {} (build {})",
            entry.region, entry.versions_name, entry.build_id
        );
    }

    Ok(())
}
```

## 📦 Installation

### CLI Tool

#### Install with Cargo
```bash
cargo install ngdp-client
```

#### Install with Script (Unix/Linux/macOS)
```bash
curl -fsSL https://raw.githubusercontent.com/wowemulation-dev/cascette-rs/main/install.sh | bash
```

#### Install with Script (Windows PowerShell)
```powershell
irm https://raw.githubusercontent.com/wowemulation-dev/cascette-rs/main/install.ps1 | iex
```

### Library Usage

```bash
cargo add ribbit-client ngdp-bpsv tact-client tact-parser ngdp-cdn ngdp-cache blte ngdp-crypto
```

### From source

```bash
git clone https://github.com/wowemulation-dev/cascette-rs
cd cascette-rs
cargo build --release
# CLI binary will be at target/release/ngdp
```

## 🖥️ CLI Usage

The `ngdp-client` provides a comprehensive command-line interface for NGDP operations:

### Install Game Clients

```bash
# Install WoW Classic Era (minimal installation)
ngdp install game wow_classic_era --install-type minimal --path ./wow-client

# Full installation with verification
ngdp install game wow --install-type full --verify --path ./wow-retail

# Dry-run to see what would be installed
ngdp install game wow_classic --install-type minimal --dry-run --path ./test
```

### Version and CDN Information

```bash
# Query product versions (uses modern HTTPS endpoints)
ngdp query versions wow

# Get CDN configuration
ngdp query cdns wow

# Inspect build configurations
ngdp inspect build-config <hash>
```

### BPSV and File Analysis

```bash
# Parse and display BPSV files
ngdp inspect bpsv ./file.bpsv

# Analyze BLTE files
ngdp inspect blte ./encrypted.blte
```

All commands support multiple output formats (`--format json|text|bpsv`) and the install command automatically creates `.build.info` files for client restoration.

## 📚 Documentation

- [BPSV Format Specification](docs/bpsv-format.md)
- [BPSV Examples](ngdp-bpsv/examples)
- [Ribbit Protocol](docs/ribbit-protocol.md)
- [Ribbit Examples](ribbit-client/examples)
- [TACT Protocol](docs/tact-protocol.md)

## 📚 Online References

- [TACT Reference](https://wowdev.wiki/TACT)
- [Ribbit Reference](https://wowdev.wiki/Ribbit)
- [CASC Reference](https://wowdev.wiki/CASC)

## 🔧 Features

### Complete

- **BPSV Parser/Writer** (`ngdp-bpsv`)
  - ✅ Complete BPSV format support with zero-copy parsing
  - ✅ Type-safe field definitions (STRING, HEX, DEC)
  - ✅ Schema validation and sequence number handling
  - ✅ Builder pattern for document creation
  - ✅ Round-trip compatibility

- **Ribbit Protocol Client** (`ribbit-client`)
  - ✅ All Blizzard regions (US, EU, CN, KR, TW, SG)
  - ✅ V1 (MIME) and V2 (raw) protocol support
  - ✅ Typed API for all endpoints
  - ✅ PKCS#7/CMS signature verification
  - ✅ Certificate and OCSP support
  - ✅ Automatic retry with exponential backoff
  - ✅ DNS caching for performance

- **TACT HTTP Client** (`tact-client`)
  - ✅ Version and CDN configuration queries
  - ✅ Support for V1 (port 1119) and V2 (HTTPS) protocols
  - ✅ Typed response parsing
  - ✅ Automatic retry handling
  - ✅ All Blizzard regions supported

- **CDN Content Delivery** (`ngdp-cdn`)
  - ✅ Parallel downloads with progress tracking
  - ✅ Streaming operations for large files
  - ✅ Automatic retry with rate limit handling
  - ✅ Content verification
  - ✅ Configurable connection pooling
  - ✅ Automatic fallback to backup CDN servers
  - ✅ Built-in support for community mirrors (arctium.tools, reliquaryhq.com)

- **Caching Layer** (`ngdp-cache`)
  - ✅ Transparent caching for all NGDP operations
  - ✅ TTL-based expiration policies
  - ✅ Streaming I/O for memory efficiency
  - ✅ CDN-compatible directory structure
  - ✅ Batch operations for performance

- **TACT File Parsers** (`tact-parser`)
  - ✅ Encoding files (CKey ↔ EKey mapping)
  - ✅ Install manifests with tag-based filtering
  - ✅ Download manifests with priority sorting
  - ✅ Size files for installation calculations
  - ✅ Build configurations (key-value format)
  - ✅ TVFS (TACT Virtual File System)
  - ✅ 40-bit integer and varint support

- **BLTE Decompression** (`blte`)
  - ✅ All compression modes (None, ZLib, LZ4, Frame, Encrypted)
  - ✅ Multi-chunk file support
  - ✅ Checksum verification
  - ✅ Integration with ngdp-crypto for encrypted blocks
  - ✅ Memory-efficient processing

- **Encryption Support** (`ngdp-crypto`)
  - ✅ Salsa20 stream cipher (modern WoW encryption)
  - ✅ ARC4/RC4 cipher (legacy content)
  - ✅ Key management and automatic loading
  - ✅ Multiple key file formats (CSV, TXT, TSV)
  - ✅ TACTKeys repository integration

- **CLI Tool** (`ngdp-client`)
  - ✅ Product queries and version information
  - ✅ Certificate operations
  - ✅ BPSV inspection and build config analysis
  - ✅ Encryption key management commands
  - ✅ Enhanced inspect commands with BLTE support
  - ✅ Multiple output formats (text, JSON, BPSV)
  - ✅ Beautiful terminal formatting

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Special thanks to the WoW emulation community and the documentation efforts at
[wowdev.wiki](https://wowdev.wiki).

## 📄 License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

---

**Note**: This project is not affiliated with or endorsed by Blizzard Entertainment.
It is an independent implementation based on reverse engineering efforts by the
community for educational and preservation purposes.

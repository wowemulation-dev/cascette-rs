# cascette-rs

Open-source Rust implementation of Blizzard's NGDP (Next Generation Distribution
Pipeline) for World of Warcraft emulation.

<div align="center">

[![Discord](https://img.shields.io/discord/1394228766414471219?logo=discord&style=flat-square)](https://discord.gg/QbXn7Vqb)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![CI Status](https://github.com/wowemulation-dev/cascette-rs/workflows/CI/badge.svg)](https://github.com/wowemulation-dev/cascette-rs/actions)
[![Crates.io Version](https://img.shields.io/crates/v/cascette)](https://crates.io/crates/cascette)
[![docs.rs](https://img.shields.io/docsrs/cascette)](https://docs.rs/cascette)

</div>

## 🎯 Project Status

**Current Version**: 0.1.0 (Ready for Release)

### Core Components

| Component       | Version | Status      | Description                                        |
| --------------- | ------- | ----------- | -------------------------------------------------- |
| `ngdp-bpsv`     | 0.1.0   | ✅ Stable   | BPSV parser/writer for NGDP formats                |
| `ribbit-client` | 0.1.0   | ✅ Stable   | Ribbit protocol client with signature verification |
| `tact-client`   | 0.1.0   | ✅ Stable   | TACT HTTP client for version/CDN queries          |
| `ngdp-cdn`      | 0.1.0   | ✅ Stable   | CDN content delivery with parallel downloads       |
| `ngdp-cache`    | 0.1.0   | ✅ Stable   | Caching layer for NGDP operations                 |
| `ngdp-client`   | 0.1.0   | ✅ Stable   | CLI tool for NGDP operations                      |

### Implementation Progress

- ✅ **Ribbit Protocol**: Full implementation including V1/V2, signature verification, all endpoints
- ✅ **TACT Protocol**: HTTP/HTTPS clients for version and CDN queries
- ✅ **BPSV Format**: Complete parser and builder with zero-copy optimizations
- ✅ **CDN Operations**: Parallel downloads, streaming, retry logic, rate limiting
- ✅ **Caching**: Transparent caching for all protocols with TTL support
- ✅ **CLI Tool**: Feature-complete command-line interface
- 🚧 **CASC Storage**: Local storage implementation (planned for v0.2.0)
- 🚧 **TVFS**: TACT Virtual File System (planned for v0.2.0)

## 🚀 Quick Start

### Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
ribbit-client = "0.1"
ngdp-bpsv = "0.1"
```

Basic example:

```rust
use ribbit_client::{Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client for US region
    let client = RibbitClient::new(Region::US);

    // Request WoW versions with typed API
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

```bash
cargo install ngdp-client
```

### Library Usage

```bash
cargo add ribbit-client ngdp-bpsv tact-client ngdp-cdn ngdp-cache
```

### From source

```bash
git clone https://github.com/wowemulation-dev/cascette-rs
cd cascette-rs
cargo build --release
# CLI binary will be at target/release/ngdp
```

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

- **CLI Tool** (`ngdp-client`)
  - ✅ Product queries and version information
  - ✅ Certificate operations
  - ✅ BPSV inspection
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

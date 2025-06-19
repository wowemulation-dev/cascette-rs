# cascette-rs

Open-source Rust implementation of Blizzard's NGDP (Next Generation Data Pipeline)
for World of Warcraft emulation.

<div align="center">

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![CI Status](https://github.com/wowemulation-dev/cascette-rs/workflows/CI/badge.svg)](https://github.com/wowemulation-dev/cascette-rs/actions)
[![Crates.io Version](https://img.shields.io/crates/v/cascette)](https://crates.io/crates/cascette)
[![docs.rs](https://img.shields.io/docsrs/cascette)](https://docs.rs/cascette)

</div>

## 🎯 Project Status

| Component       | Status      | Description                         |
| --------------- | ----------- | ----------------------------------- |
| `ngdp-bpsv`     | Ready       | BPSV parser/writer for NGDP formats |
| `ribbit-client` | Ready       | Ribbit protocol client              |
| `tact-client`   | Planned     | TACT content transfer protocol      |

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
use ribbit_client::{RibbitClient, Region, Endpoint};
use ngdp_bpsv::BpsvDocument;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client for US region
    let client = RibbitClient::new(Region::US);

    // Get WoW version information
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    let response = client.request(&endpoint).await?;

    // Parse the BPSV data
    if let Some(data) = response.data {
        let doc = BpsvDocument::parse(&data)?;
        println!("Found {} versions", doc.rows().len());

        // Access specific fields
        for row in doc.rows() {
            let region = row.get_raw_by_name("Region", doc.schema()).unwrap_or("");
            let build_id = row.get_raw_by_name("BuildId", doc.schema()).unwrap_or("");
            println!("{}: {}", region, build_id);
        }
    }

    Ok(())
}
```

## 📦 Installation

### From crates.io

```bash
cargo add ribbit-client ngdp-bpsv
```

### From source

```bash
git clone https://github.com/wowemulation-dev/cascette-rs
cd cascette-rs
cargo build --release
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

### Current

- **BPSV Parser/Writer**
  - ✅ Complete BPSV format support
  - ✅ Type-safe field definitions (STRING, HEX, DEC)
  - ✅ Schema validation
  - ✅ Sequence number handling
  - ✅ Builder pattern for document creation
  - ✅ Round-trip compatibility
  - ✅ Empty value support

- **Ribbit Protocol Client**
  - ✅ All Blizzard regions (US, EU, CN, KR, TW, SG)
  - ✅ V1 (MIME) and V2 (PSV) protocol support
  - ✅ Product version queries
  - ✅ CDN configuration retrieval
  - ✅ Certificate and OCSP endpoints
  - ✅ SHA-256 checksum validation
  - ✅ PKCS#7/CMS signature parsing
  - ✅ Async/await with Tokio

### Planned

- **TACT Implementation**
  - Content manifest parsing
  - Encoding tables
  - Download manifests
  - Install manifests
  - Patch manifests

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

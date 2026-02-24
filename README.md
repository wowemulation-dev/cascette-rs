# cascette-rs

Rust implementation of Blizzard's NGDP content distribution system.

<div align="center">

[![Discord](https://img.shields.io/discord/1394228766414471219?logo=discord&style=flat-square)](https://discord.gg/Jj4uWy3DGP)
[![Sponsor](https://img.shields.io/github/sponsors/danielsreichenbach?logo=github&style=flat-square)](https://github.com/sponsors/danielsreichenbach)
[![CI Status](https://github.com/wowemulation-dev/cascette-rs/workflows/CI/badge.svg)](https://github.com/wowemulation-dev/cascette-rs/actions)
[![WASM](https://img.shields.io/badge/WASM-compatible-blueviolet.svg)](https://webassembly.org/)
[![Rust Version](https://img.shields.io/badge/rust-1.92+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

</div>

## Overview

NGDP (Next Generation Distribution Pipeline) is the system Blizzard uses to
distribute and update all modern games including World of Warcraft, Diablo IV,
and Overwatch. It consists of:

- **TACT** - Content delivery via CDN (downloading game files)
- **CASC** - Local storage format (how files are stored on disk)

Files are identified by content hash rather than filename, enabling efficient
patching, verification, and deduplication.

cascette-rs focuses on World of Warcraft and Battle.net products. Contributions
adding support for other Blizzard products are welcome.

## Features

### Library

Rust crates for building your own tools:

- Binary format parsers for all NGDP/CASC formats
- CDN client with automatic failover
- Encryption key management
- Multi-layer caching
- Local CASC storage (IDX/KMT indices, data archives, shared memory IPC)

**WASM Compatible**: Core libraries compile to WebAssembly (`wasm32-unknown-unknown`),
enabling browser-based tools and web applications. All cryptographic and format
parsing code uses pure Rust implementations with no C dependencies.

### Ribbit Server

A Ribbit protocol server for hosting and distributing custom game builds.
Intended for mod developers and private server operators who need to serve
their own content to clients.

### Planned

These features are implemented in an internal development branch and are being
ported incrementally:

- **CLI Tools**: Browse and extract files from local WoW installations, download
  builds from CDN, mirror builds for archival
- **Download Agent**: HTTP service (port 1120) for downloading World of Warcraft
  products with automatic CDN failover

## Project Status

Under active development. See [CHANGELOG.md](CHANGELOG.md) for progress.

## Related Projects

### [cascette-py](https://github.com/wowemulation-dev/cascette-py)

Python prototyping environment for NGDP/CASC format analysis and verification.

## Development

This project requires Rust 1.92.0 and mdbook for documentation. You can install
and manage these dependencies automatically with [mise](https://mise.jdx.dev/):

```bash
mise install
```

This reads `.mise.toml` from the project root and installs the pinned tool
versions.

## Support the Project

If you find this project useful, please consider
[sponsoring the project](https://github.com/sponsors/danielsreichenbach).

This is currently a nights-and-weekends effort by one person. Funding goals:

- **20 hours/week** - Sustained funding to dedicate real development time
  instead of squeezing it into spare hours
- **Public CDN mirror** - Host a community mirror for World of Warcraft builds,
  ensuring long-term availability of historical game data

## Contributing

See the [Contributing Guide](CONTRIBUTING.md) for development setup and
guidelines. Thanks to all [contributors](CONTRIBUTORS.md).

## License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

You may choose to use either license at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

---

**Note**: This project is not affiliated with Blizzard Entertainment. It is
an independent implementation based on reverse engineering by the World of
Warcraft emulation community.

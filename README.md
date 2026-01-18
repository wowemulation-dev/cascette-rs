# cascette-rs

Rust implementation of Blizzard's NGDP content distribution system.

<div align="center">

[![Discord](https://img.shields.io/discord/1394228766414471219?logo=discord&style=flat-square)](https://discord.gg/Q44pPMvGEd)
[![Sponsor](https://img.shields.io/github/sponsors/wowemulation-dev?logo=github&style=flat-square)](https://github.com/sponsors/wowemulation-dev)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![WASM](https://img.shields.io/badge/WASM-compatible-blueviolet.svg)](https://webassembly.org/)

</div>

## What is NGDP?

NGDP (Next Generation Distribution Pipeline) is the system Blizzard uses to
distribute and update all modern games including World of Warcraft, Diablo IV,
and Overwatch. It consists of:

- **TACT** - Content delivery via CDN (downloading game files)
- **CASC** - Local storage format (how files are stored on disk)

Files are identified by content hash rather than filename, enabling efficient
patching, verification, and deduplication.

cascette-rs focuses on World of Warcraft and Battle.net products. Contributions
adding support for other Blizzard products are welcome.

## What does cascette-rs provide?

### CLI Tools

- Browse and extract files from local WoW installations
- Download builds directly from Blizzard's CDN
- Mirror entire builds for archival and preservation
- Query build information and version history

### Ribbit Server

A Ribbit protocol server for hosting and distributing custom game builds.
Intended for mod developers and private server operators who need to serve
their own content to clients.

### Download Agent

An HTTP service (port 1120) for downloading World of Warcraft products. Downloads
from official Blizzard CDNs with automatic fallback to community archive mirrors.
Useful for:

- Archivists preserving historical builds
- Players on Classic private servers needing specific client versions
- Developers testing against particular game versions

### Library

Rust crates for building your own tools:

- Binary format parsers for all NGDP/CASC formats
- CDN client with automatic failover
- Encryption key management
- Multi-layer caching

**WASM Compatible**: Core libraries compile to WebAssembly (`wasm32-unknown-unknown`),
enabling browser-based tools and web applications. All cryptographic and format
parsing code uses pure Rust implementations with no C dependencies.

## Project Status

Under active development. See [CHANGELOG.md](CHANGELOG.md) for progress.

## Community

cascette-rs is developed in the open with the emulation, archival, and modding
communities in mind. Our Discord brings together modders, developers of tools
for modern WoW, and developers of tools for WoW 3.3.5a.

**This project is and will always be open source.**

[Join the Discord](https://discord.gg/Q44pPMvGEd)

## Support the Project

If you find cascette-rs useful, please consider [sponsoring the project](https://github.com/sponsors/wowemulation-dev).

cascette-rs is currently a nights-and-weekends effort by one person. Funding
goals:

- **20 hours/week** - Sustained funding to dedicate real development time to
  the project instead of squeezing it into spare hours
- **Public CDN mirror** - Host a community mirror for World of Warcraft builds,
  ensuring long-term availability of historical game data

## Related Projects

### [cascette-py](https://github.com/wowemulation-dev/cascette-py)

Python prototyping environment for NGDP/CASC format analysis and verification.

## Contributing

- [CONTRIBUTING.md](CONTRIBUTING.md) - Contribution guidelines
- [CONTRIBUTORS.md](CONTRIBUTORS.md) - Contributors list

## License

Dual-licensed under MIT or Apache 2.0 at your option.

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

---

**Note**: This project is not affiliated with Blizzard Entertainment. It is
an independent implementation based on reverse engineering by the World of
Warcraft emulation community.

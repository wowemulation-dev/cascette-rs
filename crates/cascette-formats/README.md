# cascette-formats

Binary format parsers and builders for NGDP/CASC file formats.

## Status

Working implementation of all 14 NGDP/CASC formats.

## Features

- Parser and builder for each format with round-trip validation
- Builder-as-mutator pattern for modifying existing files
- Zero-copy parsing where possible using binrw
- Big-endian byte order (NGDP standard)

## Supported Formats

- `archive` - CDN archive indices for content location
- `blte` - Block Table Encoded compression and encryption
- `bpsv` - Blizzard Pipe-Separated Values format
- `cdn` - CDN-specific configuration formats
- `config` - Build, CDN, product, and patch configurations
- `download` - Download priority manifests
- `encoding` - Content key to encoding key mappings
- `espec` - Encoding specification format
- `install` - Installation manifests with tagging
- `patch_archive` - PA differential patch format
- `root` - File catalog (v1-v4 supported)
- `tvfs` - TACT Virtual File System (CASC v3)
- `zbsdiff` - ZBSDIFF1 binary patches

## Dependencies

- `binrw` - Binary parsing and building
- `thiserror` - Error handling
- `flate2` - zlib compression
- `lz4_flex` - LZ4 compression (pure Rust, WASM compatible)
- `cascette-crypto` - Content key hashing and encryption

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

---

**Note**: This project is not affiliated with Blizzard Entertainment. It is
an independent implementation based on reverse engineering by the World of
Warcraft emulation community.

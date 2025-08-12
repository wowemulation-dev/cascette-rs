# tact-parser

Parser for TACT (Trusted Application Content Transfer) file formats used in Blizzard's NGDP distribution system.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tact-parser = "0.4.3"
```

## Overview

This crate provides parsers for various TACT file formats used by Blizzard's content delivery network:

- **Encoding Files** - Maps content keys (CKey) to encoded keys (EKey)
- **Install Manifests** - Lists files for installation with tag-based filtering
- **Download Manifests** - Defines download priorities and file grouping
- **Size Files** - Tracks file sizes for installation planning
- **Build Configurations** - Key-value format for build metadata
- **WoW Root Files** - Maps file IDs to content hashes
- **TVFS (TACT Virtual File System)** - Virtual filesystem structure
- **ESpec Parser** - Encoding specification parser for BLTE compression modes

## Features

### Implemented Parsers

- ✅ **Encoding Table** (`encoding.rs`)
  - CKey ↔ EKey bidirectional mapping
  - Page-based structure with checksums
  - Support for multiple encoding formats

- ✅ **Install Manifest** (`install.rs`)
  - Tag-based file filtering
  - File metadata (name, MD5, size)
  - Efficient tag matching

- ✅ **Download Manifest** (`download.rs`)
  - Priority-based download ordering
  - File grouping for batch operations
  - Size tracking for bandwidth planning

- ✅ **Size Files** (`size.rs`)
  - Installed and download size tracking
  - File count statistics
  - Installation planning support

- ✅ **Build Config** (`config.rs`)
  - Key-value configuration parsing
  - Support for multi-value keys
  - Build metadata extraction

- ✅ **WoW Root** (`wow_root.rs`)
  - File ID to content hash mapping
  - Locale and content flag support
  - Efficient lookup structures

- ✅ **TVFS** (`tvfs.rs`)
  - Virtual filesystem parsing with specification compliance
  - Big-endian 40-bit integer support for modern game builds
  - Directory structure recreation and file attribute support

- ✅ **ESpec Parser** (`espec.rs`)
  - Complete EBNF grammar implementation for BLTE compression
  - Support for all modes: None, ZLib, Encrypted, BlockTable, BCPack, GDeflate
  - Complex block specifications with size expressions (K/M units, multipliers)
  - Integration with BLTE decompression system

### Utility Features

- **40-bit Integer Support** - Big/little-endian implementation
  - Standard 40-bit integers and TACT encoding format (1 byte + 4-byte BE u32)
  - Support for file sizes up to 1TB with proper endianness handling
- **Variable-length Integer Parsing** - Varint implementation
- **Jenkins Hash** - TACT's hash algorithm implementation
- **Compression Support** - Integration with BLTE decompression

## Usage Examples

### Parse Encoding File

```rust
use tact_parser::EncodingFile;

let data = std::fs::read("encoding")?;
let encoding = EncodingFile::parse(&data)?;

// Look up EKey for a given CKey
if let Some(ekey) = encoding.get_ekey(&ckey) {
    println!("Found EKey: {:?}", ekey);
}
```

### Parse Install Manifest

```rust
use tact_parser::InstallFile;

let data = std::fs::read("install")?;
let install = InstallFile::parse(&data)?;

// Filter files by tags
let files = install.filter_files(&["enUS", "Windows"]);
for file in files {
    println!("{}: {} bytes", file.name, file.size);
}
```

### Parse ESpec Compression Specification

```rust
use tact_parser::ESpec;

// Parse a complex block table specification
let spec = ESpec::parse("b:{1M*3=z:9,512K=n,*=z:6}")?;

// Check compression properties
println!("Uses compression: {}", spec.is_compressed());
println!("Type: {}", spec.compression_type());

// Convert back to string format
println!("ESpec: {}", spec.to_string());
```

### Parse Build Configuration

```rust
use tact_parser::parse_build_config;

let data = std::fs::read_to_string("config")?;
let config = parse_build_config(&data)?;

// Access configuration values
if let Some(version) = config.get("version") {
    println!("Build version: {}", version[0]);
}
```

## Performance

The parsers are optimized for:

- Memory efficiency with streaming where possible
- Lookups using appropriate data structures
- Minimal allocations during parsing
- Support for large files (GB+ encoding tables)

## Integration

This crate integrates with other cascette-rs components:

- Uses `blte` for decompression
- Works with `ngdp-crypto` for encrypted content
- Compatible with `ngdp-cdn` for downloading files
- Used by `ngdp-client` for CLI operations

## License

This crate is dual-licensed under either:

- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

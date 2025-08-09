# tact-parser

Parser for TACT (Trusted Application Content Transfer) file formats used in Blizzard's NGDP distribution system.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tact-parser = "0.3"
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
  - Virtual filesystem parsing
  - Directory structure recreation
  - File attribute support

### Utility Features

- **40-bit Integer Support** - Custom type for TACT's 40-bit integers
- **Variable-length Integer Parsing** - Efficient varint implementation
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
- Fast lookups using appropriate data structures
- Minimal allocations during parsing
- Support for large files (GB+ encoding tables)

## Integration

This crate integrates with other cascette-rs components:

- Uses `blte` for decompression
- Works with `ngdp-crypto` for encrypted content
- Compatible with `ngdp-cdn` for downloading files
- Used by `ngdp-client` for CLI operations

## License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.

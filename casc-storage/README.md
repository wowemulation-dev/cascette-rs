# casc-storage

CASC (Content Addressable Storage Container) implementation for local storage of NGDP content.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
casc-storage = "0.4.3"
```

## Overview

This crate provides local storage functionality for NGDP content using the CASC format, which is Blizzard's content-addressable storage system. It handles:

- **Archive Management**: Reading and writing `.idx` and data archive files
- **Index Files**: Managing the index structures that map content keys to archive locations
- **Loose Files**: Support for both archived and loose file storage
- **Progressive Loading**: Memory loading of large archives
- **Manifest Integration**: TACT manifest support for installation tracking

## Features

- **Archive Support**: Read/write CASC archive files (data.XXX)
- **Index Management**: Handle `.idx` files with bucket organization
- **Binary Search**: Content lookup in sorted indices
- **Memory Usage**: Progressive loading and memory-mapped I/O
- **Cache**: Thread-safe access to frequently used data
- **TACT Integration**: Support for install and download manifests

## Usage

### Reading from CASC Storage

```rust
use casc_storage::CascStorage;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open existing CASC storage
    let storage = CascStorage::open(Path::new("/path/to/data"))?;

    // Look up content by key
    let key = "abc123def456..."; // 32-char hex string
    if let Some(data) = storage.read(key).await? {
        println!("Found content: {} bytes", data.len());
    }

    Ok(())
}
```

### Writing to CASC Storage

```rust
use casc_storage::CascStorageBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create new CASC storage
    let mut builder = CascStorageBuilder::new("/path/to/data")?;

    // Add content
    let content = b"Hello, CASC!";
    let key = builder.add_content(content)?;

    println!("Stored with key: {}", key);

    // Build the storage (creates archives and indices)
    builder.build().await?;

    Ok(())
}
```

## Architecture

The CASC storage system consists of:

1. **Data Archives** (`data.000`, `data.001`, etc.)
   - Contains the actual file content
   - Limited to ~1GB per archive
   - Files can span multiple archives

2. **Index Files** (`.idx` files)
   - Maps content keys to archive locations
   - Uses truncated keys (9 bytes) for space optimization
   - Organized in buckets (00-0F) for parallel access

3. **Loose Files** (`data/XX/XXXXXXXX...`)
   - Alternative to archived storage
   - Direct file system storage using key-based paths
   - Used for frequently modified content

## Status

⚠️ **Beta**: This crate is under active development. The API may change before 1.0 release.

## License

This crate is dual-licensed under either:

- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

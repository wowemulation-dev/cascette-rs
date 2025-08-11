# ngdp-patch

Patch file support for NGDP (Next Generation Distribution Pipeline) content updates.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-patch = "0.4"
```

## Overview

This crate provides functionality for creating and applying binary patches in Blizzard's NGDP system. It implements the patch formats used for incremental game updates, reducing download sizes for clients.

## Features

- 🔧 **Patch Generation**: Create binary diff patches between file versions
- 📦 **Patch Application**: Apply patches to transform old files to new versions
- 🗜️ **Compression**: BLTE-compressed patch storage
- 🔐 **Verification**: Checksum validation for patch integrity
- ⚡ **Optimized**: Efficient algorithms for large game files

## Usage

### Creating a Patch

```rust
use ngdp_patch::PatchBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let old_file = std::fs::read("game_v1.dat")?;
    let new_file = std::fs::read("game_v2.dat")?;
    
    // Create a patch
    let patch = PatchBuilder::new()
        .source(&old_file)
        .target(&new_file)
        .build()?;
    
    // Save the patch
    std::fs::write("update.patch", patch)?;
    
    Ok(())
}
```

### Applying a Patch

```rust
use ngdp_patch::PatchApplier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let old_file = std::fs::read("game_v1.dat")?;
    let patch_data = std::fs::read("update.patch")?;
    
    // Apply the patch
    let applier = PatchApplier::new(patch_data)?;
    let new_file = applier.apply(&old_file)?;
    
    // Save the updated file
    std::fs::write("game_v2.dat", new_file)?;
    
    Ok(())
}
```

## Patch Format

NGDP patches use a custom binary diff format optimized for game content:

- **Header**: Magic bytes, version, and metadata
- **Operations**: Copy, add, and modify instructions
- **Data**: New content blocks referenced by operations
- **Checksums**: MD5 hashes for validation

## Performance

The patch system is optimized for:
- Large files (multi-GB game assets)
- Minimal memory usage during application
- Parallel processing where possible
- Streaming operation for reduced memory footprint

## Status

⚠️ **Beta**: This crate is under active development. Core functionality is working but the API may change.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
# blte

BLTE (Block Table Encoded) decompression library for Blizzard's NGDP/CASC system.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
blte = "0.3"
ngdp-crypto = "0.3"  # Required for encrypted content
```

## Overview

This crate provides complete support for decompressing BLTE-encoded files used in Blizzard's content distribution system. BLTE is a container format that supports multiple compression algorithms and encryption.

## Features

- All compression modes supported:
  - None (N) - Uncompressed data
  - ZLib (Z) - Standard deflate compression
  - LZ4 (4) - Fast compression
  - Frame (F) - Nested BLTE frames
  - Encrypted (E) - Salsa20/ARC4 encrypted blocks
- Multi-chunk file handling
- Checksum verification
- Streaming decompression for large files
- Zero-copy where possible

## Usage

```rust
use blte::decompress_blte;
use ngdp_crypto::KeyService;

// For unencrypted content
let data = std::fs::read("file.blte")?;
let decompressed = decompress_blte(data, None)?;

// For encrypted content
let key_service = KeyService::new();
let decompressed = decompress_blte(data, Some(&key_service))?;

// Streaming decompression for large files
use blte::{BLTEStream, create_streaming_reader};
use std::io::Read;

let mut stream = create_streaming_reader(data, Some(key_service))?;
let mut buffer = [0u8; 8192];
let mut decompressed = Vec::new();

loop {
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 { break; }
    decompressed.extend_from_slice(&buffer[..bytes_read]);
}
```

## Compression Modes

### Mode 'N' (None)
Raw uncompressed data. The first byte is the mode indicator, followed by the raw data.

### Mode 'Z' (ZLib)
Standard deflate compression using the flate2 crate.

### Mode '4' (LZ4)
LZ4 compression for fast decompression of large files.

### Mode 'F' (Frame)
Recursive BLTE frame - the payload is another complete BLTE file.

### Mode 'E' (Encrypted)
Encrypted blocks that must be decrypted before decompression. Supports:
- Salsa20 stream cipher (type 'S')
- ARC4/RC4 cipher (type 'A')

## Multi-Chunk Files

Large files are split into multiple chunks for efficient streaming and parallel processing:

```rust
use blte::BLTEFile;

let blte_file = BLTEFile::parse(data)?;
if blte_file.is_multi_chunk() {
    println!("File has {} chunks", blte_file.chunk_count());
}
```

## Dependencies

- `flate2` - ZLib decompression
- `lz4_flex` - LZ4 decompression
- `ngdp-crypto` - Encryption support
- `md5` - Checksum verification

## License

MIT OR Apache-2.0
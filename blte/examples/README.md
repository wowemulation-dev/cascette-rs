# BLTE Examples

This directory contains examples demonstrating how to use the `blte` crate for decompressing BLTE-encoded files.

## Available Examples

Currently, the `blte` crate doesn't have standalone examples as it's primarily used as a library by other crates. However, you can find usage examples in the main README.md and in the integration tests.

## Usage in Other Crates

The `blte` crate is used extensively by:

- `tact-parser` - For decompressing manifest files
- `ngdp-client` - For CLI operations involving BLTE files

## Basic Usage

```rust
use blte::decompress_blte;
use ngdp_crypto::KeyService;

// For unencrypted content
let data = std::fs::read("file.blte")?;
let decompressed = decompress_blte(data, None)?;

// For encrypted content
let key_service = KeyService::new();
let decompressed = decompress_blte(data, Some(&key_service))?;
```

## Running Tests

See the `tests/` directory for comprehensive examples of how the BLTE decompression works with various compression modes and encryption types.

```bash
cargo test -p blte
```

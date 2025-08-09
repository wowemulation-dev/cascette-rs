# ngdp-crypto Examples

This directory contains examples demonstrating how to use the `ngdp-crypto` crate for encryption/decryption operations.

## Available Examples

Currently, the `ngdp-crypto` crate doesn't have standalone examples as it's primarily used as a library by other crates. However, you can find comprehensive usage examples in the main README.md and in the unit tests.

## Usage in Other Crates

The `ngdp-crypto` crate is used extensively by:

- `blte` - For decrypting encrypted BLTE blocks
- `ngdp-client` - For key management commands
- `tact-parser` - For handling encrypted manifest files

## Basic Usage

### Key Management

```rust
use ngdp_crypto::KeyService;

// Initialize and load keys
let mut key_service = KeyService::new();
key_service.load_from_standard_dirs()?;

// Add a specific key
key_service.add_key(0xFA505078126ACB3E, [0xBD, 0xC5, /* ... */]);

// Get a key for decryption
if let Some(key) = key_service.get_key(0xFA505078126ACB3E) {
    println!("Found key: {:?}", key);
}
```

### Decryption

```rust
use ngdp_crypto::{decrypt_salsa20, decrypt_arc4};

// Salsa20 decryption (modern WoW)
let decrypted = decrypt_salsa20(
    encrypted_data,
    key,
    iv,
    block_index
)?;

// ARC4 decryption (legacy content)
let decrypted = decrypt_arc4(
    encrypted_data,
    key,
    iv,
    block_index
)?;
```

## Running Tests

See the `tests/` directory and unit tests for comprehensive examples:

```bash
cargo test -p ngdp-crypto
```

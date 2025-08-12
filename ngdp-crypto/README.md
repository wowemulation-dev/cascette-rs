# ngdp-crypto

Encryption and decryption support for Blizzard's NGDP/TACT system.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-crypto = "0.4.3"
```

## Overview

This crate provides cryptographic functionality for handling encrypted content in Blizzard's games. It implements the specific cipher configurations and key management required for TACT (Trusted Application Content Transfer) files.

## Features

- **Salsa20 Stream Cipher**: Encryption used in recent WoW versions
- **ARC4 (RC4) Cipher**: Legacy encryption for older content
- **Key Management**: Loading and management of encryption keys
- **Multiple Key Sources**: File-based, environment variables, and hardcoded keys
- **Format Support**: CSV, TXT, and TSV key file formats

## Usage

### Basic Decryption

```rust
use ngdp_crypto::{KeyService, decrypt_salsa20};

// Initialize key service
let mut key_service = KeyService::new();
key_service.load_from_standard_dirs()?;

// Get a key by name
let key_name = 0xFA505078126ACB3E_u64;
if let Some(key) = key_service.get_key(key_name) {
    // Decrypt data
    let decrypted = decrypt_salsa20(encrypted_data, key, iv, block_index)?;
}
```

### Key File Management

The KeyService searches for key files in:

- `~/.config/cascette/`
- `~/.tactkeys/`
- Path specified in `CASCETTE_KEYS_PATH` environment variable

Supported key file formats:

```text
# CSV Format (WoW.txt style)
FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200

# TXT Format with description
FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200 WoW 8.2.0.30898 Nazjatar Cinematic

# TSV Format
FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200
```

### Cipher Details

#### Salsa20

- Uses 16-byte keys extended to 32 bytes (by duplication)
- 4-byte IV extended to 8 bytes (by duplication)
- Block index XORed with first 4 bytes of IV
- Compatible with WoW's BLTE encryption

#### ARC4/RC4

- Combines key (16 bytes) + IV (4 bytes) + block_index (4 bytes)
- Padded to 32 bytes with zeros
- Used for legacy content

## Key Sources

The crate includes hardcoded keys for common WoW content and can load additional keys from:

1. **TACTKeys Repository**: Community-maintained key database
2. **Local Files**: User-provided key files
3. **Built-in Keys**: Common keys hardcoded in the library

## Examples

### Loading Keys from File

```rust
use ngdp_crypto::KeyService;
use std::path::Path;

let mut key_service = KeyService::new();
let keys_loaded = key_service.load_key_file(Path::new("/path/to/WoW.txt"))?;
println!("Loaded {} keys", keys_loaded);
```

### Decrypting BLTE Blocks

```rust
use ngdp_crypto::{decrypt_salsa20, decrypt_arc4};

// For Salsa20 encrypted blocks
let decrypted = decrypt_salsa20(
    encrypted_data,
    key,
    iv,
    block_index
)?;

// For ARC4 encrypted blocks
let decrypted = decrypt_arc4(
    encrypted_data,
    key,
    iv,
    block_index
)?;
```

## Performance

- Zero-copy operations where possible
- Key lookups using HashMap
- Minimal allocations during decryption

## License

This crate is dual-licensed under either:

- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

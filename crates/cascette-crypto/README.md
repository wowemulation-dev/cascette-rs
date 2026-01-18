# cascette-crypto

Cryptographic operations for NGDP/CASC content verification and encryption.

## Features

- **MD5 hashing** - Content keys and encoding keys for file identification
- **Jenkins96** - Hash function for CASC path lookups
- **Salsa20** - Stream cipher for BLTE encrypted blocks (CASC variant)
- **ARC4** - Stream cipher for legacy encrypted content
- **TACT key management** - In-memory store with trait for custom backends
- **WASM compatible** - Compiles to `wasm32-unknown-unknown` without configuration

## Usage

### Content Key Generation

```rust
use cascette_crypto::ContentKey;

let data = b"file contents";
let key = ContentKey::from_data(data);
println!("Content key: {}", key.to_hex());
```

### Decrypting BLTE Content

```rust
use cascette_crypto::{Salsa20Cipher, TactKeyStore};

// Get encryption key from store
let store = TactKeyStore::new(); // Includes hardcoded WoW keys
let key = store.get(0xFA505078126ACB3E).expect("key not found");

// Decrypt content
let iv = [0x01, 0x02, 0x03, 0x04];
let mut cipher = Salsa20Cipher::new(key, &iv, 0).expect("cipher init");
let mut data = encrypted_data.to_vec();
cipher.apply_keystream(&mut data);
```

### Custom Key Storage

Implement `TactKeyProvider` for persistent storage:

```rust
use cascette_crypto::{TactKeyProvider, TactKey, CryptoError};

struct KeyringStore { /* ... */ }

impl TactKeyProvider for KeyringStore {
    fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        // Look up from OS keychain, database, etc.
        todo!()
    }

    fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
        todo!()
    }

    fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        todo!()
    }

    fn key_count(&self) -> Result<usize, CryptoError> {
        todo!()
    }

    fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError> {
        todo!()
    }
}
```

### Loading Keys from Files

```rust
use cascette_crypto::TactKeyStore;

let mut store = TactKeyStore::empty();

// CSV format: key_id,key_hex
let csv = "FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200";
store.load_from_csv(csv);

// TXT format: key_id key_hex (whitespace separated)
let txt = "FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200";
store.load_from_txt(txt);
```

## WASM Support

The crate compiles to WebAssembly without any feature flags:

```bash
cargo build --target wasm32-unknown-unknown -p cascette-crypto
```

All cryptographic operations work in WASM. Key storage is in-memory only;
applications should implement `TactKeyProvider` for browser-based persistence
(e.g., IndexedDB, localStorage).

## Modules

| Module | Purpose |
|--------|---------|
| `md5` | ContentKey, EncodingKey, FileDataId types |
| `jenkins` | Jenkins96 hash for path lookups |
| `salsa20` | Salsa20 cipher (CASC 16-byte key variant) |
| `arc4` | ARC4 cipher for legacy content |
| `keys` | TactKey, TactKeyStore (in-memory) |
| `store_trait` | TactKeyProvider trait for custom backends |
| `error` | CryptoError type |

## CASC-Specific Implementation Notes

### Salsa20 Variant

CASC uses a non-standard Salsa20 configuration:

- 16-byte keys (not 32-byte) with "expand 16-byte k" constants
- 4-byte IV extended to 8 bytes internally
- Block index XORed with IV for multi-block files

### Key Format

TACT keys are 16-byte values identified by 64-bit IDs:

```text
Key ID:  FA505078126ACB3E (8 bytes, hex)
Key:     BDC51862ABED79B2DE48C8E7E66C6200 (16 bytes, hex)
```

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

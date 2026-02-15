# Salsa20 Encryption in CASC

Salsa20 is the primary stream cipher used for encrypting sensitive content in
CASC archives. It provides fast, secure encryption for game assets while
maintaining streaming capabilities.

## Overview

Salsa20 in CASC provides:

- Stream cipher encryption for efficient processing

- Per-file key management

- Initialization vector (IV) support

- 128-bit (16-byte) keys with tau constants

## Algorithm Details

### Salsa20 Core

Salsa20 is a stream cipher designed by Daniel J. Bernstein:

- **Key size**: 128 bits (16 bytes) in CASC; 256 bits (32 bytes) in standard
  Salsa20

- **Nonce/IV size**: 64 bits (8 bytes)

- **Block size**: 512 bits (64 bytes)

- **Rounds**: 20 (reduced variants use 8 or 12)

### Core Function

```rust
fn salsa20_core(input: &[u32; 16]) -> [u32; 16] {
    let mut x = *input;

    // 20 rounds (10 double-rounds)
    for _ in 0..10 {
        // Column round
        quarter_round(&mut x, 0, 4, 8, 12);
        quarter_round(&mut x, 5, 9, 13, 1);
        quarter_round(&mut x, 10, 14, 2, 6);
        quarter_round(&mut x, 15, 3, 7, 11);

        // Row round
        quarter_round(&mut x, 0, 1, 2, 3);
        quarter_round(&mut x, 5, 6, 7, 4);
        quarter_round(&mut x, 10, 11, 8, 9);
        quarter_round(&mut x, 15, 12, 13, 14);
    }

    // Add input to output
    for i in 0..16 {
        x[i] = x[i].wrapping_add(input[i]);
    }

    x
}

fn quarter_round(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[b] ^= (x[a].wrapping_add(x[d])).rotate_left(7);
    x[c] ^= (x[b].wrapping_add(x[a])).rotate_left(9);
    x[d] ^= (x[c].wrapping_add(x[b])).rotate_left(13);
    x[a] ^= (x[d].wrapping_add(x[c])).rotate_left(18);
}
```

## CASC Implementation

### BLTE Encryption Block

In BLTE files, encrypted blocks use format:

```text
[0x45] [key_name_size:1] [key_name:8] [iv_size:1] [iv:4] [type:1]
[encrypted_data...]
```

Where:

- `0x45`: 'E' marker for encrypted block

- `key_name`: 64-bit key identifier

- `iv`: Initialization vector (1-8 bytes, typically 4)

- `type`: 0x53 ('S') for Salsa20. 0x41 ('A') for ARC4 in legacy CASC
  versions (not used in TACT 3.13.3+)

### Key Lookup

CASC uses a 64-bit key name to look up the 16-byte encryption key from a key
store. The agent calls a key getter callback with the key name; there is no
key derivation in the encryption path.

```rust
struct CASCKeyManager {
    keys: HashMap<u64, [u8; 16]>,  // key_name -> 16-byte key
}

impl CASCKeyManager {
    pub fn get_key(&self, key_name: u64) -> Option<[u8; 16]> {
        self.keys.get(&key_name).copied()
    }
}
```

### IV Modification for Chunks

For multi-chunk BLTE files, the IV is modified per chunk:

```rust
fn modify_iv_for_chunk(base_iv: u32, chunk_index: usize) -> u32 {
    let mut iv_bytes = base_iv.to_le_bytes();

    // XOR with chunk index
    for i in 0..4 {
        iv_bytes[i] ^= ((chunk_index >> (i * 8)) & 0xFF) as u8;
    }

    u32::from_le_bytes(iv_bytes)
}
```

## Salsa20 State Setup

### State Initialization

```rust
struct Salsa20State {
    state: [u32; 16],
    counter: u64,
}

impl Salsa20State {
    pub fn new(key: &[u8; 16], nonce: &[u8; 8]) -> Self {
        let mut state = [0u32; 16];

        // Tau constants "expand 16-byte k" (CASC uses 16-byte keys)
        state[0]  = 0x61707865; // "expa"
        state[5]  = 0x3120646e; // "nd 1"
        state[10] = 0x79622d36; // "6-by"
        state[15] = 0x6b206574; // "te k"

        // 16-byte key placed at positions 1-4 and duplicated at 11-14
        for i in 0..4 {
            let word = u32::from_le_bytes([
                key[i * 4],
                key[i * 4 + 1],
                key[i * 4 + 2],
                key[i * 4 + 3],
            ]);
            state[1 + i] = word;
            state[11 + i] = word;  // Duplicate for 16-byte key mode
        }

        // Counter (initially 0)
        state[8] = 0;
        state[9] = 0;

        // Nonce
        state[6] = u32::from_le_bytes([nonce[0], nonce[1], nonce[2], nonce[3]]);
        state[7] = u32::from_le_bytes([nonce[4], nonce[5], nonce[6], nonce[7]]);

        Salsa20State { state, counter: 0 }
    }
}
```

## Encryption/Decryption

### Stream Generation

```rust
impl Salsa20State {
    pub fn generate_keystream(&mut self, output: &mut [u8]) {
        let mut pos = 0;

        while pos < output.len() {
            // Generate next block
            let block = salsa20_core(&self.state);

            // Convert to bytes
            let block_bytes = unsafe {
                std::slice::from_raw_parts(
                    block.as_ptr() as *const u8,
                    64
                )
            };

            // Copy to output
            let copy_len = std::cmp::min(64, output.len() - pos);
            output[pos..pos + copy_len]
                .copy_from_slice(&block_bytes[..copy_len]);

            // Increment counter
            self.increment_counter();
            pos += copy_len;
        }
    }

    fn increment_counter(&mut self) {
        self.counter += 1;
        self.state[8] = (self.counter & 0xFFFFFFFF) as u32;
        self.state[9] = (self.counter >> 32) as u32;
    }
}
```

### Decryption Process

```rust
pub fn decrypt_salsa20(
    ciphertext: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 8]
) -> Vec<u8> {
    let mut state = Salsa20State::new(key, nonce);
    let mut keystream = vec![0u8; ciphertext.len()];
    state.generate_keystream(&mut keystream);

    // XOR ciphertext with keystream
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    for i in 0..ciphertext.len() {
        plaintext.push(ciphertext[i] ^ keystream[i]);
    }

    plaintext
}
```

## CASC-Specific Usage

### BLTE Decryption

```rust
fn decrypt_blte_chunk(
    chunk_data: &[u8],
    chunk_index: usize,
    key_manager: &CASCKeyManager
) -> Result<Vec<u8>> {
    // Parse encryption header
    let key_name_size = chunk_data[0] as usize;
    let key_name = u64::from_le_bytes(
        chunk_data[1..1 + key_name_size].try_into()?
    );

    let iv_offset = 1 + key_name_size;
    let iv_size = chunk_data[iv_offset] as usize;
    let base_iv = u32::from_le_bytes(
        chunk_data[iv_offset + 1..iv_offset + 1 + iv_size].try_into()?
    );

    let cipher_type = chunk_data[iv_offset + 1 + iv_size];

    if cipher_type != 0x53 {  // 'S' for Salsa20
        return Err("Not Salsa20 encrypted");
    }

    // Get encryption key
    let key = key_manager.get_key(key_name)
        .ok_or("Key not found")?;

    // Modify IV for chunk
    let iv = modify_iv_for_chunk(base_iv, chunk_index);
    let mut nonce = [0u8; 8];
    nonce[..4].copy_from_slice(&iv.to_le_bytes());

    // Decrypt data
    let encrypted_offset = iv_offset + 1 + iv_size + 1;
    let ciphertext = &chunk_data[encrypted_offset..];

    Ok(decrypt_salsa20(ciphertext, &key, &nonce))
}
```

## Known Keys

CASC uses various encryption keys for different content:

```rust
// Example key names (actual keys not included for legal reasons)
const CINEMATIC_KEY: u64 = 0xFAC5C7F366D20C85;
const ACHIEVEMENT_KEY: u64 = 0x0123456789ABCDEF;
const PVP_KEY: u64 = 0xDEADBEEFCAFEBABE;
```

## Performance Optimization

### SIMD Implementation

Using SIMD for parallel processing:

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

unsafe fn salsa20_core_simd(input: &[u32; 16]) -> [u32; 16] {
    // Load state into SIMD registers
    let mut row0 = _mm_loadu_si128(input[0..4].as_ptr() as *const __m128i);
    let mut row1 = _mm_loadu_si128(input[4..8].as_ptr() as *const __m128i);
    let mut row2 = _mm_loadu_si128(input[8..12].as_ptr() as *const __m128i);
    let mut row3 = _mm_loadu_si128(input[12..16].as_ptr() as *const __m128i);

    // Perform rounds using SIMD operations
    // ... (implementation details)

    // Store results
    let mut output = [0u32; 16];
    _mm_storeu_si128(output[0..4].as_mut_ptr() as *mut __m128i, row0);
    _mm_storeu_si128(output[4..8].as_mut_ptr() as *mut __m128i, row1);
    _mm_storeu_si128(output[8..12].as_mut_ptr() as *mut __m128i, row2);
    _mm_storeu_si128(output[12..16].as_mut_ptr() as *mut __m128i, row3);

    output
}
```

### Buffered Decryption

For large files:

```rust
struct BufferedSalsa20 {
    state: Salsa20State,
    buffer: [u8; 4096],
    buffer_pos: usize,
}

impl BufferedSalsa20 {
    pub fn decrypt_stream<R: Read, W: Write>(
        &mut self,
        input: &mut R,
        output: &mut W
    ) -> Result<()> {
        let mut cipher_buffer = [0u8; 4096];

        loop {
            let bytes_read = input.read(&mut cipher_buffer)?;
            if bytes_read == 0 {
                break;
            }

            self.state.generate_keystream(&mut self.buffer[..bytes_read]);

            for i in 0..bytes_read {
                self.buffer[i] ^= cipher_buffer[i];
            }

            output.write_all(&self.buffer[..bytes_read])?;
        }

        Ok(())
    }
}
```

## Security Considerations

1. **Key Management**: Never store keys in source code
2. **IV Uniqueness**: Ensure IVs are never reused with same key
3. **Side Channels**: Use constant-time operations
4. **Key Rotation**: Regularly update encryption keys
5. **Secure Storage**: Protect key storage locations

## Testing

### Test Vectors

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_salsa20_encryption() {
        let key = [0u8; 32];
        let nonce = [0u8; 8];
        let plaintext = b"Hello, World!";

        let ciphertext = encrypt_salsa20(plaintext, &key, &nonce);
        let decrypted = decrypt_salsa20(&ciphertext, &key, &nonce);

        assert_eq!(plaintext, &decrypted[..]);
    }
}
```

## cascette-crypto API

The `cascette-crypto` crate provides CASC-specific Salsa20 implementation.

### Basic Decryption

```rust
use cascette_crypto::salsa20::{decrypt_salsa20, Salsa20Cipher};

// CASC uses 16-byte keys and 4-byte IVs
let key: [u8; 16] = [0x01; 16];
let iv: [u8; 4] = [0x02, 0x03, 0x04, 0x05];
let block_index = 0; // First block in BLTE file

let ciphertext = &[/* encrypted data */];
let plaintext = decrypt_salsa20(ciphertext, &key, &iv, block_index)
    .expect("decryption failed");
```

### In-Place Processing

```rust
use cascette_crypto::Salsa20Cipher;

let key: [u8; 16] = [0x42; 16];
let iv: [u8; 4] = [0x11, 0x22, 0x33, 0x44];

let mut cipher = Salsa20Cipher::new(&key, &iv, 0)
    .expect("cipher creation failed");

let mut data = vec![0u8; 1024];
cipher.apply_keystream(&mut data);
```

### TACT Key Management

```rust
use cascette_crypto::{TactKeyStore, TactKey};

// Create store with hardcoded WoW keys
let store = TactKeyStore::new();

// Look up key by ID
let key_id = 0xFA505078126ACB3E_u64;
if let Some(key) = store.get(key_id) {
    // Use key for decryption
    println!("Found key: {:02X?}", key);
}

// Add custom key
let mut store = TactKeyStore::empty();
let key = TactKey::from_hex(
    0x1234567890ABCDEF,
    "0123456789ABCDEF0123456789ABCDEF"
).expect("invalid key hex");
store.add(key);

// Load keys from string content (file I/O is caller's responsibility)
let csv_content = "FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200";
store.load_from_csv(csv_content);

let txt_content = "FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200";
store.load_from_txt(txt_content);
```

### Custom Storage Backends

The `TactKeyProvider` trait allows implementing custom key storage:

```rust
use cascette_crypto::{TactKeyProvider, TactKey, CryptoError};

// Implement for keyring, database, encrypted files, etc.
struct MyKeyStore { /* ... */ }

impl TactKeyProvider for MyKeyStore {
    fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        // Look up key from your storage backend
        todo!()
    }

    fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
        // Store key in your backend
        todo!()
    }

    // ... other trait methods
}
```

### ARC4 (Legacy)

```rust
use cascette_crypto::Arc4Cipher;

// ARC4 used in older BLTE encrypted blocks
let key = b"encryption_key";
let mut cipher = Arc4Cipher::new(key)
    .expect("cipher creation failed");

let encrypted = cipher.encrypt(b"plaintext");

// Decrypt requires fresh cipher instance
let mut cipher = Arc4Cipher::new(key)
    .expect("cipher creation failed");
let decrypted = cipher.decrypt(&encrypted);
```

## Implementation Details

### CASC-Specific Differences

The CASC Salsa20 variant differs from standard Salsa20:

| Aspect | Standard Salsa20 | CASC Salsa20 |
|--------|------------------|--------------|
| Key size | 32 bytes | 16 bytes (duplicated internally) |
| IV/Nonce size | 8 bytes | 4 bytes (extended internally) |
| Constants | "expand 32-byte k" | "expand 16-byte k" |
| Block index | Counter-based | XORed with IV |

### Key Duplication

CASC uses 16-byte keys with the "expand 16-byte k" (tau) constants:

```rust
// Tau constants for 16-byte keys
state[0]  = 0x61707865; // "expa"
state[5]  = 0x3120646e; // "nd 1"
state[10] = 0x79622d36; // "6-by"
state[15] = 0x6b206574; // "te k"

// Key bytes 0-15 placed at positions 1-4
// Key bytes 0-15 repeated at positions 11-14
```

### IV Extension

The IV (1-8 bytes, typically 4) is zero-padded to 8 bytes for the Salsa20
nonce. The block index is XORed into the first 4 bytes before extension:

```rust
// XOR block index with first 4 IV bytes (for multi-chunk BLTE)
let block_bytes = (block_index as u32).to_le_bytes();
for i in 0..std::cmp::min(4, iv.len()) {
    iv[i] ^= block_bytes[i];
}

// Zero-pad IV to 8 bytes (NOT duplicated)
let mut nonce = [0u8; 8];
nonce[..iv.len()].copy_from_slice(&iv);
// Bytes iv.len()..8 remain zero
```

## Validation Status

- Verified against Agent.exe (TACT 3.13.3) binary behavior

- Integration tests with real WoW encryption keys

- Test suite validates against known BLTE 'E' mode samples

- Zero-allocation keystream generation for performance

Note: CascLib duplicates the IV (same bug as was in cascette-rs before the
fix). The Agent.exe binary is the authoritative reference and confirms
zero-padding.

### TACT Key Coverage

The cascette-crypto crate includes hardcoded TACT keys for major WoW expansions:

- Battle for Azeroth, Shadowlands, The War Within, Classic Era

Keys are stored with redacted debug output to prevent accidental logging.

## Binary Verification (Agent.exe, TACT 3.13.3)

Verified against Agent.exe (WoW Classic Era) using Binary Ninja on
2026-02-15.

### Confirmed Correct

| Claim | Agent Evidence |
|-------|---------------|
| 16-byte keys with tau constants | `sub_6fe3ff` uses tau ("expand 16-byte k") at 0x9b8e10 |
| Key duplication: bytes 0-15 at state[1-4] and state[11-14] | `sub_6fe3ff` confirmed |
| IV XOR with block_index (LE u32 into first 4 bytes) | `sub_6f5946` confirmed |
| Counter at state[8-9], starting at 0 | `sub_6fe3ff` confirmed |
| Nonce at state[6-7] | `sub_6fe3ff` confirmed |
| 20 rounds (10 double-rounds) | `sub_6fe3ff` uses SSE intrinsics for Salsa20 core |
| Agent only uses Salsa20 (0x53) for BLTE encryption | `sub_6f5c45` at 0x6f5d78 |

### Changes Applied

1. Fixed IV extension from duplication to zero-padding (confirmed bug
   in cascette-crypto, now fixed)
2. Fixed key size in overview to 128 bits (16 bytes) for CASC
3. Replaced sigma constants with tau in state initialization code
4. Replaced SHA-256 key derivation with key store lookup
5. Removed ChaCha20 variant section (no evidence in agent binary)
6. Noted ARC4 as legacy, not used in TACT 3.13.3+
7. Updated IV size from fixed "32-bit" to 1-8 bytes
8. Updated validation claim to reference Agent.exe instead of CascLib

### Source Files

Agent source paths from PDB info:
- `d:\package_cache\tact\3.13.3\src\codec\codec_encryption.cpp`
- Key expansion function: `sub_6fe3ff`
- Wrapper (always 128-bit): `sub_6fe3dc`
- Salsa20 constants at: 0x9b8e10

## References

- [Salsa20 Specification](https://cr.yp.to/snuffle/spec.pdf)

- See [BLTE Format](blte.md) for encryption in BLTE blocks

- See [Archives](archives.md) for encrypted content storage

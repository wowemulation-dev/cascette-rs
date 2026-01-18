# Salsa20 Encryption in CASC

Salsa20 is the primary stream cipher used for encrypting sensitive content in
CASC archives. It provides fast, secure encryption for game assets while
maintaining streaming capabilities.

## Overview

Salsa20 in CASC provides:

- Stream cipher encryption for efficient processing

- Per-file key management

- Initialization vector (IV) support

- ChaCha20 variant support (newer implementations)

## Algorithm Details

### Salsa20 Core

Salsa20 is a stream cipher designed by Daniel J. Bernstein:

- **Key size**: 256 bits (32 bytes)

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

- `iv`: 32-bit initialization vector

- `type`: 0x53 ('S') for Salsa20, 0x41 ('A') for ARC4

### Key Derivation

CASC uses a key name to derive the actual encryption key:

```rust
struct CASCKeyManager {
    keys: HashMap<u64, [u8; 32]>,  // key_name -> actual_key
}

impl CASCKeyManager {
    pub fn get_key(&self, key_name: u64) -> Option<[u8; 32]> {
        self.keys.get(&key_name).copied()
    }

    pub fn derive_key(&self, key_name: u64, base_key: &[u8]) -> [u8; 32] {
        // Key derivation function (game-specific)
        let mut hasher = Sha256::new();
        hasher.update(&key_name.to_le_bytes());
        hasher.update(base_key);

        let hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);
        key
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
    pub fn new(key: &[u8; 32], nonce: &[u8; 8]) -> Self {
        let mut state = [0u32; 16];

        // Constants "expand 32-byte k"
        state[0] = 0x61707865;
        state[5] = 0x3320646e;
        state[10] = 0x79622d32;
        state[15] = 0x6b206574;

        // Key
        for i in 0..8 {
            state[1 + i % 4] = u32::from_le_bytes([
                key[i * 4],
                key[i * 4 + 1],
                key[i * 4 + 2],
                key[i * 4 + 3],
            ]);

            if i >= 4 {
                state[11 + i % 4] = u32::from_le_bytes([
                    key[i * 4],
                    key[i * 4 + 1],
                    key[i * 4 + 2],
                    key[i * 4 + 3],
                ]);
            }
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

## ChaCha20 Variant

Newer CASC versions may use ChaCha20 (Salsa20 variant):

```rust
fn chacha20_quarter_round(a: &mut u32, b: &mut u32, c: &mut u32, d: &mut u32) {
    *a = a.wrapping_add(*b); *d ^= *a; *d = d.rotate_left(16);
    *c = c.wrapping_add(*d); *b ^= *c; *b = b.rotate_left(12);
    *a = a.wrapping_add(*b); *d ^= *a; *d = d.rotate_left(8);
    *c = c.wrapping_add(*d); *b ^= *c; *b = b.rotate_left(7);
}
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

## Implementation Status

### Rust Implementation (cascette-crypto)

Complete Salsa20 implementation with CASC-specific features:

- **Core Salsa20** - Standard 20-round cipher implementation (complete)

- **CASC key handling** - 16-byte key duplication to 32-byte (complete)

- **IV extension** - 4-byte IV expanded to 8-byte with frame index XOR
(complete)

- **Tau constants** - Correct "expand 16-byte k" constants for CASC (complete)

- **Zero-copy processing** - Efficient keystream application (complete)

- **Multi-chunk support** - Frame index modification for BLTE chunks (complete)

**Validation Status:**

- CASC compatibility verified against CascLib behavior

- Integration tests with real WoW encryption keys

- Test suite validates against known BLTE 'E' mode samples

- Zero-allocation keystream generation for performance

### TACT Key Management

The cascette-crypto crate includes hardcoded TACT keys for major WoW expansions:

- Battle for Azeroth, Shadowlands, The War Within, Classic Era

Keys are stored securely with redacted debug output to prevent accidental
logging.

## References

- [Salsa20 Specification](https://cr.yp.to/snuffle/spec.pdf)

- See [BLTE Format](blte.md) for encryption in BLTE blocks

- See [Archives](archives.md) for encrypted content storage

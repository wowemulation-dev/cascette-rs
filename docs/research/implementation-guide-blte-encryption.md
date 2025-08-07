# BLTE Encryption Implementation Guide

## Overview

This guide provides step-by-step instructions for implementing BLTE encryption support in cascette-rs, based on the working implementation found in the prototype.

## Required Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
salsa20 = "0.10"
rc4 = "0.1"
cipher = "0.4"
generic-array = "0.14"
```

## Implementation Structure

### 1. Create New Module: `blte-crypto`

Create a new crate `blte-crypto` in the workspace with the following structure:
```
blte-crypto/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── decrypt.rs
│   ├── salsa20.rs
│   ├── arc4.rs
│   └── key_service.rs
```

## Core Implementation

### Step 1: Define Encryption Types

```rust
// src/lib.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncryptionType {
    Salsa20 = 0x53,  // 'S'
    Arc4 = 0x41,      // 'A'
}

#[derive(Debug, Clone)]
pub struct EncryptedBlock {
    pub key_name: u64,
    pub iv: Vec<u8>,
    pub encryption_type: EncryptionType,
    pub encrypted_data: Vec<u8>,
}
```

### Step 2: Implement Block Parser

```rust
// src/decrypt.rs
pub fn parse_encrypted_block(data: &[u8]) -> Result<EncryptedBlock> {
    ensure!(data.len() >= 26, "Encrypted block too short");
    ensure!(data[0] == 0x45, "Not an encrypted block (expected 0x45)");
    
    let mut offset = 1;
    
    // Read key name size (must be 8)
    let key_name_size = u64::from_le_bytes(data[offset..offset + 8].try_into()?);
    ensure!(key_name_size == 8, "Invalid key name size: {}", key_name_size);
    offset += 8;
    
    // Read key name
    let key_name = u64::from_le_bytes(data[offset..offset + 8].try_into()?);
    offset += 8;
    
    // Read IV size (must be 4)
    let iv_size = u32::from_le_bytes(data[offset..offset + 4].try_into()?);
    ensure!(iv_size == 4, "Invalid IV size: {}", iv_size);
    offset += 4;
    
    // Read IV
    let iv = data[offset..offset + 4].to_vec();
    offset += 4;
    
    // Read encryption type
    let encryption_type = match data[offset] {
        0x53 => EncryptionType::Salsa20,
        0x41 => EncryptionType::Arc4,
        other => bail!("Unknown encryption type: 0x{:02X}", other),
    };
    offset += 1;
    
    Ok(EncryptedBlock {
        key_name,
        iv,
        encryption_type,
        encrypted_data: data[offset..].to_vec(),
    })
}
```

### Step 3: Implement Salsa20 Decryption

```rust
// src/salsa20.rs
use salsa20::cipher::{KeyIvInit, StreamCipher};
use salsa20::{Key, Nonce, Salsa20};

pub fn decrypt_salsa20(
    encrypted_data: &[u8],
    key: &[u8; 16],
    iv: &[u8],
    block_index: usize,
) -> Result<Vec<u8>> {
    // CRITICAL: Extend 16-byte key to 32 bytes by duplication
    let mut salsa20_key = [0u8; 32];
    salsa20_key[0..16].copy_from_slice(key);
    salsa20_key[16..32].copy_from_slice(key);
    
    // CRITICAL: Extend 4-byte IV to 8 bytes by duplication
    let mut extended_iv = [0u8; 8];
    extended_iv[0..4].copy_from_slice(iv);
    extended_iv[4..8].copy_from_slice(iv);
    
    // CRITICAL: XOR block index with first 4 bytes of IV
    for i in 0..4 {
        extended_iv[i] ^= ((block_index >> (i * 8)) & 0xFF) as u8;
    }
    
    // Create cipher and decrypt
    let key = Key::from_slice(&salsa20_key);
    let nonce = Nonce::from_slice(&extended_iv);
    let mut cipher = Salsa20::new(key, nonce);
    
    let mut decrypted = encrypted_data.to_vec();
    cipher.apply_keystream(&mut decrypted);
    
    Ok(decrypted)
}
```

### Step 4: Implement ARC4 Decryption

```rust
// src/arc4.rs
use rc4::{Rc4, KeyInit, StreamCipher};
use generic_array::typenum::U32;

pub fn decrypt_arc4(
    encrypted_data: &[u8],
    key: &[u8; 16],
    iv: &[u8],
    block_index: usize,
) -> Result<Vec<u8>> {
    // CRITICAL: Create combined key = base_key + IV + block_index
    let mut arc4_key = Vec::with_capacity(32);
    
    // Add base key (16 bytes)
    arc4_key.extend_from_slice(key);
    
    // Add IV (4 bytes)
    arc4_key.extend_from_slice(iv);
    
    // Add block index as little-endian bytes (4 bytes)
    arc4_key.extend_from_slice(&(block_index as u32).to_le_bytes());
    
    // CRITICAL: Pad to exactly 32 bytes with zeros
    while arc4_key.len() < 32 {
        arc4_key.push(0);
    }
    
    // Create cipher and decrypt
    let mut cipher: Rc4<U32> = Rc4::new_from_slice(&arc4_key)
        .map_err(|e| anyhow!("Failed to create RC4 cipher: {:?}", e))?;
    
    let mut decrypted = encrypted_data.to_vec();
    cipher.apply_keystream(&mut decrypted);
    
    Ok(decrypted)
}
```

### Step 5: Implement Main Decryption Function

```rust
// src/decrypt.rs
pub fn decrypt_blte_block(
    data: &[u8],
    block_index: usize,
    key_service: &KeyService,
) -> Result<Vec<u8>> {
    let block = parse_encrypted_block(data)?;
    
    // Get encryption key from service
    let key = key_service
        .get_key(block.key_name)
        .ok_or_else(|| anyhow!("Encryption key not found: {:016X}", block.key_name))?;
    
    // Decrypt based on type
    match block.encryption_type {
        EncryptionType::Salsa20 => {
            decrypt_salsa20(&block.encrypted_data, key, &block.iv, block_index)
        }
        EncryptionType::Arc4 => {
            decrypt_arc4(&block.encrypted_data, key, &block.iv, block_index)
        }
    }
}
```

## Integration with BLTE Parser

### Modify Existing BLTE Parser

In the existing BLTE implementation, add support for encrypted blocks:

```rust
// In blte crate
use blte_crypto::{decrypt_blte_block, KeyService};

pub fn decompress_chunk(
    data: &[u8], 
    block_index: usize,
    key_service: Option<&KeyService>
) -> Result<Vec<u8>> {
    match data[0] {
        0x4E => Ok(data[1..].to_vec()),  // 'N' - No compression
        0x5A => decompress_zlib(&data[1..]),  // 'Z' - Zlib
        0x34 => decompress_lz4(&data[1..]),   // '4' - LZ4
        0x46 => {  // 'F' - Frame (recursive BLTE)
            decompress_blte(&data[1..], key_service)
        }
        0x45 => {  // 'E' - Encrypted
            let key_service = key_service
                .ok_or_else(|| anyhow!("Encrypted block but no key service"))?;
            let decrypted = decrypt_blte_block(data, block_index, key_service)?;
            // Recursively decompress the decrypted data
            decompress_chunk(&decrypted, block_index, Some(key_service))
        }
        _ => bail!("Unknown compression type: 0x{:02X}", data[0]),
    }
}
```

## Testing

### Test Vectors

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_salsa20_decryption() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;
        
        // Create test encrypted data
        let plaintext = b"Hello, World!";
        let encrypted = encrypt_salsa20(plaintext, &key, &iv, block_index).unwrap();
        
        // Decrypt and verify
        let decrypted = decrypt_salsa20(&encrypted, &key, &iv, block_index).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_arc4_decryption() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let block_index = 0;
        
        // Similar test for ARC4
        // ...
    }
    
    #[test]
    fn test_block_index_affects_decryption() {
        let key = [0x01u8; 16];
        let iv = [0x02, 0x03, 0x04, 0x05];
        let plaintext = b"Test data";
        
        // Encrypt with different block indices
        let encrypted_0 = encrypt_salsa20(plaintext, &key, &iv, 0).unwrap();
        let encrypted_1 = encrypt_salsa20(plaintext, &key, &iv, 1).unwrap();
        
        // Should produce different ciphertext
        assert_ne!(encrypted_0, encrypted_1);
        
        // But decrypt to same plaintext
        let decrypted_0 = decrypt_salsa20(&encrypted_0, &key, &iv, 0).unwrap();
        let decrypted_1 = decrypt_salsa20(&encrypted_1, &key, &iv, 1).unwrap();
        assert_eq!(decrypted_0, plaintext);
        assert_eq!(decrypted_1, plaintext);
    }
}
```

## Critical Implementation Notes

### Key Extension Mechanisms

1. **Salsa20 Key Extension**:
   - BLTE uses 16-byte keys
   - Salsa20 requires 32-byte keys
   - Solution: Duplicate the 16-byte key

2. **IV Extension**:
   - BLTE provides 4-byte IV
   - Salsa20 needs 8-byte nonce
   - Solution: Duplicate the 4-byte IV

3. **Block Index Integration**:
   - XOR block index with IV for multi-chunk files
   - Ensures different chunks have different keystreams
   - Critical for security

### ARC4 Specifics

1. **Key Construction**:
   - Combine: base_key (16) + IV (4) + block_index (4) = 24 bytes
   - Pad to 32 bytes with zeros
   - Different from standard ARC4 usage

2. **Why 32 Bytes**:
   - Standardizes key size across implementations
   - Matches prototype behavior exactly

## Common Pitfalls

1. **Forgetting Key Extension**: Using 16-byte keys directly will fail
2. **Wrong Byte Order**: Key names and block indices are little-endian
3. **Missing Recursion**: Decrypted data might be compressed
4. **No Key Service**: Must provide keys for encrypted content

## Performance Considerations

1. **Key Caching**: Cache expanded keys to avoid repeated setup
2. **Stream Processing**: Use streaming for large files
3. **Parallel Decryption**: Chunks can be decrypted in parallel

## Validation

Compare output with:
1. Prototype implementation results
2. CascLib decryption output
3. Official game client behavior

## Next Steps

After implementing encryption:
1. Integrate with BLTE parser
2. Add key service (see next guide)
3. Test with real encrypted game files
4. Add benchmarks for performance
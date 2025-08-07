# BLTE Format - Complete Technical Specification

## Overview

BLTE (Block Table Encoded) is Blizzard's proprietary compression and encoding format used throughout TACT/CASC. It supports multiple compression algorithms, encryption, and recursive encoding.

## Binary Structure

### File Header

```c
struct BLTEHeader {
    uint32_t magic;          // 0x45544C42 ("BLTE" in little-endian)
    uint32_t headerSize;     // Big-endian! Size of header after this field
};
```

**Critical Notes**:
- `headerSize` is **big-endian** (unusual for Blizzard formats)
- If `headerSize == 0`: single chunk mode (no chunk table)
- If `headerSize > 0`: multi-chunk mode with chunk table

### Chunk Table (when headerSize > 0)

```c
struct ChunkTable {
    uint8_t flags;           // Table format: 0x0F or 0x10
    uint24_t chunkCount;     // 3-byte chunk count (big-endian)
    ChunkInfo chunks[];      // Array of chunk information
};
```

#### Chunk Info Format 0x0F (Standard)

```c
struct ChunkInfo_0F {
    uint32_t compressedSize;    // Compressed chunk size
    uint32_t decompressedSize;  // Decompressed chunk size
    uint8_t  checksum[16];       // MD5 of compressed data
};
```

#### Chunk Info Format 0x10 (Extended)

```c
struct ChunkInfo_10 {
    uint32_t compressedSize;    // Compressed chunk size
    uint32_t decompressedSize;  // Decompressed chunk size
    uint8_t  compressedHash[16];    // MD5 of compressed data
    uint8_t  decompressedHash[16];  // MD5 of decompressed data
};
```

## Compression Modes

Each chunk starts with a 1-byte mode identifier:

### Mode 'N' (0x4E) - No Compression

```c
struct ModeN {
    uint8_t mode;  // 'N'
    uint8_t data[];  // Raw uncompressed data
};
```

### Mode 'Z' (0x5A) - ZLib Compression

```c
struct ModeZ {
    uint8_t mode;  // 'Z'
    uint8_t zlibData[];  // ZLib compressed data (deflate)
};
```

**ZLib Settings**:
- Window bits: -15 (raw deflate without zlib header)
- Compression level: Variable (typically 6-9)

### Mode '4' (0x34) - LZ4 Compression

```c
struct ModeLZ4 {
    uint8_t mode;  // '4'
    uint32_t decompressedSize;  // Little-endian
    uint32_t compressedSize;    // Little-endian
    uint8_t lz4Data[];           // LZ4HC compressed data
};
```

**LZ4 Notes**:
- Uses LZ4HC (high compression) variant
- Requires explicit size fields

### Mode 'F' (0x46) - Frame/Recursive BLTE

```c
struct ModeF {
    uint8_t mode;  // 'F'
    uint8_t blteData[];  // Another complete BLTE structure
};
```

**Recursive Processing**:
1. Extract inner BLTE data
2. Process as complete BLTE file
3. Return decompressed result

### Mode 'E' (0x45) - Encrypted

```c
struct ModeE {
    uint8_t mode;         // 'E'
    uint8_t keyNameSize;  // Size of key name
    uint8_t keyName[keyNameSize];  // Key identifier
    uint8_t ivSize;       // IV size (≤ 8 bytes)
    uint8_t iv[ivSize];   // Initialization vector
    uint8_t encType;      // Encryption type (S=Salsa20, A=ARC4)
    uint8_t encData[];    // Encrypted BLTE data
};
```

**Encryption Types**:
- `'S'` (0x53): Salsa20
- `'A'` (0x41): ARC4/RC4

## Decompression Algorithm

```python
def decompress_blte(data):
    # Read header
    magic = read_uint32_le(data[0:4])
    assert magic == 0x45544C42  # "BLTE"
    
    header_size = read_uint32_be(data[4:8])
    offset = 8
    
    if header_size == 0:
        # Single chunk
        return decompress_chunk(data[offset:])
    else:
        # Multi-chunk
        flags = data[offset]
        chunk_count = read_uint24_be(data[offset+1:offset+4])
        offset += 4
        
        # Parse chunk table
        chunks = []
        if flags == 0x0F:
            chunk_size = 24
        else:  # 0x10
            chunk_size = 40
            
        for i in range(chunk_count):
            chunk_info = parse_chunk_info(data[offset:offset+chunk_size], flags)
            chunks.append(chunk_info)
            offset += chunk_size
        
        # Decompress chunks
        result = bytearray()
        for chunk in chunks:
            chunk_data = data[offset:offset+chunk.compressed_size]
            
            # Verify checksum
            actual_hash = md5(chunk_data)
            assert actual_hash == chunk.compressed_hash
            
            decompressed = decompress_chunk(chunk_data)
            
            # Verify decompressed size
            assert len(decompressed) == chunk.decompressed_size
            
            result.extend(decompressed)
            offset += chunk.compressed_size
        
        return bytes(result)

def decompress_chunk(data):
    mode = data[0]
    
    if mode == 0x4E:  # 'N'
        return data[1:]
        
    elif mode == 0x5A:  # 'Z'
        return zlib.decompress(data[1:], -15)
        
    elif mode == 0x34:  # '4'
        decompressed_size = read_uint32_le(data[1:5])
        compressed_size = read_uint32_le(data[5:9])
        return lz4.decompress(data[9:9+compressed_size], decompressed_size)
        
    elif mode == 0x46:  # 'F'
        # Recursive BLTE
        return decompress_blte(data[1:])
        
    elif mode == 0x45:  # 'E'
        return decrypt_and_decompress(data[1:])
    
    else:
        raise ValueError(f"Unknown BLTE mode: {mode:02x}")
```

## Encryption Details

### Salsa20 Encryption

```python
def decrypt_salsa20(data, key_name, iv):
    # Get key from key service
    key = get_encryption_key(key_name)
    
    # Expand IV for each chunk
    expanded_iv = bytearray(8)
    expanded_iv[:len(iv)] = iv
    
    # For chunked data, XOR with chunk index
    if is_chunked:
        chunk_index = get_chunk_index()
        for i in range(8):
            expanded_iv[i] ^= (chunk_index >> (i * 8)) & 0xFF
    
    # Create Salsa20 cipher
    cipher = Salsa20(key, expanded_iv, rounds=20)
    
    # Decrypt
    decrypted = cipher.decrypt(data)
    
    # Decrypted data is another BLTE structure
    return decompress_blte(decrypted)
```

### ARC4 Encryption

```python
def decrypt_arc4(data, key_name, iv):
    key = get_encryption_key(key_name)
    
    # Initialize ARC4
    cipher = ARC4(key)
    
    # Apply IV
    cipher.encrypt(iv)  # Discard keystream
    
    # Decrypt
    decrypted = cipher.decrypt(data)
    
    return decompress_blte(decrypted)
```

## ESpec (Encoding Specification)

ESpec strings define how to encode data:

### Format

```
{type[,args]...}:{type[,args]...}:...
```

### Examples

```
n                       # No compression
z                       # ZLib default
z,9                     # ZLib level 9
z,9,{512*1024}          # ZLib level 9, 512KB blocks
e,s,16KB                # Encrypted, Salsa20, 16KB blocks
b:{n}:{z,9}             # Block table: first block uncompressed, second ZLib 9
```

### Common Patterns

```
# Large files with mixed compression
b,{1024*1024}:{n}:{z,9}:{z,9}:{z,9}

# Encrypted sensitive data
e,s,{64*1024}:z,9

# Streaming optimized
b,{256*1024}:{z,6}:{z,6}:{z,6}:{z,6}
```

## Implementation Considerations

### Memory Management

1. **Streaming**: Process chunks sequentially for large files
2. **Buffer Pooling**: Reuse decompression buffers
3. **Lazy Loading**: Decompress chunks on demand

### Error Handling

```python
class BLTEError(Exception):
    pass

class BLTEChecksumError(BLTEError):
    pass

class BLTEDecompressionError(BLTEError):
    pass

class BLTEEncryptionError(BLTEError):
    pass

def safe_decompress(data):
    try:
        # Validate header
        if len(data) < 8:
            raise BLTEError("Truncated BLTE header")
        
        magic = read_uint32_le(data[0:4])
        if magic != 0x45544C42:
            raise BLTEError(f"Invalid BLTE magic: {magic:08x}")
        
        # Process with error recovery
        return decompress_blte(data)
        
    except zlib.error as e:
        raise BLTEDecompressionError(f"ZLib error: {e}")
    except Exception as e:
        raise BLTEError(f"Decompression failed: {e}")
```

### Performance Optimizations

1. **Parallel Decompression**: Process chunks in parallel
2. **Memory Mapping**: Use mmap for large files
3. **Cache Decompressed Data**: LRU cache for frequently accessed files
4. **SIMD Checksums**: Use hardware acceleration for MD5

## Edge Cases

### Zero-Length Files

```python
if header_size == 0 and len(data) == 8:
    # Empty file
    return b""
```

### Corrupted Headers

```python
if header_size > len(data) - 8:
    raise BLTEError("Header size exceeds file size")
```

### Invalid Chunk Counts

```python
if chunk_count == 0:
    raise BLTEError("Zero chunks in multi-chunk mode")

if chunk_count > 100000:  # Sanity check
    raise BLTEError("Excessive chunk count")
```

## Testing Vectors

### Simple Uncompressed

```
Input:  42 4C 54 45 00 00 00 00 4E 48 65 6C 6C 6F
        B  L  T  E  [size=0]   N  H  e  l  l  o
Output: "Hello"
```

### ZLib Compressed

```
Input:  42 4C 54 45 00 00 00 00 5A [zlib data]
        B  L  T  E  [size=0]   Z
Output: [decompressed data]
```

### Multi-Chunk

```
Input:  42 4C 54 45 00 00 00 34 0F 00 00 02 [chunk table] [chunk1] [chunk2]
        B  L  T  E  [size=52]   [2 chunks]
Output: [chunk1 decompressed] + [chunk2 decompressed]
```

## Rust Implementation Guidelines

```rust
use std::io::{Read, Write, Cursor};
use flate2::read::ZlibDecoder;
use lz4::block::decompress;
use md5::{Md5, Digest};

pub struct BLTEDecoder {
    mode: CompressionMode,
    chunks: Vec<ChunkInfo>,
}

impl BLTEDecoder {
    pub fn new(data: &[u8]) -> Result<Self, BLTEError> {
        // Parse header
        let magic = u32::from_le_bytes(data[0..4].try_into()?);
        if magic != 0x45544C42 {
            return Err(BLTEError::InvalidMagic(magic));
        }
        
        let header_size = u32::from_be_bytes(data[4..8].try_into()?);
        
        // Parse based on header size
        if header_size == 0 {
            Ok(Self {
                mode: CompressionMode::Single,
                chunks: vec![],
            })
        } else {
            let chunks = Self::parse_chunks(&data[8..], header_size)?;
            Ok(Self {
                mode: CompressionMode::Multi,
                chunks,
            })
        }
    }
    
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, BLTEError> {
        match self.mode {
            CompressionMode::Single => self.decompress_chunk(data),
            CompressionMode::Multi => self.decompress_multi(data),
        }
    }
}
```
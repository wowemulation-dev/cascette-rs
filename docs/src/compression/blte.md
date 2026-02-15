# BLTE (Block Table Encoded) Format

BLTE is NGDP's container format for compressed and optionally encrypted content.
It provides block-based compression, encryption support, and efficient streaming
capabilities for game data delivery.

## Overview

BLTE files wrap game content with:

- Optional multi-block structure for large files

- Per-block compression (none, zlib, or others)

- Optional encryption (Salsa20 or ARC4)

- MD5 checksums for integrity verification

## Binary Format

### File Structure

```text
BLTE File Layout:
┌─────────────────────────┐
│ BLTE Header (8 bytes)   │
├─────────────────────────┤
│ Extended Header         │ (optional, if header_size > 0)
│ - Flags (1 byte)        │
│ - Chunk Count (3 bytes) │
├─────────────────────────┤
│ Chunk Info Table        │ (24 bytes per chunk)
│ - Compressed Size       │
│ - Decompressed Size     │
│ - MD5 Checksum          │
├─────────────────────────┤
│ Data Block 1            │
│ - Encoding Type (1 byte)│
│ - Compressed Data       │
├─────────────────────────┤
│ Data Block 2            │
│ ...                     │
└─────────────────────────┘
```

### Header Format

```rust
// Primary BLTE header (always 8 bytes)
struct BlteHeader {
    magic: [u8; 4],        // "BLTE" (0x424C5445 in big-endian)
    header_size: u32,      // Big-endian, total header size including these 8 bytes
}
```

#### Header Size Values

- `header_size == 0`: Single chunk file, no extended header

- `header_size > 0`: Multi-chunk file with extended header

### Extended Header

Present only when `header_size > 0`:

```rust
struct ExtendedHeader {
    flags: u8,             // 0x0F = standard, 0x10 = extended
    chunk_count: [u8; 3],  // 24-bit big-endian chunk count
}
```

### Chunk Information Table

#### Standard Format (flags = 0x0F)

Each chunk has a 24-byte entry:

```rust
struct ChunkInfo {
    compressed_size: u32,      // Big-endian
    decompressed_size: u32,    // Big-endian
    checksum: [u8; 16],        // MD5 of compressed chunk data
}
```

#### Extended Format (flags = 0x10)

Each chunk has a 40-byte entry:

```rust
struct ExtendedChunkInfo {
    compressed_size: u32,      // Big-endian
    decompressed_size: u32,    // Big-endian
    checksum: [u8; 16],        // MD5 of compressed chunk data
    decompressed_checksum: [u8; 16], // MD5 of decompressed chunk data
}
```

This extended format provides additional integrity checking with MD5 checksums
of both compressed and decompressed data.

### Formula Validation

For standard chunks (flags = 0x0F):

```text
header_size = 12 + (chunk_count * 24)
```

For extended chunks (flags = 0x10):

```text
header_size = 12 + (chunk_count * 40)
```

Where:

- 12 = 8 (BLTE header) + 1 (flags) + 3 (chunk count)

- 24 = size of standard ChunkInfo entry

- 40 = size of extended ChunkInfo entry

The header_size field includes the 8-byte BLTE header ("BLTE" magic +
header_size u32). Data starts at offset header_size from the beginning of the
file.

## Encoding Types

Each data block starts with a single-byte encoding type:

| Byte | Character | Type | Description |
|------|-----------|------|-------------|
| 0x4E | 'N' | None | Uncompressed data |
| 0x5A | 'Z' | ZLib | ZLib compressed (deflate) |
| 0x34 | '4' | LZ4 | LZ4HC high compression |
| 0x45 | 'E' | Encrypted | Encrypted data block |
| 0x46 | 'F' | Frame | Recursive BLTE (deprecated) |

## Compression Formats

### None (0x4E)

Uncompressed data follows immediately after the encoding byte:

```text
[0x4E] [raw data...]
```

### ZLib (0x5A)

Standard zlib compression:

```text
[0x5A] [2-byte zlib header] [deflate stream...]
```

**Important**: Most implementations skip the zlib header and use raw deflate.

### LZ4 (0x34)

LZ4HC (high compression) format:

```text
[0x34] [decompressed_size:8] [compressed_lz4_data...]
```

- `decompressed_size`: 64-bit little-endian size

- Uses LZ4HC compression with block shift range 5-16

- Provides ~200-300 MB/s decompression speed

## Encryption Format

### Encrypted Block Structure

```text
[0x45] [key_name_size:1] [key_name:8] [iv_size:1] [iv:4] [type:1]
[encrypted_data...]
```

Fields:

- `key_name_size`: Usually 8

- `key_name`: 64-bit key identifier

- `iv_size`: Usually 4

- `iv`: Initialization vector

- `type`: 0x53 ('S') for Salsa20, 0x41 ('A') for ARC4 (legacy, not used in
  TACT 3.13.3+)

### IV Extension and Modification for Chunks

The IV (typically 4 bytes) is zero-padded to 8 bytes for the Salsa20 nonce:

```rust
let mut nonce = [0u8; 8];  // zero-initialized
nonce[..iv_size].copy_from_slice(&iv);
// Remaining bytes stay zero (NOT duplicated)
```

For multi-chunk files, the IV is XORed with the chunk index before extension:

```rust
fn modify_iv(iv: &mut [u8], chunk_index: usize) {
    for i in 0..4 {
        iv[i] ^= ((chunk_index >> (i * 8)) & 0xFF) as u8;
    }
}
```

## Parsing Algorithm

### Step 1: Read BLTE Header

```rust
let magic = read_u32_be();  // Must be 0x424C5445 ("BLTE")
let header_size = read_u32_be();
```

### Step 2: Determine Structure

```rust
if header_size == 0 {
    // Single chunk file
    // Data starts at offset 8
    // Chunk size = file_size - 8 - 1 (encoding byte)
} else {
    // Multi-chunk file
    // Read extended header and chunk table
    // Note: Data offset calculation varies by format!
}
```

The data offset for multi-chunk files is always `header_size` from the start
of the file. The header_size field includes the 8-byte BLTE header.

### Step 3: Read Extended Header (if present)

```rust
let flags = read_u8();  // 0x0F for standard, 0x10 for extended
let chunk_count = read_u24_be();  // 24-bit big-endian

// Read chunk information table
let chunks = Vec::with_capacity(chunk_count);
for _ in 0..chunk_count {
    chunks.push(ChunkInfo {
        compressed_size: read_u32_be(),
        decompressed_size: read_u32_be(),
        checksum: read_bytes(16),
    });
}
```

### Step 4: Process Data Blocks

```rust
let mut output = Vec::new();
let mut offset = header_size;

for chunk_info in chunks {
    // Read chunk data
    let chunk_data = &data[offset..offset + chunk_info.compressed_size];

    // Verify MD5 checksum
    let hash = md5::compute(chunk_data);
    assert_eq!(hash.0, chunk_info.checksum);

    // Decompress based on encoding type
    let decompressed = decompress_chunk(chunk_data);
    output.extend_from_slice(&decompressed);

    offset += chunk_info.compressed_size;
}
```

## Decompression Implementation

```rust
fn decompress_chunk(data: &[u8]) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Err("Empty chunk");
    }

    match data[0] {
        0x4E => {
            // None - return raw data
            Ok(data[1..].to_vec())
        },
        0x5A => {
            // ZLib - decompress using deflate
            // Skip: [0x5A] [78 9C] (zlib header)
            let deflate_data = &data[3..];
            decompress_deflate(deflate_data)
        },
        0x34 => {
            // LZ4 - high compression
            let decompressed_size = u64::from_le_bytes(
                data[1..9].try_into()?
            );
            let compressed_data = &data[9..];
            decompress_lz4(compressed_data, decompressed_size as usize)
        },
        0x45 => {
            // Encrypted - requires key
            decrypt_chunk(&data[1..])
        },
        0x46 => {
            // Frame - recursive BLTE
            let inner_blte = &data[1..];
            parse_blte(inner_blte)
        },
        _ => Err("Unknown encoding type"),
    }
}
```

## Real-World Example

Let's examine the encoding file we fetched earlier:

```text
00000000: 424c 5445 0000 00b4 0f00 0007 0000 0017  BLTE............
          ^^^^^^^^^ ^^^^^^^^^ ^^ ^^^^^^^ ^^^^^^^^^
          Magic     Hdr Size  F  Count   CompSize

Breaking down the header:

- Magic: 0x424C5445 = "BLTE"

- Header Size: 0x000000B4 = 180 bytes

- Flags: 0x0F (required value)

- Chunk Count: 0x000007 = 7 chunks

- First Chunk Compressed Size: 0x00000017 = 23 bytes
```

This indicates:

- Multi-chunk file (header_size > 0)

- 7 chunks total

- Extended header size = 12 + (7 * 24) = 180 bytes

## Performance Characteristics

### Compression Mode Comparison

| Mode | Compression Speed | Decompression Speed | Compression Ratio | Memory Usage |
|------|------------------|---------------------|-------------------|--------------|
| None | ~500 MB/s | ~500 MB/s | 1.0x | Minimal |
| LZ4 | ~200 MB/s | ~300 MB/s | 2-4x | ~64 KB |
| ZLib | ~50-150 MB/s | ~100-200 MB/s | 3-8x | ~256 KB |

### Data Type Recommendations

| Data Type | Recommended Mode | Reasoning |
|-----------|-----------------|-----------|
| Text/Config | ZLib (level 6-9) | High compressibility, access infrequent |
| Textures | LZ4 or None | Often pre-compressed, need fast access |
| Audio | None or LZ4 | Poor compressibility, streaming required |
| Models | ZLib (level 3-6) | Structured data compresses well |
| Temporary | None | Speed critical, short-lived |

## Special Cases

### Headerless Files

When `header_size == 0`:

- Single chunk only

- No chunk information table

- Data starts immediately at offset 8

- Entire remaining file is one compressed block

### Empty Chunks

Some chunks may have:

- `compressed_size == 0`

- `decompressed_size == 0`

- Usually placeholders or removed content

### Large Files

Benefits of multi-chunk structure:

- Parallel decompression

- Streaming installation

- Partial downloads

- Resume capability

## Error Handling

Critical checks:

1. Verify BLTE magic number
2. Validate flags == 0x0F for extended headers
3. Check chunk count > 0 when header_size > 0
4. Verify MD5 checksums match
5. Handle unknown encoding types gracefully
6. Ensure decompressed size matches expected

## Implementation Considerations

### Memory Efficiency

- Stream processing for large files

- Don't load entire file into memory

- Process chunks incrementally

### Performance Optimization

- Parallel chunk decompression

- Cache decompressed data

- Reuse decompression contexts

### Security

- Always verify checksums

- Validate sizes before allocation

- Handle encryption keys securely

- Prevent decompression bombs

## Integration with NGDP

BLTE files in NGDP context:

1. Fetched using encoding keys from CDN
2. May be stored in archives or as loose files
3. Encoding file maps content keys to BLTE-encoded versions
4. Archive indices point to BLTE data within archives

## Debugging Tips

### Identifying BLTE Files

```bash
# Check for BLTE magic
xxd -l 4 file.bin
# Should show: 424c 5445 (BLTE)

# Check header size
xxd -s 4 -l 4 -e file.bin
# Big-endian u32 value
```

### Common Issues

1. **Wrong endianness**: BLTE uses big-endian, not little-endian
2. **Skipping zlib header**: Most implementations skip bytes 1-2 after 0x5A
3. **IV modification**: Remember to XOR IV with chunk index for encryption
4. **Checksum validation**: Use MD5 of compressed data, not decompressed

## Implementation Status

### Rust Implementation (cascette-formats)

Complete BLTE parser and builder with full format support:

- **None (N)** - Uncompressed passthrough (complete)

- **ZLib (Z)** - Deflate compression using flate2 (complete)

- **LZ4 (4)** - LZ4 compression with proper size headers (complete)

- **Encrypted (E)** - Salsa20 and ARC4 encryption with multi-chunk support
(complete)

- **Frame (F)** - Recursive BLTE support (not implemented, deprecated format)

- **Extended Format** - Full support for 0x10 format with dual checksums
(complete)

**Validation Status:**

- Byte-perfect round-trip validation with real WoW files

- Successfully processes encoding, root, install, and download files

- Integration tests with WoW Classic Era production data

- Builder support for creating valid BLTE files programmatically

- Both standard (0x0F) and extended (0x10) chunk formats supported

### Python Tools (cascette-py)

Analysis and decompression tool supports:

- None (N), ZLib (Z), Frame (F) modes

- LZ4 (4) - Analysis only, decompression requires Rust implementation

- Encrypted (E) - Detection and metadata extraction

See <https://github.com/wowemulation-dev/cascette-py> for the Python
implementation.
## References

- BLTE is specific to Blizzard's NGDP system

- Replaces older MPQ compression schemes

- Designed for CDN delivery and streaming installation

- Supports future compression algorithms through encoding type system

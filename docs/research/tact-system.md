# TACT System - Transfer And Content Transfer

## Overview

TACT (Transfer And Content Transfer) is Blizzard's protocol for distributing game content through their CDN infrastructure. It provides content addressing, compression, and integrity verification for game files.

## Content Addressing Scheme

### Three-Layer Addressing

1. **File Data ID** (FDid)
   - Numeric identifier for game files
   - Unique per file within a product
   - Used by game engine for file references

2. **Content Key** (CKey)
   - MD5 hash of uncompressed file content
   - 16 bytes, typically represented as 32-character hex string
   - Ensures content deduplication

3. **Encoding Key** (EKey)
   - MD5 hash of encoded/compressed file
   - Maps to actual CDN storage location
   - Multiple encodings possible for same content

### File Resolution Flow

```
Game Request → File Data ID → Root File → Content Key → Encoding File → Encoding Key → CDN → BLTE Data
```

## System Files

### Build Configuration

**Purpose**: Central registry of system files for a specific build

**Location**: `/config/{hash}`

**Format**: Key-value pairs
```
root = abc123def456...
install = 789012345678...
download = abcdef123456...
encoding = 123456789abc...
size = def456789012...
patch = 345678901234...
patch-config = 567890123456...
```

**Key Fields**:
- `root`: Root file hash (required)
- `install`: Install file hash (required for installation)
- `download`: Download file hash (for updates)
- `encoding`: Encoding file hash and size (required)
- `size`: Size file hash (optional)
- `patch`: Patch file hash (for delta updates)
- `patch-config`: Patch configuration

### CDN Configuration

**Purpose**: Defines archive structure and CDN settings

**Location**: `/config/{hash}`

**Format**: Key-value pairs
```
archives = abc123 def456 789012
archives-index-size = abc123 123456 def456 234567
patch-archives = 345678 901234
patch-archives-index-size = 345678 345678
file-index = 567890123456789
file-index-size = 1234567
```

**Key Fields**:
- `archives`: List of archive hashes
- `archives-index-size`: Archive hash and size pairs
- `patch-archives`: Patch-specific archives
- `file-index`: Alternative file index
- `file-index-size`: File index size

### Encoding File

**Purpose**: Maps content keys to encoding keys with compression metadata

**Structure**:
```c
struct EncodingHeader {
    char magic[2];      // 'EN'
    uint8_t version;    // Version (1)
    uint8_t hashSizeKey;    // CKey size
    uint8_t hashSizeEKey;   // EKey size
    uint16_t pageSizePower; // Page size (2^n KB)
    uint32_t pageCount;     // Number of pages
    uint8_t unk;
};

struct EncodingPage {
    uint8_t firstKey[16];   // First CKey in page
    uint8_t checksum[16];   // Page checksum
};

struct EncodingEntry {
    uint16_t keyCount;      // Number of keys
    uint32_t size;          // Uncompressed size
    uint8_t ckey[16];       // Content key
    uint8_t ekeys[][16];    // Encoding keys
};
```

**Encoding Specifications**:
- Multiple encodings per file supported
- Each encoding has unique EKey
- Supports different compression levels

### Root File

**Purpose**: Maps File Data IDs to content keys with metadata

**Structure Types**:

1. **Content Flags Format** (WoW)
```c
struct RootEntry {
    uint32_t fileDataId;
    uint8_t contenthash[16];  // MD5
    uint32_t namedHash;       // Jenkins hash
    uint8_t localeFlags[4];   // Locale bitmask
    uint8_t contentFlags[4];  // Content type flags
};
```

2. **TV2 Format** (Modern games)
```c
struct TV2RootEntry {
    uint32_t fileDataId;
    uint8_t contenthash[16];
    uint64_t nameHash;        // 64-bit hash
    uint32_t contentFlags;
};
```

**Content Flags**:
```
0x1: Install on Windows
0x2: Install on macOS
0x4: Low violence version
0x8: Install on x86
0x10: Install on x64
0x20: Install on ARM64
0x40: Encrypted
0x80: NoCompression
0x100: Chinese localization
```

**Locale Flags**:
```
enUS = 0x2
koKR = 0x4
frFR = 0x10
deDE = 0x20
zhCN = 0x40
esES = 0x80
zhTW = 0x100
enGB = 0x200
```

### Install File

**Purpose**: Lists files required for game installation

**Structure**:
```c
struct InstallHeader {
    char magic[2];     // 'IN'
    uint8_t version;   // Version (1)
    uint8_t hashSize;  // Hash size (16)
    uint16_t tagCount; // Number of tags
    uint32_t entryCount; // Number of entries
};

struct InstallTag {
    char name[tagNameLength];  // Tag name
    uint16_t type;             // Tag type
};

struct InstallEntry {
    char name[nameLength];     // File path
    uint8_t contentHash[16];   // Content key
    uint32_t size;            // File size
    uint16_t tagBitmask;      // Tags for this file
};
```

**Tag Types**:
- Architecture tags (x86, x64, ARM)
- Language tags (enUS, deDE, etc.)
- Platform tags (Windows, Mac, Linux)

### Download File

**Purpose**: Lists files for game updates and patches

**Structure**: Similar to Install file but focused on:
- Update-specific files
- Priority download order
- Optional content
- Background download candidates

### Size File

**Purpose**: Tracks file sizes for space calculations

**Structure**:
```c
struct SizeEntry {
    uint8_t ekey[16];     // Encoding key
    uint32_t size;        // Compressed size
};
```

## BLTE Encoding

### Overview

BLTE (Block Table Encoded) is TACT's compression and encoding format.

### File Structure

```c
struct BLTEHeader {
    char magic[4];        // 'BLTE'
    uint32_t headerSize;  // Header size
};

// If headerSize > 0
struct BLTEChunkInfo {
    uint8_t flags;        // Encoding mode flags
    uint24_t chunkCount;  // Number of chunks
};

struct BLTEChunk {
    uint32_t compSize;    // Compressed size
    uint32_t decompSize;  // Decompressed size
    uint8_t checksum[16]; // Chunk checksum
};
```

### Encoding Modes

| Mode | Character | Description |
|------|-----------|-------------|
| None | N | Uncompressed data |
| Zlib | Z | Zlib compression |
| Encrypted | E | Salsa20 encryption |
| Frame | F | Recursive BLTE frame |
| ZStd | * | ZStandard compression |

### Compression Process

1. **Single Block Files** (< 256KB)
   - Direct compression
   - Single encoding mode
   - No chunk table

2. **Multi-Block Files** (>= 256KB)
   - Split into chunks
   - Each chunk compressed independently
   - Chunk table in header

### Decompression Algorithm

```python
def decompress_blte(data):
    magic = data[0:4]
    assert magic == b'BLTE'
    
    header_size = read_uint32(data[4:8])
    
    if header_size == 0:
        # Single chunk
        return decompress_chunk(data[8:])
    else:
        # Multiple chunks
        chunks = parse_chunk_table(data[8:8+header_size])
        result = []
        offset = 8 + header_size
        
        for chunk in chunks:
            chunk_data = data[offset:offset+chunk.comp_size]
            result.append(decompress_chunk(chunk_data))
            offset += chunk.comp_size
        
        return b''.join(result)

def decompress_chunk(data):
    mode = data[0]
    
    if mode == ord('N'):
        return data[1:]
    elif mode == ord('Z'):
        return zlib.decompress(data[1:], -15)
    elif mode == ord('E'):
        return decrypt_salsa20(data[1:])
    # ... other modes
```

## Content Distribution

### CDN Path Structure

Files are stored using a hierarchical path based on their hash:

```
http://{cdn_host}/{cdn_path}/{type}/{hash[0:2]}/{hash[2:4]}/{full_hash}
```

Example:
```
Hash: abc123def4567890...
Path: /tpr/wow/data/ab/c1/abc123def4567890...
```

### File Types

| Type | Path | Description |
|------|------|-------------|
| config | /config/ | System configuration files |
| data | /data/ | Game content files |
| patch | /patch/ | Patch files |

### Archive Files

Large collections of files stored together:
- Named by hash (e.g., `abc123def456.index`)
- Contains multiple BLTE-encoded files
- Indexed for efficient access

## Encryption

### Salsa20 Encryption

Used for protecting sensitive content:
- Encryption keys distributed via KeyRing
- Per-file encryption possible
- Transparent decryption on client

### Key Format

Keys are base32-encoded strings:
```
Example: EU-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
```

### Decryption Process

1. Check encryption flag in BLTE header
2. Retrieve key from KeyRing
3. Apply Salsa20 decryption
4. Continue with decompression

## Performance Optimizations

### Chunked Downloads

- Files split into manageable chunks
- Parallel download support
- Resume capability for interrupted transfers

### Compression Efficiency

- Adaptive compression based on file type
- Pre-compressed content detection
- Streaming decompression support

### Caching Strategy

- System files cached locally
- Frequently accessed files prioritized
- LRU eviction for cache management

## Error Handling

### Checksum Verification

Every level has integrity checks:
1. BLTE chunk checksums
2. File content checksums (CKey)
3. Encoded file checksums (EKey)
4. System file checksums

### Recovery Mechanisms

- Automatic retry with exponential backoff
- CDN fallback on failure
- Partial file recovery
- Repair tool support
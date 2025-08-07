# Complete Functionality Map for Cascette-RS

## Overview

This document maps all NGDP/TACT/CASC functionality to specific crates, showing what's implemented (✅), partially implemented (🟡), and not implemented (❌).

## Crate Structure

### Existing Crates

1. **`ngdp-bpsv`** - BPSV (Bar-Separated Values) Parser
2. **`ribbit-client`** - Ribbit Protocol Client
3. **`tact-client`** - TACT HTTP Client
4. **`tact-parser`** - TACT File Format Parser
5. **`ngdp-cdn`** - CDN Content Delivery
6. **`ngdp-cache`** - Caching Layer
7. **`ngdp-client`** - CLI Tool

### Proposed New Crates

8. **`blte`** - BLTE Compression/Decompression (❌ NEEDED)
9. **`ngdp-crypto`** - Encryption/Decryption (❌ NEEDED)
10. **`casc-storage`** - Local CASC Storage (❌ NEEDED)
11. **`ngdp-patch`** - Patch/Update System (❌ NEEDED)

## Detailed Functionality Map

### 1. Network & Protocol Layer

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Ribbit Protocol** |
| TCP Socket Communication | ✅ | `ribbit-client` | `ribbit-client` | |
| V1 Protocol Request/Response | ✅ | `ribbit-client` | `ribbit-client` | |
| PKCS#7/CMS Signature Verification | ✅ | `ribbit-client` | `ribbit-client` | |
| V2 Protocol Support | ❌ | - | `ribbit-client` | Low priority |
| **HTTP/CDN** |
| Basic HTTP Client | ✅ | `tact-client` | `tact-client` | |
| CDN Fallback Mechanism | ✅ | `ngdp-cdn` | `ngdp-cdn` | |
| HTTP Range Requests | ❌ | - | `tact-client` | For partial downloads |
| Resume Support | ❌ | - | `tact-client` | |
| Bandwidth Throttling | ❌ | - | `tact-client` | |
| Parallel Downloads | 🟡 | `ngdp-cdn` | `ngdp-cdn` | Basic implementation |

### 2. Data Format Parsing

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **BPSV Format** |
| BPSV Parsing | ✅ | `ngdp-bpsv` | `ngdp-bpsv` | |
| BPSV Writing | ✅ | `ngdp-bpsv` | `ngdp-bpsv` | |
| **Configuration Files** |
| Build Config Parser | ❌ | - | `tact-parser` | Key-value format |
| CDN Config Parser | ❌ | - | `tact-parser` | Key-value format |
| Product Config Parser | ❌ | - | `tact-parser` | |
| **TACT Manifests** |
| Versions Manifest | ✅ | `tact-client` | `tact-client` | Via BPSV |
| CDNs Manifest | ✅ | `tact-client` | `tact-client` | Via BPSV |
| BGDL Manifest | ✅ | `tact-client` | `tact-client` | Via BPSV |

### 3. TACT File Formats

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Root Files** |
| WoW Root V1 (MFST) | ❌ | - | `tact-parser` | Legacy format |
| WoW Root V2 (TSFM) | ✅ | `tact-parser` | `tact-parser` | Modern format |
| FileDataID → CKey Mapping | ✅ | `tact-parser` | `tact-parser` | |
| Locale/Content Flags | ✅ | `tact-parser` | `tact-parser` | |
| **Encoding File** |
| Header Parsing | ❌ | - | `tact-parser` | Big-endian! |
| CKey → EKey Mapping | ❌ | - | `tact-parser` | |
| EKey → CKey Reverse Lookup | ❌ | - | `tact-parser` | |
| Page Table Parsing | ❌ | - | `tact-parser` | |
| 40-bit Integer Support | ❌ | - | `tact-parser` | Critical |
| **Install Manifest** |
| Header Parsing | ❌ | - | `tact-parser` | |
| Tag System | ❌ | - | `tact-parser` | |
| File Entry Parsing | ❌ | - | `tact-parser` | |
| Bitmask Operations | ❌ | - | `tact-parser` | |
| **Download Manifest** |
| Priority File List | ❌ | - | `tact-parser` | |
| Download Entry Parsing | ❌ | - | `tact-parser` | |
| **Size File** |
| Size Information Parsing | ❌ | - | `tact-parser` | |
| Total Size Calculation | ❌ | - | `tact-parser` | |
| **Patch Files** |
| Patch Manifest Parsing | ❌ | - | `ngdp-patch` | |
| Old → New Mapping | ❌ | - | `ngdp-patch` | |
| **TVFS (Modern Format)** |
| TVFS Header Parsing | ❌ | - | `tact-parser` | |
| Path Table | ❌ | - | `tact-parser` | |
| VFS Table | ❌ | - | `tact-parser` | |
| CFT Table | ❌ | - | `tact-parser` | |
| Directory Structure | ❌ | - | `tact-parser` | |

### 4. BLTE Compression/Decompression

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **BLTE Header Parsing** |
| Magic Number Validation | ❌ | - | `blte` | 'BLTE' |
| Header Size Reading | ❌ | - | `blte` | |
| Chunk Information | ❌ | - | `blte` | |
| **Compression Modes** |
| Mode 'N' (None) | ❌ | - | `blte` | Raw data |
| Mode 'Z' (ZLib) | ❌ | - | `blte` | zlib compression |
| Mode '4' (LZ4) | ❌ | - | `blte` | LZ4HC compression |
| Mode 'F' (Frame) | ❌ | - | `blte` | Recursive BLTE |
| Mode 'E' (Encrypted) | ❌ | - | `blte` | Requires crypto |
| **Multi-Chunk Support** |
| Chunk Table Parsing | ❌ | - | `blte` | |
| Parallel Decompression | ❌ | - | `blte` | |
| Checksum Verification | ❌ | - | `blte` | MD5 per chunk |
| **Streaming Support** |
| Stream Decompression | ❌ | - | `blte` | |
| Progressive Reading | ❌ | - | `blte` | |

### 5. Encryption/Decryption

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Key Management** |
| Key Service | ❌ | - | `ngdp-crypto` | |
| Hardcoded Keys Database | ❌ | - | `ngdp-crypto` | 100+ keys |
| Key File Loading | ❌ | - | `ngdp-crypto` | TactKeys.csv |
| Directory Search | ❌ | - | `ngdp-crypto` | Standard paths |
| Runtime Key Addition | ❌ | - | `ngdp-crypto` | |
| **Encryption Algorithms** |
| Salsa20 Stream Cipher | ❌ | - | `ngdp-crypto` | |
| ARC4/RC4 Stream Cipher | ❌ | - | `ngdp-crypto` | |
| Key Extension (16→32 bytes) | ❌ | - | `ngdp-crypto` | Critical! |
| IV Extension (4→8 bytes) | ❌ | - | `ngdp-crypto` | Critical! |
| Block Index XOR | ❌ | - | `ngdp-crypto` | For chunks |
| **Encrypted Block Parsing** |
| Key Name Extraction | ❌ | - | `ngdp-crypto` | |
| IV Extraction | ❌ | - | `ngdp-crypto` | |
| Encryption Type Detection | ❌ | - | `ngdp-crypto` | |

### 6. Hashing & Checksums

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Hash Algorithms** |
| Jenkins3 (Lookup3) Hash | ✅ | `tact-parser` | `tact-parser` | |
| Path Normalization | ✅ | `tact-parser` | `tact-parser` | |
| MD5 Hashing | 🟡 | Various | `tact-parser` | Via std libs |
| SHA-1 Hashing | 🟡 | Various | `tact-parser` | For signatures |
| SHA-256 Hashing | 🟡 | Various | `tact-parser` | Modern builds |
| **Checksum Operations** |
| File Checksum Verification | ❌ | - | `blte` | |
| Chunk Checksum Verification | ❌ | - | `blte` | |
| Page Checksum Verification | ❌ | - | `tact-parser` | |

### 7. CASC Local Storage

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Index Files** |
| Index V5 Parsing | ❌ | - | `casc-storage` | Legacy |
| Index V7 Parsing | ❌ | - | `casc-storage` | Modern |
| Index V9 Parsing | ❌ | - | `casc-storage` | Latest |
| Bucket-Based Lookup | ❌ | - | `casc-storage` | XOR buckets |
| EKey → Archive Location | ❌ | - | `casc-storage` | |
| **Archive Files** |
| Archive Header Parsing | ❌ | - | `casc-storage` | |
| Archive Entry Reading | ❌ | - | `casc-storage` | |
| Archive Creation | ❌ | - | `casc-storage` | |
| Memory-Mapped Access | ❌ | - | `casc-storage` | Performance |
| **Storage Operations** |
| Read by EKey | ❌ | - | `casc-storage` | |
| Write by EKey | ❌ | - | `casc-storage` | |
| Loose File Support | ❌ | - | `casc-storage` | |
| Storage Verification | ❌ | - | `casc-storage` | |
| Storage Repair | ❌ | - | `casc-storage` | |

### 8. Patch/Update System

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Patch Formats** |
| ZBSDIFF Format | ❌ | - | `ngdp-patch` | Binary diff |
| Patch Application | ❌ | - | `ngdp-patch` | |
| Delta Encoding | ❌ | - | `ngdp-patch` | |
| **Update Process** |
| Version Comparison | ❌ | - | `ngdp-patch` | |
| Patch Download | ❌ | - | `ngdp-patch` | |
| Incremental Updates | ❌ | - | `ngdp-patch` | |
| Rollback Support | ❌ | - | `ngdp-patch` | |

### 9. Caching Layer

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Cache Operations** |
| Memory Cache | ✅ | `ngdp-cache` | `ngdp-cache` | |
| Disk Cache | ✅ | `ngdp-cache` | `ngdp-cache` | |
| Cache Key Generation | ✅ | `ngdp-cache` | `ngdp-cache` | |
| TTL Support | ✅ | `ngdp-cache` | `ngdp-cache` | |
| LRU Eviction | 🟡 | `ngdp-cache` | `ngdp-cache` | Basic impl |
| Cache Statistics | ❌ | - | `ngdp-cache` | |
| Cache Warming | ❌ | - | `ngdp-cache` | |

### 10. CLI Tool

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Commands** |
| Query Versions | ✅ | `ngdp-client` | `ngdp-client` | |
| Query CDNs | ✅ | `ngdp-client` | `ngdp-client` | |
| Download File | ❌ | - | `ngdp-client` | Needs BLTE |
| Extract Archive | ❌ | - | `ngdp-client` | Needs CASC |
| Verify Installation | ❌ | - | `ngdp-client` | |
| Apply Patch | ❌ | - | `ngdp-client` | |
| **Options** |
| Product Selection | ✅ | `ngdp-client` | `ngdp-client` | |
| Region Selection | ✅ | `ngdp-client` | `ngdp-client` | |
| Output Format | ✅ | `ngdp-client` | `ngdp-client` | |
| Verbose Logging | ✅ | `ngdp-client` | `ngdp-client` | |

### 11. Utility Functions

| Functionality | Status | Current Crate | Should Be In | Notes |
|--------------|--------|---------------|--------------|-------|
| **Binary Operations** |
| 40-bit Integer Reading | ❌ | - | `tact-parser` | TACT specific |
| Variable-Length Integers | ❌ | - | `tact-parser` | Varint encoding |
| Big-Endian Reading | 🟡 | Various | `tact-parser` | Via byteorder |
| Little-Endian Reading | 🟡 | Various | `tact-parser` | Via byteorder |
| **String Operations** |
| C-String Reading | ❌ | - | `tact-parser` | Null-terminated |
| Path Normalization | ✅ | `tact-parser` | `tact-parser` | |
| Hex Encoding/Decoding | ✅ | Various | Various | Via hex crate |

## Implementation Priority

### Phase 1: Critical Foundation (MUST HAVE)
1. **`blte` crate** - Without this, cannot read ANY game files
2. **`ngdp-crypto` crate** - Many files are encrypted
3. **Encoding file parser** in `tact-parser` - Core file mapping
4. **Install manifest parser** in `tact-parser` - Installation files

### Phase 2: Storage Layer (SHOULD HAVE)
1. **`casc-storage` crate** - Local file management
2. **Download manifest parser** in `tact-parser`
3. **Size file parser** in `tact-parser`
4. **Build/CDN config parsers** in `tact-parser`

### Phase 3: Advanced Features (NICE TO HAVE)
1. **`ngdp-patch` crate** - Incremental updates
2. **TVFS support** in `tact-parser` - Modern format
3. **HTTP range requests** in `tact-client`
4. **Advanced caching** in `ngdp-cache`

### Phase 4: Optimizations (FUTURE)
1. Memory-mapped files
2. SIMD optimizations
3. Parallel processing
4. Platform-specific features

## Crate Dependencies

```
ngdp-client (CLI)
├── ribbit-client (Protocol)
├── tact-client (HTTP)
├── ngdp-cdn (CDN)
│   └── tact-client
├── ngdp-cache (Cache)
├── tact-parser (Formats)
│   └── ngdp-bpsv
├── blte (Compression) [NEW]
│   └── ngdp-crypto [NEW]
├── ngdp-crypto (Encryption) [NEW]
└── casc-storage (Storage) [NEW]
    ├── blte
    └── tact-parser

ngdp-patch (Updates) [NEW]
├── blte
└── tact-parser
```

## Testing Requirements

Each crate should have:
1. **Unit tests** - Core functionality
2. **Integration tests** - Cross-crate interaction
3. **Benchmarks** - Performance critical paths
4. **Fuzz tests** - Binary parsers
5. **Doc tests** - Example code

## Resource Requirements

### Development Resources
- Reference implementations (CascLib, TACT.Net, prototype)
- Test data files (encoding, root, install files)
- Known encryption keys
- WowDev Wiki documentation

### Runtime Resources
- Disk space for CASC storage (50-100GB for WoW)
- Memory for caching (configurable)
- Network bandwidth for downloads
- CPU for decompression/decryption

## Success Metrics

### Functional Success
- [ ] Can download and decrypt a known WoW file
- [ ] Can parse all TACT file formats
- [ ] Can manage local CASC storage
- [ ] Can apply incremental patches

### Performance Success
- [ ] Comparable speed to CascLib
- [ ] Efficient memory usage
- [ ] Parallel processing utilized
- [ ] Minimal allocations in hot paths

### Quality Success
- [ ] 80% test coverage
- [ ] Zero unsafe code (or well-documented)
- [ ] Complete API documentation
- [ ] Cross-platform support
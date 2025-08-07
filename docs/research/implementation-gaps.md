# Implementation Gaps in Cascette-RS

## Overview

Based on thorough analysis of reference implementations and specifications, this document identifies critical gaps in the current cascette-rs implementation and provides a roadmap for achieving feature parity with production NGDP systems.

**Last Updated**: 2025-08-06

**Current Status**: The project has successfully implemented the network layer (Ribbit protocol, CDN clients) and partial TACT parsing (WoW root files, Jenkins hash). Critical gaps remain in BLTE decompression, encryption support, and complete file format parsers.

## Critical Missing Components

### 1. BLTE Compression/Decompression ⚠️ **CRITICAL**

**Current Status**: Not implemented in main project
**Reference Available**: COMPLETE implementation in prototype at `/home/danielsreichenbach/Downloads/wow/cascette-rs`

**Required Implementation**:
```rust
// Needed in new crate: blte
pub struct BLTEDecoder {
    pub fn decode(data: &[u8]) -> Result<Vec<u8>, BLTEError>;
    pub fn decode_stream(reader: impl Read) -> Result<impl Read, BLTEError>;
}

pub struct BLTEEncoder {
    pub fn encode(data: &[u8], spec: &EncodingSpec) -> Result<Vec<u8>, BLTEError>;
    pub fn encode_stream(reader: impl Read, spec: &EncodingSpec) -> Result<impl Read, BLTEError>;
}
```

**Missing Features**:
- [ ] Mode 'N' - Uncompressed
- [ ] Mode 'Z' - ZLib compression
- [ ] Mode '4' - LZ4HC compression
- [ ] Mode 'F' - Recursive BLTE (frame mode)
- [ ] Mode 'E' - Encryption support
- [ ] Multi-chunk handling
- [ ] Checksum verification
- [ ] Streaming decompression

**Impact**: Cannot read any game data files

### 2. Encryption Support ⚠️ **CRITICAL**

**Current Status**: Not implemented in main project
**Reference Available**: COMPLETE implementation in prototype with BOTH Salsa20 AND ARC4

**Required Implementation**:
```rust
// Needed in ngdp-crypto crate
pub struct KeyService {
    keys: HashMap<u64, Vec<u8>>,
    pub fn add_key(&mut self, key_name: u64, key_data: Vec<u8>);
    pub fn lookup_key(&self, key_name: u64) -> Option<&[u8]>;
}

pub struct Salsa20Decryptor {
    pub fn decrypt(data: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>, CryptoError>;
}

pub struct Arc4Decryptor {
    pub fn decrypt(data: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>, CryptoError>;
}
```

**Missing Features**:
- [ ] Salsa20 stream cipher
- [ ] ARC4/RC4 stream cipher
- [ ] Key management service
- [ ] KeyRing file parsing
- [ ] Known key database
- [ ] IV expansion for chunks

**Impact**: Cannot access encrypted content (many critical game files)

### 3. CASC Storage Implementation 🔴 **HIGH PRIORITY**

**Current Status**: Planned for v0.2.0

**Required Implementation**:
```rust
// Needed in casc-storage crate
pub struct CascStorage {
    indices: HashMap<u8, Index>,
    archives: Vec<Archive>,
    
    pub fn read(&self, ekey: &[u8]) -> Result<Vec<u8>, StorageError>;
    pub fn write(&mut self, ekey: &[u8], data: &[u8]) -> Result<(), StorageError>;
    pub fn build_indices(&mut self) -> Result<(), StorageError>;
}

pub struct IndexFile {
    version: IndexVersion,
    entries: Vec<IndexEntry>,
    
    pub fn lookup(&self, ekey: &[u8]) -> Option<ArchiveLocation>;
    pub fn add_entry(&mut self, ekey: &[u8], location: ArchiveLocation);
}
```

**Missing Features**:
- [ ] Index file parsing (v5, v7)
- [ ] Archive file reading
- [ ] Bucket-based indexing
- [ ] Memory-mapped file access
- [ ] Loose file support
- [ ] Index generation
- [ ] Archive creation

**Impact**: Cannot store or retrieve game files locally

### 4. Complete File Format Parsers 🟡 **MEDIUM PRIORITY**

**Current Status**: 
- ✅ WoW Root file parser (implemented in `tact-parser`)
- ✅ Jenkins3 hash (implemented in `tact-parser`)
- ❌ Encoding file parser (not implemented)
- ❌ Install manifest parser (not implemented)
- ❌ Download file parser (not implemented)
- ❌ Size file parser (not implemented)
- ❌ Build/CDN config parsers (not implemented)

**Required Implementations**:

#### Encoding File Parser
```rust
pub struct EncodingFile {
    header: EncodingHeader,
    ce_key_pages: Vec<CEKeyPage>,
    ekey_spec_pages: Vec<EKeySpecPage>,
    espec_strings: Vec<String>,
    
    pub fn lookup_by_ckey(&self, ckey: &[u8]) -> Option<Vec<[u8; 16]>>;
    pub fn lookup_by_ekey(&self, ekey: &[u8]) -> Option<EncodingInfo>;
}
```

#### Install File Parser
```rust
pub struct InstallFile {
    tags: Vec<InstallTag>,
    entries: Vec<InstallEntry>,
    
    pub fn get_files_for_tags(&self, tags: &[TagType]) -> Vec<FileInfo>;
}
```

#### Download File Parser
```rust
pub struct DownloadFile {
    entries: Vec<DownloadEntry>,
    
    pub fn get_priority_files(&self) -> Vec<FileInfo>;
}
```

#### Size File Parser
```rust
pub struct SizeFile {
    entries: HashMap<[u8; 16], FileSizeInfo>,
    
    pub fn get_total_size(&self) -> u64;
}
```

**Impact**: Limited ability to understand game file organization

### 5. TVFS Support 🟡 **MEDIUM PRIORITY**

**Current Status**: Not implemented

**Required Implementation**:
```rust
pub struct TVFSManifest {
    header: TVFSHeader,
    path_table: PathTable,
    vfs_table: VFSTable,
    
    pub fn resolve_path(&self, path: &str) -> Option<FileInfo>;
    pub fn list_directory(&self, path: &str) -> Vec<DirEntry>;
}
```

**Impact**: Cannot use modern manifest format (required for newer games)

### 6. Patch System 🟡 **MEDIUM PRIORITY**

**Current Status**: Not implemented

**Required Implementation**:
```rust
pub struct PatchFile {
    entries: Vec<PatchEntry>,
    
    pub fn get_patch(&self, old_ekey: &[u8], new_ekey: &[u8]) -> Option<PatchInfo>;
}

pub struct ZBSDIFFPatcher {
    pub fn apply_patch(old_data: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchError>;
}
```

**Impact**: Cannot perform incremental updates

## Algorithm Implementations Missing

### 1. Jenkins Hash (Lookup3) ✅ **IMPLEMENTED**

**Current Status**: Fully implemented in `tact-parser/src/jenkins3.rs`

**Implemented**:
```rust
// Available in tact-parser crate
pub fn jenkins3_hash(data: &[u8]) -> u64
pub fn normalize_path(path: &str) -> String
```

**Impact**: Can resolve file names in root files

### 2. XOR Bucket Calculation 🟡 **Not yet implemented**

**Required**:
```rust
pub fn get_bucket_index(ekey: &[u8]) -> u8 {
    ekey.iter().fold(0u8, |acc, &byte| acc ^ byte) & 0x0F
}
```

**Impact**: Needed for CASC index lookups

### 3. Variable-Length Integer Encoding 🟡

**Required**:
```rust
pub fn encode_varint(value: u64) -> Vec<u8>;
pub fn decode_varint(data: &[u8]) -> (u64, usize);
```

## Network and Protocol Gaps

### 1. Ribbit Protocol ✅ **IMPLEMENTED**

**Current Status**: 
- ✅ V1 protocol fully implemented
- ✅ TCP socket communication
- ✅ Request/response parsing
- ✅ Signature verification (PKCS#7/CMS)
- 🟡 V2 protocol not implemented (low priority)

### 2. CDN Client ✅ **IMPLEMENTED**

**Current Status**:
- ✅ Basic HTTP client (`tact-client`)
- ✅ CDN fallback support (`ngdp-cdn`)
- ✅ BPSV manifest parsing (`ngdp-bpsv`)
- ✅ Caching layer (`ngdp-cache`)
- 🟡 Missing HTTP range requests
- 🟡 Missing partial file downloads
- 🟡 Missing resume support
- 🟡 Missing bandwidth throttling
- 🟡 Missing peer-to-peer support

### 3. Certificate Validation ✅ **PARTIAL**

**Current Status**:
- ✅ Basic certificate validation
- 🟡 Missing OCSP checking
- 🟡 Missing full certificate chain validation
- 🟡 Missing CRL support

## Performance Optimizations Missing

### 1. Memory-Mapped Files ⚠️ **CRITICAL for large files**

```rust
use memmap2::MmapOptions;

pub struct MappedFile {
    mmap: Mmap,
    
    pub fn open(path: &Path) -> Result<Self, IoError>;
    pub fn read_at(&self, offset: usize, len: usize) -> &[u8];
}
```

### 2. SIMD Optimizations 🟢

- [ ] SIMD MD5 hashing
- [ ] SIMD Jenkins hash
- [ ] SIMD XOR operations

### 3. Parallel Processing 🟡

```rust
use rayon::prelude::*;

pub fn decompress_chunks_parallel(chunks: &[ChunkInfo]) -> Vec<Vec<u8>> {
    chunks.par_iter()
        .map(|chunk| decompress_chunk(chunk))
        .collect()
}
```

## Platform-Specific Features

### 1. Windows-Specific 🟢

- [ ] Named pipes for IPC
- [ ] Windows certificate store
- [ ] NTFS sparse file support

### 2. macOS-Specific 🟢

- [ ] Keychain integration
- [ ] APFS clone support
- [ ] Notarization checks

### 3. Linux-Specific 🟢

- [ ] inotify for file watching
- [ ] splice() for zero-copy

## Testing Infrastructure Gaps

### 1. Test Data Generation

```rust
pub struct TestDataGenerator {
    pub fn generate_blte(size: usize, mode: CompressionMode) -> Vec<u8>;
    pub fn generate_encoding_file(entries: usize) -> Vec<u8>;
    pub fn generate_root_file(version: u32, entries: usize) -> Vec<u8>;
}
```

### 2. Fuzzing Harnesses

```rust
#[cfg(fuzzing)]
pub fn fuzz_blte_decoder(data: &[u8]);
pub fn fuzz_encoding_parser(data: &[u8]);
pub fn fuzz_jenkins_hash(data: &[u8]);
```

### 3. Performance Benchmarks

```rust
#[bench]
fn bench_blte_decompress(b: &mut Bencher);
fn bench_jenkins_hash(b: &mut Bencher);
fn bench_index_lookup(b: &mut Bencher);
```

## Implementation Roadmap

### Phase 1: Critical Foundation (v0.2.0)
**Timeline**: 1-2 months

1. **BLTE Implementation** (2 weeks)
   - Basic decompression
   - All compression modes
   - Checksum verification

2. **Encryption Support** (1 week)
   - Salsa20 implementation
   - Key service
   - Basic key management

3. **File Format Parsers** (2 weeks)
   - Encoding file complete
   - Install file parser
   - Download file parser

4. **Jenkins Hash** (3 days)
   - Complete implementation
   - Path normalization

### Phase 2: Storage Layer (v0.3.0)
**Timeline**: 1-2 months

1. **CASC Storage** (3 weeks)
   - Index file parsing
   - Archive reading
   - Basic storage operations

2. **TVFS Support** (1 week)
   - Manifest parsing
   - Path resolution

3. **Memory Optimizations** (1 week)
   - Memory-mapped files
   - Streaming operations

### Phase 3: Advanced Features (v0.4.0)
**Timeline**: 1 month

1. **Patch System** (2 weeks)
   - ZBSDIFF implementation
   - Patch application

2. **Performance** (1 week)
   - Parallel processing
   - SIMD optimizations

3. **Advanced CDN** (1 week)
   - Range requests
   - Resume support

### Phase 4: Production Ready (v1.0.0)
**Timeline**: 1 month

1. **Comprehensive Testing**
2. **Documentation**
3. **Platform-specific features**
4. **Security audit**

## Risk Assessment

### High Risk Items
- **BLTE Implementation**: Core functionality blocker
- **Encryption**: Required for many files
- **Jenkins Hash**: Required for name resolution

### Medium Risk Items
- **CASC Storage**: Can work around with CDN-only
- **File Parsers**: Can implement incrementally

### Low Risk Items
- **TVFS**: Only needed for newest games
- **Optimizations**: Can add later
- **Platform features**: Not critical for MVP

## Recommended Next Steps

1. **Immediate Priority**: Implement BLTE decompression
2. **Second Priority**: Add encryption support
3. **Third Priority**: Complete file format parsers
4. **Fourth Priority**: Implement CASC storage

## Testing Strategy

### Unit Tests Needed
```rust
#[test]
fn test_blte_mode_n();
fn test_blte_mode_z();
fn test_blte_mode_4();
fn test_blte_mode_f();
fn test_blte_mode_e();
fn test_jenkins_hash_known_values();
fn test_salsa20_test_vectors();
```

### Integration Tests Needed
```rust
#[test]
fn test_full_file_download_and_decode();
fn test_encoding_lookup_chain();
fn test_casc_storage_roundtrip();
```

### Real-World Test Cases
- Download and extract a known WoW file
- Verify against reference implementation output
- Performance comparison with CascLib

## Documentation Needed

1. **API Documentation**: All public interfaces
2. **Implementation Guide**: How components interact
3. **File Format Specs**: Binary layouts
4. **Usage Examples**: Common operations
5. **Migration Guide**: From other implementations

## Current Implementation Summary

### ✅ Completed Components
1. **Ribbit Protocol Client** (`ribbit-client`)
   - TCP communication
   - Request/response parsing
   - PKCS#7/CMS signature verification
   
2. **CDN Infrastructure** 
   - TACT HTTP client (`tact-client`)
   - CDN fallback mechanism (`ngdp-cdn`)
   - BPSV manifest parser (`ngdp-bpsv`)
   - Caching layer (`ngdp-cache`)
   
3. **Partial TACT Parser** (`tact-parser`)
   - WoW root file parsing
   - Jenkins3 hash implementation
   
4. **CLI Tool** (`ngdp-client`)
   - Basic command-line interface

### ❌ Critical Missing Components
1. **BLTE Compression/Decompression** - Cannot read any game files
2. **Encryption Support** - Cannot decrypt protected content
3. **Complete File Parsers** - Cannot understand file organization
4. **CASC Storage** - Cannot store/retrieve files locally

### 🟡 Medium Priority Gaps
1. **TVFS Support** - Modern manifest format
2. **Patch System** - Incremental updates
3. **Advanced CDN Features** - Range requests, resume
4. **Performance Optimizations** - Memory mapping, SIMD

## Conclusion

The cascette-rs project has successfully implemented the network and protocol layers of NGDP but lacks critical components for actual game file handling. The most urgent priorities are:

1. **BLTE decompression** - Cannot read any game files without this
2. **Encryption support** - Many files are encrypted
3. **Complete file parsers** - Need to understand file organization
4. **CASC storage** - Local file management

With these components implemented, cascette-rs would achieve functional parity with reference implementations and be suitable for production use in WoW emulation projects.

**Note**: Complete implementation guides for the missing components are available in:
- `implementation-guide-blte-encryption.md`
- `implementation-guide-key-service.md`
- `implementation-guide-file-parsers.md`
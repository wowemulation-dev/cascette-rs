# Implementation TODO for Cascette-RS

## Overview

This document provides a comprehensive, prioritized list of all missing functionality in cascette-rs. Tasks are organized by priority, with existing crates completed first, followed by new crates in dependency order.

**Legend:**
- 🔴 **CRITICAL** - Blocks core functionality
- 🟡 **HIGH** - Important for production use  
- 🟢 **MEDIUM** - Nice to have
- 🔵 **LOW** - Future enhancement

---

## Priority 1: Complete Existing Crates

### 1.1 `tact-parser` - Complete File Format Support 🔴

#### 1.1.1 Add 40-bit Integer Support ✅
**Location:** `tact-parser/src/utils.rs` ✅
```rust
pub fn read_uint40(data: &[u8]) -> u64
pub fn write_uint40(value: u64) -> [u8; 5]
```
**Implementation:**
- [x] Create utils module for binary operations
- [x] Implement 40-bit integer reading (little-endian)
- [x] Implement 40-bit integer writing
- [x] Add tests with known values
**Testing:** Unit tests with test vectors from reference implementations ✅
**Acceptance:** Can read/write 40-bit integers matching CascLib output ✅

#### 1.1.2 Encoding File Parser ✅
**Location:** `tact-parser/src/encoding.rs` ✅
```rust
pub struct EncodingFile {
    header: EncodingHeader,
    ckey_entries: HashMap<Vec<u8>, EncodingEntry>,
    ekey_to_ckey: HashMap<Vec<u8>, Vec<u8>>,
}
```
**Implementation:**
- [x] Define header structure (BIG-ENDIAN values!)
- [x] Implement page table parsing
- [x] Parse CEKey pages (CKey → EKey mapping)
- [x] Parse EKey spec pages (EKey → CKey reverse)
- [x] Handle 40-bit file sizes
- [x] Add checksum verification for pages
- [x] Implement lookup methods:
  - [x] `lookup_by_ckey(&[u8]) -> Option<EncodingEntry>`
  - [x] `lookup_by_ekey(&[u8]) -> Option<&Vec<u8>>`
  - [x] `get_ekey_for_ckey(&[u8]) -> Option<&Vec<u8>>`
  - [x] `get_file_size(&[u8]) -> Option<u64>`
**Dependencies:** 40-bit integer support ✅
**Testing:** Parse test encoding file, verify known mappings ✅
**Acceptance:** Can parse encoding files and perform bidirectional lookups ✅

#### 1.1.3 Install Manifest Parser ✅
**Location:** `tact-parser/src/install.rs` ✅
```rust
pub struct InstallManifest {
    version: u8,
    tags: Vec<InstallTag>,
    entries: Vec<InstallEntry>,
}
```
**Implementation:**
- [x] Parse header with magic "IN" validation
- [x] Implement tag system with bitmasks
- [x] Parse file entries with paths and CKeys
- [x] Calculate bits per tag: `(num_entries + 7) / 8`
- [x] Resolve tags for each file entry
- [x] Add filtering methods:
  - [x] `get_files_for_tags(&[Tag]) -> Vec<FileInfo>`
  - [x] `get_files_for_platform(Platform) -> Vec<FileInfo>`
**Testing:** Parse install manifest, verify tag assignments ✅
**Acceptance:** Can extract platform-specific file lists ✅

#### 1.1.4 Download Manifest Parser 🟡
**Location:** `tact-parser/src/download.rs` (new file)
```rust
pub struct DownloadManifest {
    entries: Vec<DownloadEntry>,
}
```
**Implementation:**
- [ ] Parse download priority entries
- [ ] Extract file CKeys and sizes
- [ ] Implement priority sorting
- [ ] Add method: `get_priority_files() -> Vec<FileInfo>`
**Testing:** Parse download manifest, verify priority order
**Acceptance:** Can identify high-priority download files

#### 1.1.5 Build/CDN Config Parser ✅
**Location:** `tact-parser/src/config.rs` ✅
```rust
pub struct BuildConfig {
    config: ConfigFile,
}

pub struct ConfigFile {
    values: HashMap<String, String>,
    hash_pairs: HashMap<String, HashSizePair>,
}
```
**Implementation:**
- [x] Parse key-value format with " = " separator (and empty values "key =")
- [x] Handle comments (lines starting with #)
- [x] Parse hash-size pairs (e.g., "encoding = abc123 456789")
- [x] Add lookup methods:
  - [x] `get_value(&str) -> Option<&str>`
  - [x] `get_hash(&str) -> Option<&str>`
  - [x] `get_size(&str) -> Option<u64>`
  - [x] `get_hash_pair(&str) -> Option<&HashSizePair>`
- [x] Add BuildConfig helper methods:
  - [x] `root_hash()`, `encoding_hash()`, `install_hash()`, `download_hash()`, `size_hash()`
  - [x] `build_name()` for human-readable version strings
**Testing:** Parse build/CDN configs, verify known keys ✅
**Acceptance:** Can extract encoding, root, install hashes ✅

#### 1.1.6 Size File Parser 🟢
**Location:** `tact-parser/src/size.rs` (new file)
```rust
pub struct SizeFile {
    entries: HashMap<[u8; 16], SizeInfo>,
}
```
**Implementation:**
- [ ] Parse size entries (CKey → size mapping)
- [ ] Calculate total installation size
- [ ] Add methods:
  - [ ] `get_file_size(&[u8]) -> Option<u64>`
  - [ ] `get_total_size() -> u64`
**Testing:** Parse size file, verify total calculation
**Acceptance:** Can determine installation size requirements

#### 1.1.7 TVFS Parser 🟢
**Location:** `tact-parser/src/tvfs.rs` (new file)
```rust
pub struct TVFSManifest {
    header: TVFSHeader,
    path_table: Vec<PathEntry>,
    vfs_table: Vec<VFSEntry>,
    cft_table: Vec<CFTEntry>,
}
```
**Implementation:**
- [ ] Parse TVFS header with magic validation
- [ ] Read 40-bit offsets and sizes
- [ ] Parse path table (file paths)
- [ ] Parse VFS table (virtual filesystem)
- [ ] Parse CFT table (content file table)
- [ ] Implement path resolution:
  - [ ] `resolve_path(&str) -> Option<FileInfo>`
  - [ ] `list_directory(&str) -> Vec<DirEntry>`
**Dependencies:** 40-bit integer support
**Testing:** Parse TVFS manifest, verify directory structure
**Acceptance:** Can navigate modern manifest format

#### 1.1.8 Add Variable-Length Integer Support 🟡
**Location:** `tact-parser/src/utils.rs`
```rust
pub fn read_varint(data: &[u8]) -> (u32, usize)
pub fn write_varint(value: u32) -> Vec<u8>
```
**Implementation:**
- [ ] Implement 7-bit encoding with continuation bit
- [ ] Handle up to 5 bytes (35 bits max)
- [ ] Add boundary checking
**Testing:** Round-trip encoding/decoding tests
**Acceptance:** Matches protobuf varint implementation

---

### 1.2 `ngdp-cache` - Enhanced Caching 🟢

#### 1.2.1 Cache Statistics 🟢
**Location:** `ngdp-cache/src/stats.rs` (new file)
```rust
pub struct CacheStats {
    hits: u64,
    misses: u64,
    evictions: u64,
    bytes_saved: u64,
}
```
**Implementation:**
- [ ] Track cache hit/miss ratio
- [ ] Monitor bandwidth saved
- [ ] Track eviction count
- [ ] Add reporting methods
**Testing:** Verify statistics accuracy
**Acceptance:** Can report cache effectiveness

#### 1.2.2 Improved LRU Eviction 🟢
**Location:** `ngdp-cache/src/lib.rs`
**Implementation:**
- [ ] Implement proper LRU with access tracking
- [ ] Add configurable cache size limits
- [ ] Implement cache warming from file list
**Testing:** Verify LRU order under memory pressure
**Acceptance:** Evicts least recently used items correctly

---

### 1.3 `tact-client` - HTTP Enhancements 🟡

#### 1.3.1 HTTP Range Requests 🟡
**Location:** `tact-client/src/client.rs`
**Implementation:**
- [ ] Add Range header support
- [ ] Handle 206 Partial Content responses
- [ ] Implement chunked downloading
- [ ] Add method: `download_range(url, start, end) -> Result<Vec<u8>>`
**Testing:** Download partial file, verify content
**Acceptance:** Can download file segments

#### 1.3.2 Resume Support 🟡
**Location:** `tact-client/src/resumable.rs` (new file)
**Implementation:**
- [ ] Track download progress
- [ ] Persist partial downloads
- [ ] Resume from last byte
- [ ] Verify partial content integrity
**Testing:** Interrupt and resume download
**Acceptance:** Can resume interrupted downloads

---

## Priority 2: Foundation Crates (New)

### 2.1 `ngdp-crypto` - Encryption Support 🔴

#### 2.1.1 Create Crate Structure 🔴
**Location:** `ngdp-crypto/` (new crate)
```toml
[package]
name = "ngdp-crypto"

[dependencies]
salsa20 = "0.10"
rc4 = "0.1"
cipher = "0.4"
hex = "0.4"
dirs = "5.0"
```
**Implementation:**
- [ ] Create new crate in workspace
- [ ] Add to workspace Cargo.toml
- [ ] Create module structure:
  - [ ] `src/lib.rs` - Public API
  - [ ] `src/key_service.rs` - Key management
  - [ ] `src/salsa20.rs` - Salsa20 cipher
  - [ ] `src/arc4.rs` - ARC4 cipher
  - [ ] `src/keys.rs` - Hardcoded keys

#### 2.1.2 Key Service Implementation 🔴
**Location:** `ngdp-crypto/src/key_service.rs`
```rust
pub struct KeyService {
    keys: HashMap<u64, [u8; 16]>,
}
```
**Implementation:**
- [ ] Add 100+ hardcoded WoW keys (from CascLib)
- [ ] Implement key file loading (multiple formats):
  - [ ] CSV format: "keyname,keyhex"
  - [ ] TXT format: "keyname keyhex description"
  - [ ] TSV format: "keyname\tkeyhex"
- [ ] Search standard directories:
  - [ ] `~/.config/cascette/`
  - [ ] `~/.tactkeys/`
  - [ ] Environment variable: `CASCETTE_KEYS_PATH`
- [ ] Add methods:
  - [ ] `get_key(u64) -> Option<&[u8; 16]>`
  - [ ] `add_key(u64, [u8; 16])`
  - [ ] `load_key_file(&Path) -> Result<usize>`
**Testing:** Load test keys, verify lookup
**Acceptance:** Can manage 100+ encryption keys

#### 2.1.3 Salsa20 Implementation 🔴
**Location:** `ngdp-crypto/src/salsa20.rs`
```rust
pub fn decrypt_salsa20(data: &[u8], key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Vec<u8>>
```
**Implementation:**
- [ ] Extend 16-byte key to 32 bytes (duplicate)
- [ ] Extend 4-byte IV to 8 bytes (duplicate)
- [ ] XOR block index with IV first 4 bytes
- [ ] Apply Salsa20 stream cipher
**Critical:** Must match prototype's key extension exactly!
**Testing:** Decrypt known encrypted blocks
**Acceptance:** Output matches CascLib decryption

#### 2.1.4 ARC4 Implementation 🔴
**Location:** `ngdp-crypto/src/arc4.rs`
```rust
pub fn decrypt_arc4(data: &[u8], key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Vec<u8>>
```
**Implementation:**
- [ ] Combine: key (16) + IV (4) + block_index (4)
- [ ] Pad to 32 bytes with zeros
- [ ] Apply RC4 stream cipher
**Critical:** Only prototype has working ARC4!
**Testing:** Decrypt ARC4 encrypted blocks
**Acceptance:** Matches prototype output

---

### 2.2 `blte` - BLTE Compression/Decompression 🔴

#### 2.2.1 Create Crate Structure 🔴
**Location:** `blte/` (new crate)
```toml
[package]
name = "blte"

[dependencies]
flate2 = "1.0"  # For zlib
lz4 = "1.0"     # For LZ4
ngdp-crypto = { path = "../ngdp-crypto" }
```
**Implementation:**
- [ ] Create new crate in workspace
- [ ] Create module structure:
  - [ ] `src/lib.rs` - Public API
  - [ ] `src/header.rs` - BLTE header parsing
  - [ ] `src/decompress.rs` - Decompression logic
  - [ ] `src/compress.rs` - Compression logic (future)
  - [ ] `src/chunk.rs` - Chunk handling

#### 2.2.2 BLTE Header Parser 🔴
**Location:** `blte/src/header.rs`
```rust
pub struct BLTEHeader {
    magic: [u8; 4],  // 'BLTE'
    header_size: u32,
    chunks: Vec<ChunkInfo>,
}

pub struct ChunkInfo {
    compressed_size: u32,
    decompressed_size: u32,
    checksum: [u8; 16],
}
```
**Implementation:**
- [ ] Validate magic bytes "BLTE"
- [ ] Parse header size (big-endian)
- [ ] Detect single vs multi-chunk
- [ ] Parse chunk table if multi-chunk
- [ ] Extract chunk information
**Testing:** Parse various BLTE headers
**Acceptance:** Correctly identifies chunk structure

#### 2.2.3 Decompression Modes 🔴
**Location:** `blte/src/decompress.rs`
```rust
pub fn decompress_chunk(data: &[u8], block_index: usize, key_service: Option<&KeyService>) -> Result<Vec<u8>>
```
**Implementation:**
- [ ] Mode 'N' (0x4E): Return data[1..] unchanged
- [ ] Mode 'Z' (0x5A): Decompress with zlib
- [ ] Mode '4' (0x34): Decompress with LZ4
- [ ] Mode 'F' (0x46): Recursive BLTE decompression
- [ ] Mode 'E' (0x45): Decrypt then decompress:
  - [ ] Parse encrypted block structure
  - [ ] Get key from KeyService
  - [ ] Decrypt based on type (Salsa20/ARC4)
  - [ ] Recursively decompress result
**Dependencies:** ngdp-crypto for mode 'E'
**Testing:** Decompress all mode types
**Acceptance:** Output matches known decompressed files

#### 2.2.4 Multi-Chunk Support 🔴
**Location:** `blte/src/chunk.rs`
```rust
pub fn decompress_multi_chunk(header: &BLTEHeader, data: &[u8], key_service: Option<&KeyService>) -> Result<Vec<u8>>
```
**Implementation:**
- [ ] Iterate through chunks sequentially
- [ ] Decompress each chunk with correct block_index
- [ ] Verify chunk checksums (MD5)
- [ ] Concatenate decompressed chunks
- [ ] Add parallel decompression option
**Testing:** Decompress multi-chunk files
**Acceptance:** Large files decompress correctly

#### 2.2.5 Streaming Support 🟡
**Location:** `blte/src/stream.rs` (new file)
```rust
pub struct BLTEReader<R: Read> {
    reader: R,
    key_service: Option<Arc<KeyService>>,
}
```
**Implementation:**
- [ ] Implement Read trait
- [ ] Stream chunk decompression
- [ ] Minimal memory usage for large files
**Testing:** Stream decompress large file
**Acceptance:** Memory usage stays constant

---

## Priority 3: Storage Layer

### 3.1 `casc-storage` - Local CASC Storage 🔴

#### 3.1.1 Create Crate Structure 🔴
**Location:** `casc-storage/` (new crate)
```toml
[package]
name = "casc-storage"

[dependencies]
blte = { path = "../blte" }
tact-parser = { path = "../tact-parser" }
memmap2 = "0.9"  # For memory-mapped files
```
**Implementation:**
- [ ] Create crate structure:
  - [ ] `src/lib.rs` - Storage API
  - [ ] `src/index.rs` - Index file handling
  - [ ] `src/archive.rs` - Archive file handling
  - [ ] `src/bucket.rs` - Bucket calculations
  - [ ] `src/storage.rs` - Main storage operations

#### 3.1.2 Index File Parsing 🔴
**Location:** `casc-storage/src/index.rs`
```rust
pub enum IndexFile {
    V5(IndexV5),
    V7(IndexV7),
    V9(IndexV9),
}

pub struct IndexEntry {
    ekey: [u8; 9],  // First 9 bytes of EKey
    archive_index: u32,
    archive_offset: u32,
    size: u32,
}
```
**Implementation:**
- [ ] Detect index version from header
- [ ] Parse index V5 (legacy format)
- [ ] Parse index V7 (modern format)
- [ ] Parse index V9 (latest format)
- [ ] Implement bucket-based lookup:
  - [ ] Calculate bucket: `ekey.iter().fold(0, |a, &b| a ^ b) & 0x0F`
  - [ ] Binary search within bucket
- [ ] Memory-map large index files
**Testing:** Parse all index versions, lookup known EKeys
**Acceptance:** Can locate files in archives

#### 3.1.3 Archive File Reading 🔴
**Location:** `casc-storage/src/archive.rs`
```rust
pub struct Archive {
    file: MemoryMappedFile,
    index: u32,
}
```
**Implementation:**
- [ ] Open archive files (data.XXX)
- [ ] Read at specific offsets
- [ ] Extract BLTE data
- [ ] Handle archive header if present
- [ ] Memory-map for performance
**Dependencies:** blte for decompression
**Testing:** Extract known files from archives
**Acceptance:** Can read archive contents

#### 3.1.4 Storage Operations 🔴
**Location:** `casc-storage/src/storage.rs`
```rust
pub struct CascStorage {
    path: PathBuf,
    indices: HashMap<u8, IndexFile>,
    archives: Vec<Archive>,
}
```
**Implementation:**
- [ ] Initialize from game directory
- [ ] Build index from .idx files
- [ ] Implement core operations:
  - [ ] `read_by_ekey(&[u8]) -> Result<Vec<u8>>`
  - [ ] `read_by_ckey(&[u8]) -> Result<Vec<u8>>` (via encoding)
  - [ ] `exists(&[u8]) -> bool`
- [ ] Support loose files (direct file storage)
- [ ] Add write support for new files
**Testing:** Full read/write cycle
**Acceptance:** Can manage local game files

#### 3.1.5 Storage Verification 🟡
**Location:** `casc-storage/src/verify.rs` (new file)
```rust
pub fn verify_storage(storage: &CascStorage) -> VerifyReport
```
**Implementation:**
- [ ] Check all index files
- [ ] Verify archive integrity
- [ ] Report missing/corrupted files
- [ ] Calculate storage statistics
**Testing:** Verify known good/bad storage
**Acceptance:** Detects corruption accurately

---

## Priority 4: Advanced Features

### 4.1 `ngdp-patch` - Patch System 🟡

#### 4.1.1 Create Crate Structure 🟡
**Location:** `ngdp-patch/` (new crate)
```toml
[package]
name = "ngdp-patch"

[dependencies]
blte = { path = "../blte" }
bsdiff = "0.1"  # For patch application
```
**Implementation:**
- [ ] Create crate structure:
  - [ ] `src/lib.rs` - Patch API
  - [ ] `src/zbsdiff.rs` - ZBSDIFF format
  - [ ] `src/apply.rs` - Patch application

#### 4.1.2 Patch File Parser 🟡
**Location:** `ngdp-patch/src/patch.rs`
```rust
pub struct PatchFile {
    entries: Vec<PatchEntry>,
}

pub struct PatchEntry {
    old_ekey: [u8; 16],
    new_ekey: [u8; 16],
    patch_ekey: [u8; 16],
    old_size: u64,
    new_size: u64,
}
```
**Implementation:**
- [ ] Parse patch manifest
- [ ] Extract patch mappings
- [ ] Calculate patch requirements
**Testing:** Parse patch files
**Acceptance:** Can identify needed patches

#### 4.1.3 ZBSDIFF Implementation 🟡
**Location:** `ngdp-patch/src/zbsdiff.rs`
```rust
pub fn apply_patch(old_data: &[u8], patch_data: &[u8]) -> Result<Vec<u8>>
```
**Implementation:**
- [ ] Decompress patch with zlib
- [ ] Apply binary diff algorithm
- [ ] Verify output checksum
**Testing:** Apply known patches
**Acceptance:** Patched files match expected

---

### 4.2 `ngdp-client` - CLI Enhancements 🟡

#### 4.2.0 TACT Parser Integration ✅
**Location:** `ngdp-client/src/commands/` ✅
**Implementation:**
- [x] Added `inspect build-config` command with visual tree display
- [x] Enhanced `products versions` with `--parse-config` flag  
- [x] Real CDN integration for downloading build configurations
- [x] Visual tree representation using emoji and Unicode box-drawing
- [x] Shows meaningful build information instead of cryptic hashes
- [x] Support for all output formats (text, JSON, BPSV)
- [x] File size display with proper units (MB, KB)
- [x] VFS entry counting and patch status indication
**Testing:** Tested with real WoW products (wow, wow_classic_era) ✅
**Acceptance:** Can analyze and display build configurations ✅

---

#### 4.2.1 File Download Command 🟡
**Location:** `ngdp-client/src/commands/download.rs` (new file)
```rust
pub fn download_file(file_id: u32, output: &Path) -> Result<()>
```
**Implementation:**
- [ ] Resolve FileDataID → CKey (via root)
- [ ] Resolve CKey → EKey (via encoding)
- [ ] Download from CDN
- [ ] Decompress with BLTE
- [ ] Save to disk
**Dependencies:** All previous components
**Testing:** Download known file
**Acceptance:** File matches expected content

#### 4.2.2 Installation Command 🟡
**Location:** `ngdp-client/src/commands/install.rs` (new file)
```rust
pub fn install_game(product: &str, path: &Path) -> Result<()>
```
**Implementation:**
- [ ] Query latest version
- [ ] Download manifests
- [ ] Parse install manifest
- [ ] Download required files
- [ ] Build local CASC storage
- [ ] Show progress bar
**Testing:** Install minimal file set
**Acceptance:** Creates valid CASC storage

#### 4.2.3 Verification Command 🟡
**Location:** `ngdp-client/src/commands/verify.rs` (new file)
```rust
pub fn verify_installation(path: &Path) -> Result<VerifyReport>
```
**Implementation:**
- [ ] Check all files against manifests
- [ ] Verify checksums
- [ ] Report missing/corrupted files
- [ ] Suggest repair actions
**Testing:** Verify good/corrupted installation
**Acceptance:** Accurately reports issues

---

## Testing Strategy

### Unit Testing Requirements
Each component MUST have:
- [ ] Basic functionality tests
- [ ] Error condition tests
- [ ] Edge case tests (empty, maximum size, etc.)
- [ ] Known value tests (from reference implementations)

### Integration Testing Requirements
- [ ] Cross-crate integration tests
- [ ] End-to-end file download and decompression
- [ ] Full installation simulation
- [ ] Update/patch application

### Performance Testing
- [ ] Benchmark critical paths
- [ ] Memory usage profiling
- [ ] Parallel processing verification
- [ ] Large file handling (>1GB)

### Test Data Requirements
**Location:** `test-data/` (repository root)
- [ ] Sample encoding file
- [ ] Sample root file (V1 and V2)
- [ ] Sample install manifest
- [ ] Sample BLTE files (all modes)
- [ ] Encrypted test blocks
- [ ] Known key-value pairs

---

## Documentation Requirements

### API Documentation
- [ ] All public types must have doc comments
- [ ] All public methods must have:
  - [ ] Description
  - [ ] Parameters
  - [ ] Return value
  - [ ] Error conditions
  - [ ] Example usage

### Guide Documentation
**Location:** `docs/`
- [ ] Getting Started guide
- [ ] Architecture overview
- [ ] File format specifications
- [ ] Troubleshooting guide
- [ ] Contributing guide

### Example Programs
**Location:** `examples/`
- [ ] Download single file
- [ ] Parse manifest files
- [ ] Verify installation
- [ ] Extract game assets

---

## Milestones

### Milestone 1: Foundation ✅
- [x] Ribbit client
- [x] CDN client
- [x] Basic caching
- [x] CLI skeleton

### Milestone 2: File Formats ✅
- [x] Complete tact-parser core functionality
- [x] Build configuration parser with real CDN integration
- [x] Encoding file parser with 40-bit integer support
- [x] Install manifest parser
- [x] CLI integration with visual tree display
- [ ] Download manifest parser (remaining)
- [ ] Size file parser (remaining)
- [ ] TVFS parser (remaining)

### Milestone 3: Decompression 🔴
- [ ] ngdp-crypto crate
- [ ] blte crate
- [ ] Encryption support
- [ ] All compression modes

### Milestone 4: Storage 🔴
- [ ] casc-storage crate
- [ ] Index parsing
- [ ] Archive reading
- [ ] Local file management

### Milestone 5: Production Ready 🔴
- [ ] ngdp-patch crate
- [ ] Complete CLI
- [ ] Full test coverage
- [ ] Performance optimization

### Milestone 6: Release 🔴
- [ ] Documentation complete
- [ ] Cross-platform testing
- [ ] Security audit
- [ ] Version 1.0.0

---

## Success Criteria

### Functional Success
- [ ] Can download any WoW game file
- [ ] Can decrypt encrypted content
- [ ] Can parse all TACT formats
- [ ] Can manage CASC storage
- [ ] Can apply patches

### Performance Success
- [ ] Download speed ≥ 10 MB/s
- [ ] Decompression speed ≥ 100 MB/s
- [ ] Memory usage < 500 MB for normal operations
- [ ] Startup time < 1 second

### Quality Success
- [ ] Test coverage ≥ 80%
- [ ] Zero security vulnerabilities
- [ ] All clippy warnings resolved
- [ ] Documentation coverage 100%

---

## Risk Mitigation

### Technical Risks
1. **Encryption keys unavailable**
   - Mitigation: Maintain comprehensive key database
   - Fallback: Allow user-provided keys

2. **Format changes in new versions**
   - Mitigation: Version detection and branching
   - Fallback: Support multiple format versions

3. **Performance bottlenecks**
   - Mitigation: Profile early and often
   - Fallback: Add caching layers

### Project Risks
1. **Scope creep**
   - Mitigation: Strict prioritization
   - Focus: Core functionality first

2. **Dependency issues**
   - Mitigation: Minimal external dependencies
   - Fallback: Implement critical parts internally

---

## Notes for Implementers

### Critical Implementation Details
1. **Encoding file uses BIG-ENDIAN** - Different from most TACT formats!
2. **40-bit integers** - Used throughout TACT, must handle correctly
3. **Key extension** - Salsa20 needs 16→32 byte extension by duplication
4. **Block index XOR** - Critical for multi-chunk encryption
5. **Jenkins hash** - Must normalize paths (uppercase, backslash)

### Reference Implementations
- **Prototype**: `/home/danielsreichenbach/Downloads/wow/cascette-rs` - Has complete BLTE/encryption
- **CascLib**: Best for encryption keys and format variations
- **TACT.Net**: Best for async patterns and structure
- **TACTSharp**: Best for performance optimizations

### Testing Resources
- WowDev Wiki: Format specifications
- CascLib test files: Known good test data
- Prototype tests: Working implementation reference

---

## Quick Start for Contributors

1. **Start with**: Complete `tact-parser` (Priority 1.1)
2. **Then**: Create `ngdp-crypto` (Priority 2.1)
3. **Then**: Create `blte` (Priority 2.2)
4. **Finally**: Create `casc-storage` (Priority 3.1)

Each task is independent within its priority level and can be worked on in parallel by different contributors.

---

*Last Updated: 2025-08-06*
*Version: 1.0.0*
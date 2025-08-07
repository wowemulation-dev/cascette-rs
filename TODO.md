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

#### 1.1.4 Download Manifest Parser ✅

**Location:** `tact-parser/src/download.rs` ✅

```rust
pub struct DownloadManifest {
    header: DownloadHeader,
    entries: HashMap<Vec<u8>, DownloadEntry>,
    priority_order: Vec<Vec<u8>>,
    tags: Vec<DownloadTag>,
}
```

**Implementation:**

- [x] Parse download priority entries
- [x] Extract file EKeys and sizes
- [x] Implement priority sorting
- [x] Support versions 1, 2, and 3
- [x] Parse header with magic "DL" validation
- [x] Implement tag-based filtering
- [x] Add methods:
  - [x] `get_priority_files(limit) -> Vec<(Vec<u8>, u8)>`
  - [x] `get_files_for_tags(&[&str]) -> Vec<Vec<u8>>`
  - [x] `get_entry(&[u8]) -> Option<&DownloadEntry>`
**Testing:** Parse download manifest, verify priority order ✅
**Acceptance:** Can identify high-priority download files ✅

#### 1.1.5 Build/CDN Config Parser ✅

**Location:** `tact-parser/src/config.rs` ✅

```rust
pub struct BuildConfig {
    config: ConfigFile,
}

pub struct ConfigFile {
    values: HashMap<String, String>,
    hashes: HashMap<String, HashPair>,
}
```

**Implementation:**

- [x] Parse key-value format with " = " separator (and empty values "key =")
- [x] Handle comments (lines starting with #)
- [x] Parse hash-size pairs (e.g., "encoding = abc123 456789")
- [x] Support both single-hash and hash-size pair formats
- [x] Add lookup methods:
  - [x] `get_value(&str) -> Option<&str>`
  - [x] `get_hash(&str) -> Option<&str>`
  - [x] `get_size(&str) -> Option<u64>`
  - [x] `get_hash_pair(&str) -> Option<&HashPair>`
- [x] Add BuildConfig helper methods:
  - [x] `root_hash()`, `encoding_hash()`, `install_hash()`, `download_hash()`, `size_hash()`
  - [x] `build_name()` for human-readable version strings
  - [x] `extract_hash()` helper for both format types
**Testing:** Parse build/CDN configs, verify known keys ✅
**Testing with real CDN data:** Tested with wow_classic_era and wow (retail) ✅
**Acceptance:** Can extract encoding, root, install hashes from both formats ✅

#### 1.1.6 Size File Parser ✅

**Location:** `tact-parser/src/size.rs` ✅

```rust
pub struct SizeFile {
    header: SizeHeader,
    entries: HashMap<Vec<u8>, SizeEntry>,
    tags: Vec<SizeTag>,
    size_order: Vec<Vec<u8>>,
    parse_order: Vec<Vec<u8>>,
}
```

**Implementation:**

- [x] Parse size entries (partial EKey → size mapping)
- [x] Parse header with magic "DS" validation
- [x] Support tag-based size filtering
- [x] Calculate total installation size
- [x] Maintain parse order for tag mask application
- [x] Add methods:
  - [x] `get_file_size(&[u8]) -> Option<u32>`
  - [x] `get_total_size() -> u64`
  - [x] `get_size_for_tags(&[&str]) -> u64`
  - [x] `get_largest_files(count) -> Vec<(&Vec<u8>, u32)>`
  - [x] `get_statistics() -> SizeStatistics`
**Testing:** Parse size file, verify total calculation ✅
**Acceptance:** Can determine installation size requirements ✅

#### 1.1.7 TVFS Parser ✅

**Location:** `tact-parser/src/tvfs.rs` ✅

```rust
pub struct TVFSManifest {
    header: TVFSHeader,
    path_table: Vec<PathEntry>,
    vfs_table: Vec<VFSEntry>,
    cft_table: Vec<CFTEntry>,
    espec_table: Option<Vec<String>>,
}
```

**Implementation:**

- [x] Parse TVFS header with magic validation (supports both TVFS and TFVS)
- [x] Complete structure for all tables (path, VFS, CFT, EST)
- [x] Implement path resolution methods
- [x] Directory listing functionality
- [x] File information retrieval
- [x] Correct format based on real data:
  - [x] Supports both TVFS and TFVS magic bytes
  - [x] Uses big-endian byte order
  - [x] Path table uses simple length bytes
  - [x] Header uses int32 values for offsets
- [x] EST (Extended Spec Table) support for additional metadata

**Testing:** Comprehensive tests with real header structure ✅
**Acceptance:** Parser correctly handles real TVFS data ✅

#### 1.1.8 Add Variable-Length Integer Support ✅

**Location:** `tact-parser/src/utils.rs` ✅

```rust
pub fn read_varint(data: &[u8]) -> Result<(u32, usize)>
pub fn write_varint(value: u32) -> Vec<u8>
```

**Implementation:**

- [x] Implement 7-bit encoding with continuation bit
- [x] Handle up to 5 bytes (35 bits max)
- [x] Add boundary checking
- [x] Overflow protection
**Testing:** Round-trip encoding/decoding tests ✅
**Acceptance:** Matches protobuf varint implementation ✅

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

#### 1.3.1 HTTP Range Requests ✅

**Location:** `tact-client/src/http.rs` ✅
**Implementation:**

- [x] Add Range header support
- [x] Handle 206 Partial Content responses  
- [x] Implement chunked downloading
- [x] Add method: `download_file_range(cdn_host, path, hash, range) -> Result<Response>`
- [x] Add multi-range support: `download_file_multirange()`
- [x] Integration with retry logic and error handling
- [x] Example demonstrating range request usage
**Testing:** Unit tests for header formatting, example program ✅
**Acceptance:** Can download file segments ✅

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

### 2.1 `ngdp-crypto` - Encryption Support ✅

#### 2.1.1 Create Crate Structure ✅

**Location:** `ngdp-crypto/` ✅

```toml
[package]
name = "ngdp-crypto"

[dependencies]
salsa20 = "0.10"
cipher = "0.4"
hex = "0.4"
dirs = "6.0"
thiserror = "2.0"
tracing = "0.1"
```

**Implementation:**

- [x] Create new crate in workspace
- [x] Add to workspace Cargo.toml
- [x] Create module structure:
  - [x] `src/lib.rs` - Public API
  - [x] `src/key_service.rs` - Key management
  - [x] `src/salsa20.rs` - Salsa20 cipher
  - [x] `src/arc4.rs` - ARC4 cipher ✅
  - [x] `src/keys.rs` - Hardcoded keys
  - [x] `src/error.rs` - Error types

#### 2.1.2 Key Service Implementation ✅

**Location:** `ngdp-crypto/src/key_service.rs` ✅

```rust
pub struct KeyService {
    keys: HashMap<u64, [u8; 16]>,
}
```

**Implementation:**

- [x] Add initial hardcoded WoW keys (10 keys, more can be added)
- [x] Implement key file loading (multiple formats):
  - [x] CSV format: "keyname,keyhex"
  - [x] TXT format: "keyname keyhex description"
  - [x] TSV format: "keyname\tkeyhex"
- [x] Search standard directories:
  - [x] `~/.config/cascette/`
  - [x] `~/.tactkeys/`
  - [x] Environment variable: `CASCETTE_KEYS_PATH`
- [x] Add methods:
  - [x] `get_key(u64) -> Option<&[u8; 16]>`
  - [x] `add_key(u64, [u8; 16])`
  - [x] `load_key_file(&Path) -> Result<usize>`
  - [x] `load_from_standard_dirs() -> Result<usize>`
**Testing:** Load test keys, verify lookup ✅
**Acceptance:** Can manage encryption keys ✅

#### 2.1.3 Salsa20 Implementation ✅

**Location:** `ngdp-crypto/src/salsa20.rs` ✅

```rust
pub fn decrypt_salsa20(data: &[u8], key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Vec<u8>>
```

**Implementation:**

- [x] Extend 16-byte key to 32 bytes (duplicate)
- [x] Extend 4-byte IV to 8 bytes (duplicate)
- [x] XOR block index with IV first 4 bytes
- [x] Apply Salsa20 stream cipher
- [x] Add symmetric encrypt function
**Critical:** Must match prototype's key extension exactly! ✅
**Testing:** Decrypt known encrypted blocks ✅
**Acceptance:** Round-trip encryption/decryption works ✅

#### 2.1.4 ARC4 Implementation ✅

**Location:** `ngdp-crypto/src/arc4.rs`

```rust
pub fn decrypt_arc4(data: &[u8], key: &[u8; 16], iv: &[u8], block_index: usize) -> Result<Vec<u8>>
```

**Implementation:**

- [x] Combine: key (16) + IV (4) + block_index (4)
- [x] Pad to 32 bytes with zeros
- [x] Apply RC4 stream cipher
**Testing:** Decrypt ARC4 encrypted blocks ✅
**Acceptance:** Matches expected output ✅

---

### 2.2 `blte` - BLTE Compression/Decompression ✅

**CRITICAL:** Testing with real CDN data revealed that all manifest files (download, size, encoding, install) are BLTE-encoded. This crate is required before parsers can work with actual CDN files.

#### 2.2.1 Create Crate Structure ✅

**Location:** `blte/` ✅

```toml
[package]
name = "blte"

[dependencies]
flate2 = "1.0"  # For zlib
lz4-flex = "0.11"  # For LZ4
ngdp-crypto = { path = "../ngdp-crypto" }
```

**Implementation:**

- [x] Create new crate in workspace
- [x] Create module structure:
  - [x] `src/lib.rs` - Public API
  - [x] `src/header.rs` - BLTE header parsing
  - [x] `src/decompress.rs` - Decompression logic
  - [ ] `src/compress.rs` - Compression logic (future)
  - [x] `src/chunk.rs` - Chunk handling

#### 2.2.2 BLTE Header Parser ✅

**Location:** `blte/src/header.rs` ✅

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

- [x] Validate magic bytes "BLTE"
- [x] Parse header size (big-endian)
- [x] Detect single vs multi-chunk
- [x] Parse chunk table if multi-chunk
- [x] Extract chunk information
**Testing:** Parse various BLTE headers ✅
**Acceptance:** Correctly identifies chunk structure ✅

#### 2.2.3 Decompression Modes ✅

**Location:** `blte/src/decompress.rs` ✅

```rust
pub fn decompress_chunk(data: &[u8], block_index: usize, key_service: Option<&KeyService>) -> Result<Vec<u8>>
```

**Implementation:**

- [x] Mode 'N' (0x4E): Return data[1..] unchanged
- [x] Mode 'Z' (0x5A): Decompress with zlib
- [x] Mode '4' (0x34): Decompress with LZ4
- [x] Mode 'F' (0x46): Recursive BLTE decompression
- [x] Mode 'E' (0x45): Decrypt then decompress:
  - [x] Parse encrypted block structure
  - [x] Get key from KeyService
  - [x] Decrypt based on type (Salsa20/ARC4)
  - [x] Recursively decompress result
**Dependencies:** ngdp-crypto for mode 'E' ✅
**Testing:** Decompress all mode types ✅
**Acceptance:** Output matches known decompressed files ✅

#### 2.2.4 Multi-Chunk Support ✅

**Location:** `blte/src/decompress.rs` ✅

```rust
pub fn decompress_multi_chunk(header: &BLTEHeader, data: &[u8], key_service: Option<&KeyService>) -> Result<Vec<u8>>
```

**Implementation:**

- [x] Iterate through chunks sequentially
- [x] Decompress each chunk with correct block_index
- [x] Verify chunk checksums (MD5)
- [x] Concatenate decompressed chunks
- [ ] Add parallel decompression option (future enhancement)
**Testing:** Decompress multi-chunk files ✅
**Acceptance:** Large files decompress correctly ✅

#### 2.2.5 Streaming Support ✅

**Location:** `blte/src/stream.rs` ✅

```rust
pub struct BLTEStream {
    blte_file: BLTEFile,
    current_chunk: usize,
    key_service: Option<KeyService>,
    chunk_buffer: Vec<u8>,
    chunk_position: usize,
}
```

**Implementation:**

- [x] Implement Read trait
- [x] Stream chunk decompression
- [x] Minimal memory usage for large files
- [x] Support for all compression modes (N, Z, 4, F, E)
- [x] Proper checksum verification per chunk
- [x] Example showing streaming usage
**Testing:** Stream decompress single and multi-chunk files ✅
**Acceptance:** Memory usage stays constant ✅

#### 2.2.6 BLTE Compression Support 🟡

**Location:** `blte/src/compress.rs` (new file) 🟡

```rust
pub fn compress_chunk(data: &[u8], mode: CompressionMode, level: Option<u8>) -> Result<Vec<u8>>
pub fn compress_data_single(data: Vec<u8>, mode: CompressionMode, level: Option<u8>) -> Result<Vec<u8>>
```

**Implementation:**

- [ ] **Core Compression Functions**:
  - [ ] Mode 'N' (0x4E): Pass-through with mode byte prefix
  - [ ] Mode 'Z' (0x5A): ZLib compression with configurable levels (1-9)
  - [ ] Mode '4' (0x34): LZ4HC compression with size headers
  - [ ] Mode 'F' (0x46): Recursive BLTE compression support
- [ ] **Compression Level Support**:
  - [ ] ZLib: levels 1-9 (6 default for balance)
  - [ ] LZ4: HC variant for better compression ratios
  - [ ] Auto-selection based on data characteristics
- [ ] **Basic Multi-Chunk Support**:
  - [ ] Split data into chunks for multi-chunk files
  - [ ] Calculate MD5 checksums for each chunk
  - [ ] Simple chunking algorithm (fixed size)
**Dependencies:** flate2, lz4_flex (already available) ✅
**Testing:** Round-trip compress/decompress tests for all modes
**Acceptance:** Compressed files decompress to original data

#### 2.2.7 BLTE Encryption Support 🟡

**Location:** `blte/src/encrypt.rs` (new file) 🟡

```rust
pub fn encrypt_chunk(data: &[u8], key_name: u64, key_service: &KeyService, block_index: usize) -> Result<Vec<u8>>
```

**Implementation:**

- [ ] **Encrypted Block Construction**:
  - [ ] Build encrypted block header (0x45 magic byte)
  - [ ] Key name size (8 bytes) + key name (8 bytes)
  - [ ] IV size (4 bytes) + random IV (4 bytes)
  - [ ] Encryption type byte (0x53 Salsa20, 0x41 ARC4)
- [ ] **Encryption Algorithm Integration**:
  - [ ] Call existing `encrypt_salsa20()` from ngdp-crypto
  - [ ] Call existing `encrypt_arc4()` from ngdp-crypto  
  - [ ] Proper block index handling for multi-chunk files
- [ ] **IV Generation**:
  - [ ] Secure random IV generation for each chunk
  - [ ] IV uniqueness validation
  - [ ] Integration with block index for multi-chunk
**Dependencies:** ngdp-crypto (already available) ✅
**Testing:** Encrypt -> decrypt round-trip verification
**Acceptance:** Encrypted chunks decrypt to original data

#### 2.2.8 BLTE Builder Pattern 🟡

**Location:** `blte/src/builder.rs` (new file) 🟡

```rust
pub struct BLTEBuilder {
    chunks: Vec<ChunkSpec>,
    flags: ChunkTableFlags,
    compression_strategy: CompressionStrategy,
}

pub struct ChunkSpec {
    data: Vec<u8>,
    compression: CompressionMode,
    encryption: Option<EncryptionSpec>,
    target_size: Option<usize>,
}
```

**Implementation:**

- [ ] **Builder API**:
  - [ ] `new()` - Create empty builder
  - [ ] `add_chunk(data, spec)` - Add data chunk with compression/encryption spec
  - [ ] `with_compression_strategy(strategy)` - Set overall compression approach
  - [ ] `with_encryption(key_name, algorithm)` - Enable encryption for all chunks
  - [ ] `build()` -> `Result<Vec<u8>>` - Construct final BLTE file
- [ ] **Compression Strategies**:
  - [ ] `Auto` - Choose best compression per chunk
  - [ ] `Uniform(mode)` - Use same mode for all chunks
  - [ ] `Custom` - Per-chunk specifications
- [ ] **Header Construction**:
  - [ ] Single-chunk mode (headerSize = 0) for small files
  - [ ] Multi-chunk mode with chunk table for large files
  - [ ] Proper chunk info format (0x0F standard, 0x10 extended)
  - [ ] MD5 checksum calculation for each chunk
**Dependencies:** Core compression (2.2.6) and encryption (2.2.7) functions
**Testing:** Build various BLTE file configurations
**Acceptance:** Built files parse correctly with existing header parser

#### 2.2.9 ESpec Parser and Processor 🟢

**Location:** `blte/src/espec.rs` (new file) 🟡

```rust
pub struct ESpecProcessor {
    strategies: Vec<CompressionStrategy>,
}

pub enum CompressionStrategy {
    ZLib { level: u8, chunk_size: usize },
    LZ4 { chunk_size: usize },
    None { chunk_size: usize },
    Encrypted { algorithm: EncryptionType, key_name: u64 },
}
```

**Implementation:**

- [ ] **ESpec String Parsing**:
  - [ ] Parse format: `z,9,{512*1024}` (ZLib level 9, 512KB chunks)
  - [ ] Parse format: `4,{1024*1024}` (LZ4, 1MB chunks)
  - [ ] Parse format: `e,s,12345678,{256*1024}` (Encrypted Salsa20, key, 256KB chunks)
  - [ ] Support multiple strategies: `z,6,{512*1024}:4,{256*1024}`
- [ ] **Size Expression Evaluation**:
  - [ ] Support arithmetic: `{512*1024}`, `{1024*1024}`
  - [ ] Named constants: `{DEFAULT_CHUNK_SIZE}`
  - [ ] Validation of chunk sizes (minimum/maximum limits)
- [ ] **Strategy Application**:
  - [ ] Apply strategies in sequence to data
  - [ ] Fallback handling when compression increases size
  - [ ] Integration with BLTEBuilder
**Dependencies:** None (string parsing only)
**Testing:** Parse real ESpec strings from encoding files
**Acceptance:** Correctly interprets and applies compression strategies

#### 2.2.10 Parallel Compression Support 🟢

**Location:** `blte/src/parallel.rs` (new file) 🟢

```rust
pub fn compress_parallel(data: Vec<u8>, spec: &CompressionSpec, thread_count: Option<usize>) -> Result<Vec<u8>>
```

**Implementation:**

- [ ] **Parallel Chunk Processing**:
  - [ ] Split data into chunks for parallel compression
  - [ ] Thread pool management with configurable size
  - [ ] Maintain chunk order in final output
- [ ] **Thread-Safe Compression**:
  - [ ] Ensure compression libraries are thread-safe
  - [ ] Separate KeyService instances for encryption
  - [ ] Memory management across threads
- [ ] **Performance Optimization**:
  - [ ] CPU core detection for optimal thread count
  - [ ] Work-stealing for balanced load
  - [ ] Memory usage monitoring
**Dependencies:** rayon (new dependency)
**Testing:** Compare single-threaded vs parallel performance
**Acceptance:** Significant speedup on multi-core systems

#### 2.2.11 Write Trait Implementation 🟢

**Location:** `blte/src/writer.rs` (new file) 🟢

```rust
pub struct BLTEWriter<W: Write> {
    writer: W,
    builder: BLTEBuilder,
    current_chunk: Vec<u8>,
    chunk_size: usize,
}
```

**Implementation:**

- [ ] **Write Trait Implementation**:
  - [ ] `write(&mut self, buf: &[u8])` - Accumulate data for chunking
  - [ ] `flush(&mut self)` - Finalize current chunk
  - [ ] Automatic chunking when size limits reached
- [ ] **Streaming Compression**:
  - [ ] Compress chunks as they're filled
  - [ ] Write BLTE headers and chunks incrementally
  - [ ] Memory-efficient for large file creation
- [ ] **Configuration Options**:
  - [ ] Configurable chunk size thresholds
  - [ ] Compression mode selection per chunk
  - [ ] Encryption parameters
**Dependencies:** std::io::Write
**Testing:** Stream large data through writer, verify output
**Acceptance:** Streaming writes create valid BLTE files

#### 2.2.12 Compression Examples and Benchmarks 🟢

**Location:** `blte/examples/` and `blte/benches/` 🟢

**Implementation:**

- [ ] **Example Programs**:
  - [ ] `compress_file.rs` - Compress single file with various modes
  - [ ] `create_encrypted_blte.rs` - Create encrypted BLTE files
  - [ ] `batch_compress.rs` - Compress multiple files efficiently
  - [ ] `streaming_compress.rs` - Memory-efficient compression of large files
- [ ] **Benchmark Suite**:
  - [ ] Compression speed benchmarks for all modes
  - [ ] Memory usage profiling during compression
  - [ ] Compare with reference implementations
  - [ ] Parallel vs sequential compression performance
- [ ] **Integration Tests**:
  - [ ] Round-trip testing (compress -> decompress)
  - [ ] Cross-compatibility with existing decompression
  - [ ] Stress testing with various file sizes
**Dependencies:** criterion (already available) ✅
**Testing:** All examples run successfully
**Acceptance:** Benchmarks show competitive performance

#### 2.2.13 CLI Integration for Compression 🟢

**Location:** `ngdp-client/src/commands/compress.rs` (new file) 🟢

```rust
pub async fn handle_compress(cmd: CompressCommands, format: OutputFormat) -> Result<()>
```

**Implementation:**

- [ ] **Compress Command**:
  - [ ] `ngdp compress file <input> <output>` - Compress single file
  - [ ] `--mode <mode>` - Specify compression mode (auto, zlib, lz4, none)
  - [ ] `--encrypt <key-name>` - Encrypt with specified key
  - [ ] `--chunk-size <size>` - Custom chunk size
- [ ] **Batch Operations**:
  - [ ] `ngdp compress batch <directory>` - Compress all files in directory
  - [ ] `--recursive` - Process subdirectories
  - [ ] `--filter <pattern>` - File pattern matching
- [ ] **Analysis Commands**:
  - [ ] `ngdp compress analyze <file>` - Show compression statistics
  - [ ] Compare original vs compressed sizes
  - [ ] Recommend optimal compression settings
**Dependencies:** clap (already available) ✅
**Testing:** CLI commands work with various file types
**Acceptance:** User-friendly compression interface

#### 2.2.14 Advanced Compression Features 🔵

**Location:** `blte/src/advanced.rs` (new file) 🔵

**Implementation:**

- [ ] **Adaptive Compression**:
  - [ ] Analyze data characteristics to choose optimal compression
  - [ ] Detect incompressible data and use 'N' mode automatically
  - [ ] Switch compression modes mid-stream based on effectiveness
- [ ] **Compression Presets**:
  - [ ] Fast: Minimal compression for speed
  - [ ] Balanced: Good compression with reasonable speed
  - [ ] Maximum: Best compression regardless of time
- [ ] **Content-Aware Compression**:
  - [ ] Detect file types and apply appropriate strategies
  - [ ] Special handling for already-compressed formats
  - [ ] Text vs binary optimization
- [ ] **Compression Statistics**:
  - [ ] Track compression ratios across different modes
  - [ ] Performance metrics collection
  - [ ] Recommendations for optimal settings
**Dependencies:** None (analysis only)
**Testing:** Verify adaptive strategies improve overall results
**Acceptance:** Better compression ratios than static modes

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
- [x] Added `inspect encoding` command for encoding file inspection
- [x] Added `inspect install` command for install manifest inspection  
- [x] Added `inspect download-manifest` command for download manifest inspection
- [x] Added `inspect size` command for size file inspection
- [x] All manifest commands download and decompress BLTE-encoded files from CDN
**Testing:** Tested with real WoW products (wow, wow_classic_era, wowt) ✅
**Acceptance:** Can analyze and display build configurations and manifests ✅

---

#### 4.2.1 Keys Update Command ✅

**Location:** `ngdp-client/src/commands/keys.rs` ✅

```rust
pub async fn handle_keys_command(command: KeysCommands) -> Result<()>
```

**Implementation:**

- [x] Download latest TACTKeys from GitHub repository
- [x] Parse and validate key format (CSV format)
- [x] Save to ~/.config/cascette/WoW.txt (or custom path)
- [x] Report number of keys found
- [x] Add `keys status` command to show local database info
- [x] Support for forced updates with `--force` flag
**Testing:** Successfully downloads and parses TACTKeys ✅
**Acceptance:** Updates local key database ✅

#### 4.2.2 File Download Command ✅

**Location:** `ngdp-client/src/commands/download.rs` ✅

```rust
pub async fn handle(cmd: DownloadCommands, format: OutputFormat) -> Result<()>
```

**Implementation:**

- [x] Command structure and full implementation
- [x] Support for content key and encoding key patterns
- [x] BLTE decompression integration ready
- [x] Build download command working with real CDN data
- [x] Downloads BuildConfig, CDNConfig, ProductConfig, KeyRing
- [x] Integration with cached Ribbit and CDN clients
- [x] Pattern detection for content keys, encoding keys, file paths
**Dependencies:** All core components ready ✅
**Testing:** Tested with wow_classic_era build downloads ✅
**Acceptance:** Successfully downloads build files from CDN ✅

#### 4.2.3 Installation Command 🟡

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

#### 4.2.4 Verification Command 🟡

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
- [x] Build config supports both single-hash and hash-size pair formats
- [x] Encoding file parser with 40-bit integer support
- [x] Install manifest parser with tag-based filtering
- [x] Download manifest parser with priority sorting (versions 1-3)
- [x] Size file parser with tag-based size calculation
- [x] TVFS parser basic structure (needs format revision with real data)
- [x] Variable-length integer support in utils
- [x] CLI integration with visual tree display
- [x] Tested parsers with real CDN data (discovered BLTE encoding requirement)

### Milestone 3: Decompression ✅

- [x] ngdp-crypto crate
- [x] blte crate
- [x] Encryption support (Salsa20 and ARC4)
- [x] All compression modes (N, Z, 4, F, E)
- [x] Key service with automatic loading from ~/.config/cascette/
- [x] 19,419 WoW encryption keys loaded and working

### Milestone 3.5: BLTE Compression/Encryption 🟡

- [ ] BLTE compression support (all modes: N, Z, 4, F, E)
- [ ] BLTE encryption support (Salsa20 and ARC4)
- [ ] Builder pattern for BLTE file construction
- [ ] ESpec parser for compression strategies
- [ ] Parallel compression support
- [ ] Write trait implementation for streaming
- [ ] CLI integration for compression operations
- [ ] Examples and benchmarks

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
- [x] Can parse all TACT formats ✅
- [ ] **Can create BLTE files with compression and encryption**
- [ ] **Can round-trip compress/decompress all BLTE modes**
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
6. **TVFS format** - Uses TFVS magic, big-endian, int32 offsets (not 40-bit)
7. **All CDN files are BLTE-encoded** - Must decompress before parsing
8. **BLTE compression** - Must mirror decompression logic exactly for compatibility
9. **IV generation** - Use secure random IVs for encryption, must be unique per chunk
10. **ESpec strings** - Follow exact format from encoding files: `z,level,{size}`
11. **Chunk checksums** - MD5 of compressed data, calculated before encryption

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

*Last Updated: 2025-08-07*
*Version: 1.6.0*

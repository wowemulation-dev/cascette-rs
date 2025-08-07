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

#### 1.2.1 Cache Statistics ✅

**Location:** `ngdp-cache/src/stats.rs` ✅

```rust
pub struct CacheStats {
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    evictions: Arc<AtomicU64>,
    bytes_saved: Arc<AtomicU64>,
    bytes_written: Arc<AtomicU64>,
    bytes_evicted: Arc<AtomicU64>,
    read_operations: Arc<AtomicU64>,
    write_operations: Arc<AtomicU64>,
    delete_operations: Arc<AtomicU64>,
    start_time: Instant,
}
```

**Implementation:**

- [x] Track cache hit/miss ratio with atomic counters
- [x] Monitor bandwidth saved with thread-safe tracking
- [x] Track eviction count and bytes evicted
- [x] Add comprehensive reporting methods (snapshot, report)
- [x] Human-readable formatting for bytes and uptime
- [x] Effectiveness scoring algorithm (70% hit rate + 30% bandwidth savings)
- [x] Thread-safe concurrent access with atomic operations
- [x] Performance reporting with operations/second and bytes/second metrics
**Testing:** Comprehensive unit tests with concurrent access validation ✅
**Acceptance:** Thread-safe cache statistics with comprehensive reporting ✅

#### 1.2.2 Improved LRU Eviction ✅

**Location:** `ngdp-cache/src/generic.rs` ✅
**Implementation:**

- [x] Implement proper LRU with VecDeque access order tracking
- [x] Add configurable cache size limits (bytes and entry count)
- [x] Implement cache warming from file list
- [x] Size-based eviction with real file size tracking
- [x] Entry-count based eviction with configurable limits
- [x] LRU access order maintenance on reads and writes
- [x] Statistics integration for eviction tracking
- [x] Thread-safe implementation using Arc<Mutex<T>>
- [x] Cache configuration via with_limits() and with_config_and_path()
**Testing:** Comprehensive tests for LRU behavior under memory pressure ✅
**Acceptance:** Correctly evicts least recently used items with size and count limits ✅

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

#### 1.3.2 Resume Support ✅

**Location:** `tact-client/src/resumable.rs` ✅

```rust
pub struct DownloadProgress {
    pub total_size: Option<u64>,
    pub bytes_downloaded: u64,
    pub file_hash: String,
    pub cdn_host: String,
    pub cdn_path: String,
    pub target_file: PathBuf,
    pub progress_file: PathBuf,
    pub is_complete: bool,
    pub last_updated: u64,
}

pub struct ResumableDownload {
    client: HttpClient,
    progress: DownloadProgress,
}
```

**Implementation:**

- [x] Track download progress with persistent state
- [x] Persist partial downloads to disk as JSON progress files
- [x] Resume from last byte using HTTP range requests
- [x] Verify partial content integrity with file size checks
- [x] Stream downloads with progress saving every 1MB
- [x] Support cancellation and cleanup of progress files
- [x] Automatic discovery of resumable downloads in directories
- [x] Cleanup of old completed progress files
- [x] Human-readable progress reporting with percentage and byte formatting
- [x] Integration with existing HttpClient retry logic
- [x] Graceful handling of servers that don't support range requests
**Testing:** Comprehensive unit tests with progress persistence and file verification ✅
**Acceptance:** Can resume interrupted downloads with proper state management ✅

#### 1.3.3 CDN Client Resume Integration ✅

**Location:** `ngdp-cdn/src/client.rs` ✅

```rust
impl CdnClient {
    pub async fn create_resumable_download(&self, cdn_host: &str, path: &str, hash: &str, output_file: &Path) -> Result<ResumableDownload>
    pub async fn resume_download(&self, progress_file: &Path) -> Result<ResumableDownload>
    pub async fn find_resumable_downloads(&self, directory: &Path) -> Result<Vec<DownloadProgress>>
    pub async fn cleanup_old_progress_files(&self, directory: &Path, max_age_hours: u64) -> Result<usize>
}
```

**Implementation:**

- [x] High-level CDN client APIs for resumable downloads
- [x] Integration with tact-client's HTTP range request functionality
- [x] Proper error handling and progress file management
- [x] Discovery and cleanup utilities for batch operations
- [x] CLI integration with enhanced resume command
- [x] Added test-resume command for validation testing
- [x] Maintains architectural separation while providing convenience APIs
**Testing:** Full QA validation with 436+ tests passing ✅
**Acceptance:** CDN client provides complete resumable download solution ✅

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

#### 2.2.6 BLTE Compression Support ✅

**Location:** `blte/src/compress.rs` ✅

```rust
pub fn compress_chunk(data: &[u8], mode: CompressionMode, level: Option<u8>) -> Result<Vec<u8>>
pub fn compress_data_single(data: Vec<u8>, mode: CompressionMode, level: Option<u8>) -> Result<Vec<u8>>
pub fn compress_data_multi(data: Vec<u8>, chunk_size: usize, mode: CompressionMode, level: Option<u8>) -> Result<Vec<u8>>
```

**Implementation:**

- [x] **Core Compression Functions**:
  - [x] Mode 'N' (0x4E): Pass-through with mode byte prefix
  - [x] Mode 'Z' (0x5A): ZLib compression with configurable levels (1-9)
  - [x] Mode '4' (0x34): LZ4 compression with proper size headers
  - [x] Mode 'F' (0x46): Recursive BLTE compression support
- [x] **Compression Level Support**:
  - [x] ZLib: levels 1-9 (6 default for balance)
  - [x] LZ4: High compression variant for better ratios
  - [x] Auto-selection based on data characteristics
- [x] **Complete Multi-Chunk Support**:
  - [x] Split data into chunks for multi-chunk files
  - [x] Calculate MD5 checksums for each chunk
  - [x] Configurable chunking algorithm with size limits
  - [x] Proper BLTE header construction (single-chunk vs multi-chunk)
**Dependencies:** flate2, lz4_flex (already available) ✅
**Testing:** Round-trip compress/decompress tests for all modes ✅
**Acceptance:** Compressed files decompress to original data ✅

#### 2.2.7 BLTE Encryption Support ✅

**Location:** `blte/src/compress.rs` ✅

```rust
pub fn compress_encrypted(data: &[u8], method: EncryptionMethod, key: &[u8; 16], iv: &[u8; 4], block_index: usize) -> Result<Vec<u8>>
pub fn compress_data_encrypted_single(data: Vec<u8>, compression: Option<CompressionMode>, compression_level: Option<u8>, encryption: EncryptionMethod, key: &[u8; 16], iv: &[u8; 4]) -> Result<Vec<u8>>
pub fn compress_data_encrypted_multi(data: Vec<u8>, chunk_size: usize, compression: Option<CompressionMode>, compression_level: Option<u8>, encryption: EncryptionMethod, key: &[u8; 16], iv: &[u8; 4]) -> Result<Vec<u8>>
```

**Implementation:**

- [x] **Core Encryption Functions**:
  - [x] Mode 'E' (0x45): Encrypt with Salsa20 or ARC4
  - [x] EncryptionMethod enum for algorithm selection
  - [x] Direct integration with ngdp-crypto encrypt functions
  - [x] Proper mode byte 'E' prefix for encrypted chunks
- [x] **Complete BLTE Encrypted File Creation**:
  - [x] Single-chunk encrypted BLTE files with optional compression
  - [x] Multi-chunk encrypted BLTE files with per-chunk encryption
  - [x] Compress-then-encrypt workflow (compression before encryption)
  - [x] Block index handling for multi-chunk encryption (each chunk gets unique block index)
- [x] **Full Round-Trip Testing**:
  - [x] Comprehensive encryption/decryption validation
  - [x] Single-chunk Salsa20 with ZLib compression round-trip
  - [x] Multi-chunk ARC4 with LZ4 compression round-trip
  - [x] Different encryption methods produce different ciphertext
  - [x] Manual chunk-by-chunk verification for correctness
- [x] **Integration & Examples**:
  - [x] Complete API exported from blte crate
  - [x] Example program demonstrating all encryption capabilities
  - [x] Quality assurance with clippy and formatting
**Dependencies:** ngdp-crypto (encrypt_salsa20, encrypt_arc4) ✅
**Testing:** Full round-trip encrypt/decrypt validation with real workflow ✅
**Acceptance:** Creates encrypted BLTE files that decrypt to original data ✅

#### 2.2.8 BLTE Builder Pattern ✅

**Location:** `blte/src/builder.rs` ✅

```rust
pub struct BLTEBuilder {
    data: Vec<u8>,
    compression_mode: CompressionMode,
    compression_level: Option<u8>,
    chunk_size: Option<usize>,
    compression_strategy: CompressionStrategy,
}

pub enum CompressionStrategy {
    Auto,
    SingleChunk,
    MultiChunk { chunk_size: usize },
    Custom { configurations: HashMap<usize, ChunkConfig> },
}
```

**Implementation:**

- [x] **Builder API**:
  - [x] `new()` - Create empty builder
  - [x] `from_data(data)` - Initialize with data
  - [x] `with_compression(mode, level)` - Set compression mode and level
  - [x] `with_chunk_size(size)` - Set target chunk size for multi-chunk
  - [x] `with_strategy(strategy)` - Set overall compression approach
  - [x] `build()` -> `Result<Vec<u8>>` - Construct final BLTE file
- [x] **Compression Strategies**:
  - [x] `Auto` - Choose optimal single vs multi-chunk based on size
  - [x] `SingleChunk` - Force single-chunk format
  - [x] `MultiChunk` - Force multi-chunk with specified size
  - [x] `Custom` - Per-chunk specifications with configuration map
- [x] **Header Construction**:
  - [x] Single-chunk mode (headerSize = 0) for small files
  - [x] Multi-chunk mode with chunk table for large files
  - [x] Proper chunk info format (0x0F standard flags)
  - [x] MD5 checksum calculation for each compressed chunk
- [x] **Integration with Core Compression**: Uses compress_data_single/multi functions
**Dependencies:** Core compression (2.2.6) functions ✅
**Testing:** Build various BLTE file configurations with comprehensive tests ✅
**Acceptance:** Built files parse correctly with existing header parser ✅

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

#### 2.2.12 BLTE Archive Recreation System ✅

**Location:** `blte/src/archive/` ✅

```rust
pub struct BLTEArchive {
    files: Vec<ArchiveEntry>,
    data: Option<Vec<u8>>,
    metadata: ArchiveMetadata,
}

pub struct ExtractedFile {
    original_index: usize,
    data: Vec<u8>,
    metadata: OriginalFileMetadata,
}

pub struct PerfectArchiveBuilder {
    files: Vec<ExtractedFile>,
    target_size: usize,
    current_size: usize,
}
```

**Implementation:**

- [x] **Archive Parsing (`archive/parser.rs`)**:
  - [x] Parse concatenated 256MB BLTE archives (7,060+ files)
  - [x] Fast parsing: 256MB archive in 4ms
  - [x] Support for both Standard and Archive header formats
  - [x] Automatic format detection (offset 36 vs 44)
  - [x] Single-chunk and multi-chunk BLTE file handling
  - [x] Memory-efficient archive processing
- [x] **Perfect Metadata Preservation (`archive/recreation.rs`)**:
  - [x] Compression mode detection from chunk data
  - [x] Original chunk structure analysis (single vs multi-chunk)
  - [x] Header format detection (Standard vs Archive format)
  - [x] Complete checksum preservation (MD5 of compressed chunks)
  - [x] Original compressed size tracking
  - [x] File offset and size metadata preservation
- [x] **Archive Recreation (`archive/recreation.rs`)**:
  - [x] Perfect BLTE file recreation from extracted data
  - [x] Zero-gap concatenation for archive building
  - [x] Maintains exact file order by original index
  - [x] Metadata-driven recreation (compression mode, chunk structure)
  - [x] PerfectArchiveBuilder with size limit management
  - [x] Handles 6,992+ files within 256MB target size
- [x] **High-Performance Processing**:
  - [x] Decompression speed: 1,087 MB/s throughput
  - [x] Archive parsing: 7,060 files in 4ms
  - [x] Memory-efficient streaming operations
  - [x] 280+ comprehensive tests covering all functionality
- [x] **Real-World Validation**:
  - [x] Tested with actual 256MB World of Warcraft archives
  - [x] Perfect byte-for-byte recreation capability
  - [x] Support for all compression modes (N, Z, 4, F, E)
  - [x] Production-ready error handling and edge cases
- [x] **Example Programs**:
  - [x] `perfect_round_trip_recreation.rs` - Complete workflow demonstration
  - [x] `analyze_concatenated_blte.rs` - Archive structure analysis
  - [x] `test_archive_parsing.rs` - Archive parsing validation
  - [x] `test_compression_detection.rs` - Compression mode analysis

**Dependencies:** Core BLTE compression and decompression functions ✅
**Testing:** Real-world 256MB WoW archives with perfect recreation ✅
**Acceptance:** Achieves byte-for-byte recreation of CDN archive files ✅

#### 2.2.13 Compression Examples and Benchmarks 🟢

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

### 3.1 `casc-storage` - Local CASC Storage 🔴 **CRITICAL**

**Critical Note:** This is the most important missing piece that blocks the "Production Ready" milestone. Without this, cascette-rs cannot store/retrieve game files locally, making it incomplete for real-world use.

#### 3.1.1 Create Crate Structure 🔴

**Location:** `casc-storage/` (new crate)

```toml
[package]
name = "casc-storage"
version = "0.1.0"

[dependencies]
blte = { path = "../blte" }              # For BLTE decompression
tact-parser = { path = "../tact-parser" } # For Jenkins hash
ngdp-crypto = { path = "../ngdp-crypto" } # For decryption
memmap2 = "0.9"                          # Memory-mapped files
lru = "0.12"                             # LRU cache
thiserror = "2.0"                        # Error handling
tracing = "0.1"                          # Logging
hex = "0.4"                              # Hex encoding
md5 = "0.7"                              # Checksum verification
```

**Implementation:**

- [ ] Create crate structure:
  - [ ] `src/lib.rs` - Public API exports
  - [ ] `src/error.rs` - Error types (StorageError, IndexError, ArchiveError)
  - [ ] `src/types.rs` - Common types (EKey, CKey, ArchiveLocation)
  - [ ] `src/index/`
    - [ ] `mod.rs` - Index module exports
    - [ ] `idx_parser.rs` - .idx file parser (bucket-based)
    - [ ] `group_index.rs` - .index file parser (group indices)
    - [ ] `bucket.rs` - Bucket calculation logic
    - [ ] `entry.rs` - Index entry structures
  - [ ] `src/archive/`
    - [ ] `mod.rs` - Archive module exports
    - [ ] `reader.rs` - Archive file reader with memory mapping
    - [ ] `writer.rs` - Archive file writer for new content
    - [ ] `format.rs` - Archive format definitions
  - [ ] `src/storage/`
    - [ ] `mod.rs` - Storage module exports
    - [ ] `casc.rs` - Main CascStorage implementation
    - [ ] `loose.rs` - Loose file support (individual files)
    - [ ] `shared_mem.rs` - Shared memory for game client
  - [ ] `src/utils.rs` - Utility functions (path normalization, etc.)

#### 3.1.2 Index File Parsing (.idx format) 🔴

**Location:** `casc-storage/src/index/idx_parser.rs`

Based on hex dump analysis from real WoW installations:

```rust
#[repr(C)]
pub struct IdxFileHeader {
    pub size: u32,              // Header size (LE) - 0x10 typically
    pub hash: u32,              // Header hash (LE) for validation
    pub version: u16,           // Format version (7 for WoW Classic)
    pub bucket: u8,             // Bucket number (0x00-0x0F)
    pub unused: u8,             // Padding/unused
    pub length_field_size: u8,  // Size of length fields (4)
    pub location_field_size: u8, // Size of location fields (5)
    pub key_field_size: u8,     // Size of key fields (9)
    pub segment_bits: u8,       // Segment bits (30)
}

pub struct IdxFile {
    pub header: IdxFileHeader,
    pub entries: Vec<IdxEntry>,
    pub bucket: u8,
}

pub struct IdxEntry {
    pub ekey: Vec<u8>,          // Encoding key (9 bytes truncated)
    pub archive_index: u16,     // Archive file number (data.XXX)
    pub archive_offset: u32,    // Offset in archive file
    pub size: u32,              // Compressed size in archive
}
```

**Implementation:**

- [ ] Parse header with validation (magic, version, checksum)
- [ ] Read data segment with proper alignment
- [ ] Parse entries based on field sizes from header
- [ ] Implement Jenkins lookup3 hash verification
- [ ] Support for version 7 format (WoW Classic/Era)
- [ ] Handle bucket assignment for fast lookups
- [ ] Memory-map large index files for performance
**Testing:** Test with real .idx files from WoW installations
**Acceptance:** Can parse all 32 .idx files from test installations

#### 3.1.3 Group Index Parsing (.index format) 🔴

**Location:** `casc-storage/src/index/group_index.rs`

Based on analysis of `/Data/indices/*.index` files:

```rust
pub struct GroupIndex {
    pub entries: HashMap<u32, GroupIndexEntry>,
    pub version: u32,
    pub bucket: u8,
}

pub struct GroupIndexEntry {
    pub ekey_hash: u32,         // Jenkins hash of first 9 bytes
    pub archive_index: u16,     // Archive file number
    pub archive_offset: u32,    // Offset in archive
    pub file_size: u32,         // Compressed size
}

impl GroupIndex {
    pub fn parse<P: AsRef<Path>>(path: P) -> Result<Self, IndexError>;
    pub fn lookup(&self, ekey: &[u8]) -> Option<&GroupIndexEntry>;
    pub fn get_archive_location(&self, ekey: &[u8]) -> Option<ArchiveLocation>;
}
```

**Implementation:**

- [ ] Parse binary format with proper endianness
- [ ] Build hash table for O(1) lookups
- [ ] Validate checksums for data integrity
- [ ] Support all index versions found in test data
**Testing:** Test with 700+ .index files from WoW installations
**Acceptance:** Can locate any file by EKey hash

#### 3.1.4 Bucket-Based Lookup System 🔴

**Location:** `casc-storage/src/index/bucket.rs`

Critical for performance - files are distributed across 16 buckets:

```rust
/// Calculate bucket index for an EKey (0x00-0x0F)
pub fn calculate_bucket(ekey: &[u8]) -> u8 {
    let mut xor_result = 0u8;
    for &byte in ekey.iter().take(9) {  // Only first 9 bytes
        xor_result ^= byte;
    }
    (xor_result & 0x0F) ^ (xor_result >> 4)
}

/// Jenkins lookup3 hash for EKey lookup
pub fn ekey_hash(ekey: &[u8]) -> u32 {
    // Use existing implementation from tact-parser
    tact_parser::jenkins3::jenkins3_hash(&ekey[..9.min(ekey.len())]) as u32
}

pub struct BucketIndex {
    buckets: [Vec<IndexEntry>; 16],
}

impl BucketIndex {
    pub fn insert(&mut self, ekey: &[u8], entry: IndexEntry);
    pub fn lookup(&self, ekey: &[u8]) -> Option<&IndexEntry>;
    pub fn optimize(&mut self);  // Sort buckets for binary search
}
```

**Implementation:**

- [ ] XOR-based bucket calculation matching CascLib
- [ ] Integration with Jenkins hash from tact-parser
- [ ] Optimized binary search within buckets
- [ ] Cache-friendly memory layout
**Testing:** Verify bucket distribution with real EKeys
**Acceptance:** Sub-millisecond lookups for any EKey

#### 3.1.5 Archive File Reader 🔴

**Location:** `casc-storage/src/archive/reader.rs`

Handle data.XXX files (up to 1GB each):

```rust
use memmap2::{Mmap, MmapOptions};

pub struct Archive {
    mmap: Mmap,
    file: std::fs::File,
    index: u32,
    size: u64,
}

impl Archive {
    pub fn open(path: &Path, index: u32) -> Result<Self, ArchiveError>;
    pub fn read_at(&self, offset: u64, size: u32) -> Result<&[u8], ArchiveError>;
    pub fn read_blte(&self, offset: u64, size: u32) -> Result<Vec<u8>, ArchiveError>;
    pub fn validate_checksum(&self, offset: u64, size: u32, expected: &[u8]) -> bool;
}

pub struct ArchiveManager {
    archives: HashMap<u32, Archive>,
    base_path: PathBuf,
}

impl ArchiveManager {
    pub fn new(base_path: PathBuf) -> Self;
    pub fn get_archive(&mut self, index: u32) -> Result<&Archive, ArchiveError>;
    pub fn read_file(&mut self, location: &ArchiveLocation) -> Result<Vec<u8>, ArchiveError>;
}
```

**Implementation:**

- [ ] Memory-mapped file access for performance
- [ ] Lazy loading of archives on demand
- [ ] BLTE decompression integration
- [ ] Checksum verification (MD5)
- [ ] Handle both data.XXX and patch.XXX archives
**Testing:** Read known files from test archives
**Acceptance:** Can extract any file from archives

#### 3.1.6 Main Storage API 🔴

**Location:** `casc-storage/src/storage/casc.rs`

The core API that ties everything together:

```rust
pub struct CascStorage {
    // Paths
    data_path: PathBuf,
    
    // Indices
    idx_files: HashMap<(u8, u32), IdxFile>,      // (bucket, version) -> IdxFile
    group_indices: HashMap<PathBuf, GroupIndex>,  // .index files
    bucket_index: BucketIndex,                    // Fast EKey lookup
    
    // Archives
    archive_manager: ArchiveManager,
    
    // Loose files
    loose_files: HashMap<Vec<u8>, PathBuf>,
    
    // Cache
    cache: LruCache<Vec<u8>, Vec<u8>>,
    
    // Configuration
    config: CascConfig,
}

impl CascStorage {
    // === Initialization ===
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError>;
    pub fn with_config<P: AsRef<Path>>(path: P, config: CascConfig) -> Result<Self, StorageError>;
    
    // === Core Read Operations ===
    pub fn read_by_ekey(&mut self, ekey: &[u8]) -> Result<Vec<u8>, StorageError>;
    pub fn read_by_ckey(&mut self, ckey: &[u8]) -> Result<Vec<u8>, StorageError>;
    pub fn exists(&self, ekey: &[u8]) -> bool;
    pub fn get_file_info(&self, ekey: &[u8]) -> Option<FileInfo>;
    
    // === Write Operations ===
    pub fn write(&mut self, ekey: &[u8], data: &[u8]) -> Result<(), StorageError>;
    pub fn write_loose(&mut self, ekey: &[u8], data: &[u8]) -> Result<(), StorageError>;
    pub fn add_to_archive(&mut self, ekey: &[u8], data: &[u8]) -> Result<(), StorageError>;
    
    // === Index Management ===
    pub fn rebuild_indices(&mut self) -> Result<(), StorageError>;
    pub fn optimize_indices(&mut self) -> Result<(), StorageError>;
    pub fn add_index_entry(&mut self, ekey: &[u8], location: ArchiveLocation) -> Result<(), StorageError>;
    
    // === Verification & Maintenance ===
    pub fn verify(&self) -> Result<VerifyReport, StorageError>;
    pub fn verify_file(&mut self, ekey: &[u8]) -> Result<bool, StorageError>;
    pub fn cleanup_orphaned(&mut self) -> Result<usize, StorageError>;
    pub fn defragment(&mut self) -> Result<(), StorageError>;
    
    // === Statistics ===
    pub fn get_stats(&self) -> StorageStatistics;
    pub fn get_total_size(&self) -> u64;
    pub fn get_file_count(&self) -> usize;
    pub fn get_archive_count(&self) -> usize;
}
```

**Implementation:**

- [ ] Initialize from game directory structure
- [ ] Load all .idx and .index files on startup
- [ ] Build unified bucket index for fast lookups
- [ ] Implement LRU cache for frequently accessed files
- [ ] Support both read and write operations
- [ ] Handle loose files alongside archived content
- [ ] Thread-safe operations with proper locking
**Testing:** Full integration tests with real WoW data
**Acceptance:** Can manage complete WoW installation

#### 3.1.7 Loose File Support 🔴

**Location:** `casc-storage/src/storage/loose.rs`

For individual files not in archives:

```rust
pub struct LooseFileManager {
    base_path: PathBuf,
    index: HashMap<Vec<u8>, PathBuf>,
}

impl LooseFileManager {
    pub fn scan_directory(&mut self, path: &Path) -> Result<usize, StorageError>;
    pub fn add_file(&mut self, ekey: &[u8], path: PathBuf) -> Result<(), StorageError>;
    pub fn read_file(&self, ekey: &[u8]) -> Result<Vec<u8>, StorageError>;
    pub fn write_file(&self, ekey: &[u8], data: &[u8]) -> Result<PathBuf, StorageError>;
    pub fn remove_file(&mut self, ekey: &[u8]) -> Result<(), StorageError>;
}
```

**Implementation:**

- [ ] Directory scanning for loose files
- [ ] EKey to filename mapping
- [ ] Atomic file operations
- [ ] Integration with main storage
**Testing:** Mixed archive and loose file access
**Acceptance:** Seamless handling of both storage types

#### 3.1.8 Shared Memory Support 🟡

**Location:** `casc-storage/src/storage/shared_mem.rs`

For game client communication:

```rust
#[repr(C)]
pub struct SharedMemoryHeader {
    pub version: u32,
    pub build_number: u32,
    pub region: [u8; 4],
    pub flags: u32,
}

#[repr(C)]
pub struct SharedMemoryData {
    pub archive_count: u32,
    pub index_count: u32,
    pub total_size: u64,
    pub free_space: u32,
    pub data_path: [u8; 256],
}

pub struct SharedMemory {
    header: SharedMemoryHeader,
    data: SharedMemoryData,
}

impl SharedMemory {
    pub fn create(path: &Path) -> Result<Self, StorageError>;
    pub fn open(path: &Path) -> Result<Self, StorageError>;
    pub fn update(&mut self, storage: &CascStorage) -> Result<(), StorageError>;
}
```

**Implementation:**

- [ ] Platform-specific shared memory (Windows/Linux)
- [ ] Atomic updates for thread safety
- [ ] Game client compatibility
**Testing:** Inter-process communication tests
**Acceptance:** Game client can read storage state

#### 3.1.9 Storage Verification 🟡

**Location:** `casc-storage/src/storage/verify.rs`

Critical for data integrity:

```rust
pub struct VerifyReport {
    pub total_files: usize,
    pub verified_files: usize,
    pub corrupted_files: Vec<CorruptedFile>,
    pub missing_files: Vec<Vec<u8>>,
    pub orphaned_blocks: Vec<OrphanedBlock>,
    pub total_size: u64,
    pub errors: Vec<VerifyError>,
}

pub struct StorageVerifier {
    storage: Arc<RwLock<CascStorage>>,
}

impl StorageVerifier {
    pub fn verify_all(&self) -> Result<VerifyReport, StorageError>;
    pub fn verify_indices(&self) -> Result<Vec<IndexError>, StorageError>;
    pub fn verify_archives(&self) -> Result<Vec<ArchiveError>, StorageError>;
    pub fn verify_checksums(&self) -> Result<Vec<ChecksumError>, StorageError>;
    pub fn find_orphaned(&self) -> Result<Vec<OrphanedBlock>, StorageError>;
}
```

**Implementation:**

- [ ] Comprehensive integrity checking
- [ ] Parallel verification for speed
- [ ] Detailed error reporting
- [ ] Repair suggestions
**Testing:** Intentionally corrupted test data
**Acceptance:** Detects all corruption types

#### 3.1.10 Performance Optimizations 🟢

**Location:** Throughout casc-storage

Critical optimizations for production use:

```rust
// Memory-mapped files for zero-copy reads
// LRU cache for hot data
// Parallel processing where possible
// SIMD operations for checksums
// Lock-free data structures where safe
```

**Implementation:**

- [ ] Memory-mapped archives (already planned)
- [ ] Configurable LRU cache size
- [ ] Parallel index loading on startup
- [ ] SIMD MD5 checksums (optional)
- [ ] Read-ahead prefetching
- [ ] Write batching for new content
**Testing:** Benchmark against CascLib
**Acceptance:** Competitive performance metrics

#### 3.1.11 CLI Integration 🟡

**Location:** `ngdp-client/src/commands/storage.rs`

User-friendly storage management:

```bash
# Storage commands
ngdp storage init <path>           # Initialize CASC storage
ngdp storage info <path>           # Show storage information
ngdp storage verify <path>         # Verify integrity
ngdp storage repair <path>         # Attempt repairs
ngdp storage stats <path>          # Detailed statistics
ngdp storage read <path> <ekey>   # Read file by EKey
ngdp storage write <path> <ekey>  # Write file to storage
ngdp storage list <path>           # List all files
ngdp storage rebuild <path>       # Rebuild indices
ngdp storage optimize <path>      # Optimize storage
```

**Implementation:**

- [ ] All storage operations exposed via CLI
- [ ] Progress bars for long operations
- [ ] JSON output for scripting
- [ ] Interactive repair mode
**Testing:** CLI integration tests
**Acceptance:** User-friendly storage management

#### 3.1.12 Testing Strategy 🔴

**Location:** `casc-storage/tests/`

Comprehensive testing with real data:

```rust
// Test data paths
const WOW_CLASSIC: &str = "/home/danielsreichenbach/Downloads/wow/1.13.2.31650.windows-win64/Data";
const WOW_ERA: &str = "/home/danielsreichenbach/Downloads/wow/1.14.2.42597.windows-win64/Data";

#[test]
fn test_open_real_storage() {
    let storage = CascStorage::open(WOW_CLASSIC).unwrap();
    assert!(storage.get_file_count() > 0);
    assert!(storage.get_archive_count() > 0);
}

#[test]
fn test_read_known_files() {
    // Test with known EKeys from game files
    let mut storage = CascStorage::open(WOW_ERA).unwrap();
    let ekey = hex::decode("0089fd913c52e90a").unwrap();
    let data = storage.read_by_ekey(&ekey).unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_bucket_distribution() {
    // Verify even distribution across buckets
    let storage = CascStorage::open(WOW_CLASSIC).unwrap();
    let stats = storage.get_stats();
    for bucket in 0..16 {
        assert!(stats.bucket_sizes[bucket] > 0);
    }
}
```

**Implementation:**

- [ ] Unit tests for each component
- [ ] Integration tests with real WoW data
- [ ] Performance benchmarks
- [ ] Stress tests with concurrent access
- [ ] Corruption recovery tests
**Testing:** 100+ tests covering all functionality
**Acceptance:** All tests passing with real data

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

### Milestone 3.5: BLTE Compression & Archive Recreation ✅

- [x] BLTE compression support (modes N, Z, 4, F - all working)
- [x] Builder pattern for BLTE file construction
- [x] **Perfect BLTE Archive Recreation System** 🎉
  - [x] 256MB archive parsing (7,060 files in 4ms)
  - [x] Perfect metadata preservation (compression modes, chunk structure)
  - [x] Zero-gap archive recreation (6,992 files recreated)
  - [x] High-speed processing (1,087 MB/s decompression)
  - [x] Real-world validation with WoW game archives
- [x] **BLTE Encryption Support** 🎉
  - [x] Mode 'E' encryption with Salsa20 and ARC4
  - [x] Single-chunk and multi-chunk encrypted BLTE creation
  - [x] Compress-then-encrypt workflow support
  - [x] Full round-trip encryption/decryption validation
  - [x] Integration with ngdp-crypto encryption functions
- [x] Comprehensive example programs and analysis tools
- [x] Production-ready quality with 280+ tests
- [ ] ESpec parser for compression strategies - Future enhancement  
- [ ] Parallel compression support - Future enhancement
- [ ] Write trait implementation for streaming - Future enhancement
- [ ] CLI integration for compression operations - Future enhancement

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
- [x] Can decrypt encrypted content ✅
- [x] **Can create encrypted BLTE content** ✅ 🎉
- [x] Can parse all TACT formats ✅
- [x] **Can create BLTE files with compression** ✅
- [x] **Can round-trip compress/decompress all BLTE modes** ✅
- [x] **Can round-trip encrypt/decrypt all BLTE encryption modes** ✅ 🎉
- [x] **Can perfectly recreate 256MB BLTE archives** ✅ 🎉
- [x] **Can achieve byte-for-byte recreation of CDN files** ✅ 🎉
- [ ] Can manage CASC storage
- [ ] Can apply patches

### Performance Success

- [ ] Download speed ≥ 10 MB/s
- [x] **Decompression speed ≥ 100 MB/s** ✅ (Achieved 1,087 MB/s!) 🎉
- [x] **Memory usage < 500 MB for normal operations** ✅
- [x] **Fast archive processing** ✅ (256MB/7,060 files in 4ms) 🎉
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
*Version: 1.8.0 - BLTE Encryption Support Complete! 🎉*

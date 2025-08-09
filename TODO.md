# Cascette-RS TODO

> For completed work, see [ROADMAP.md](ROADMAP.md)

## Priority 1: Critical Format Parsing Gaps 🔴

### ESpec (Encoding Specification) Parser - **CRITICAL**

**Status:** ❌ **Missing completely** - We read but don't parse according to EBNF grammar  
**Location:** `tact-parser/src/espec.rs` (create)

**Issues Found:**
- We read ESpec data from encoding files but **immediately discard it** (encoding.rs:91-93)
- TVFS ESpec strings read as raw UTF-8 but not parsed into usable structures
- No implementation of EBNF grammar from https://wowdev.wiki/BLTE#Encoding_Specification_(ESpec)

**Required Implementation:**
- [ ] **CRITICAL**: Implement complete EBNF grammar parser:
  ```
  e-spec = ( 'n' ) | ( 'z' [...] ) | ( 'e' [...] ) | ( 'b' [...] ) | ( 'c' [...] ) | ( 'g' [...] )
  ```
- [ ] Parse complex block specifications: `b:{164=z,16K*565=z,1656=z,140164=z}`  
- [ ] Support nested compression modes and size expressions
- [ ] Store parsed ESpec data in `EncodingFile` struct instead of discarding
- [ ] Integration with BLTE decompression for proper file reconstruction
- [ ] Tests with real-world ESpec examples from CDN data

### TVFS Implementation Fix - **HIGH PRIORITY**

**Status:** 🟡 **Major discrepancies** from wowdev.wiki specification  
**Location:** `tact-parser/src/tvfs.rs` (fix existing)

**Critical Issues Found:**
1. ~~**Wrong Magic**: We use `TFVS` but spec shows `TVFS` (0x53465654)~~ ✅ **FIXED**
2. **Wrong Field Types**: Using i32 instead of uint40_t for offsets/sizes
3. **Missing 40-bit Integer Support**: TVFS uses 5-byte integers extensively
4. **Wrong Header Structure**: Our struct doesn't match the specification
5. **Missing Flags Support**: No proper flag parsing (INCLUDE_CKEY, WRITE_SUPPORT, etc.)

**Required Fixes:**
- [x] ~~**URGENT**: Fix magic bytes to match specification~~ ✅ **COMPLETED** 
  - Fixed parser to only accept `TVFS` (0x53465654) instead of both `TFVS`/`TVFS`
  - Updated documentation comments to reflect correct specification
  - All tests pass - test data was already using correct `TVFS` magic
- [ ] **CRITICAL**: Implement 40-bit integer reading for offsets and sizes  
- [ ] **HIGH**: Update header structure to match specification exactly
- [ ] **HIGH**: Add proper flag parsing and handling
- [ ] **MEDIUM**: Validate against latest wowdev.wiki specification  
- [ ] **LOW**: Add comprehensive tests with real TVFS data

### 40-bit Integer Support - **FOUNDATION REQUIREMENT**

**Status:** ❌ **Missing** - Required for TVFS and other TACT formats  
**Location:** `tact-parser/src/utils.rs` (extend existing)

**Background:** Multiple TACT formats use 5-byte (40-bit) integers:
- TVFS: All offset/size fields are uint40_t  
- Encoding: File sizes up to 1TB support
- Other manifests: Various size fields

**Required Implementation:**  
- [ ] Add `read_uint40_le()` and `read_uint40_be()` functions
- [ ] Support little-endian and big-endian variants  
- [ ] Integration with existing utils module
- [ ] Tests for edge cases (max values, overflow handling)

### Research Findings Summary

**Key Discoveries from docs/research/ analysis:**

1. **Our ESpec Knowledge**: We documented the format structure in `blte-format-detailed.md` but never implemented parsing
2. **Our TVFS Knowledge**: We have complete specification in `tvfs-specification.md` but implementation doesn't match
3. **Reference Implementation Gaps**: Local references mentioned in CLAUDE.md aren't accessible in current session
4. **Critical Missing Components**: 
   - ESpec grammar parser (blocks write support entirely)
   - 40-bit integer support (breaks TVFS and large file handling)  
   - Proper TVFS implementation (affects modern game versions)

**Impact Assessment:**
- **ESpec Parser**: Without this, we cannot properly decompress complex BLTE files or implement write support
- **TVFS Fixes**: Current implementation may fail with modern game builds that use correct TVFS format  
- **40-bit Integers**: Fundamental requirement for handling large files and proper TVFS parsing

## Priority 2: Write Support 🔴

### TACT Format Writers

#### Encoding File Writer

**Location:** `tact-parser/src/encoding/writer.rs`

- [ ] Create encoding header with proper magic and version
- [ ] Build CEKey pages with 40-bit size encoding (BIG-ENDIAN)
- [ ] Build EKey spec pages with proper layout
- [ ] Calculate page checksums (MD5)
- [ ] Support multiple EKeys per CKey
- [ ] Write compressed BLTE output

#### Install Manifest Writer

**Location:** `tact-parser/src/install/writer.rs`

- [ ] Write "IN" magic header
- [ ] Build tag table with proper encoding
- [ ] Calculate tag bitmask size
- [ ] Write file entries with CKeys and paths
- [ ] Apply tag bitmasks to entries
- [ ] Compress with BLTE

#### Download Manifest Writer

**Location:** `tact-parser/src/download/writer.rs`

- [ ] Write "DL" magic header
- [ ] Support versions 1, 2, and 3 formats
- [ ] Build priority-sorted entry list
- [ ] Encode file sizes and priorities
- [ ] Build tag table and apply to entries
- [ ] Compress with BLTE

#### Size File Writer

**Location:** `tact-parser/src/size/writer.rs`

- [ ] Write "SP" magic header
- [ ] Encode partial EKey entries (9 bytes)
- [ ] Build size table with proper encoding
- [ ] Support tag-based filtering
- [ ] Calculate total size statistics
- [ ] Compress with BLTE

#### Config File Writer

**Location:** `tact-parser/src/config/writer.rs`

- [ ] Write key-value pairs with " = " separator
- [ ] Support empty values ("key = ")
- [ ] Write hash-size pairs format
- [ ] Add comments support
- [ ] Maintain key ordering for consistency

#### TVFS Writer

**Location:** `tact-parser/src/tvfs/writer.rs`

- [ ] Write TVFS/TFVS magic header (big-endian)
- [ ] Build path table with compression
- [ ] Build VFS table with file entries
- [ ] Build CFT table with content info
- [ ] Optional EST table for ESpec data
- [ ] Compress with BLTE

#### Root Manifest Writer

**Location:** `tact-parser/src/root/writer.rs`

- [ ] Parse and write root file format (version 1 and 2)
- [ ] Handle FileDataID to CKey mappings
- [ ] Support locale and content flags
- [ ] Implement name hash lookups
- [ ] Compress with BLTE

### BPSV Writer

**Location:** `ngdp-bpsv/src/writer.rs`

- [ ] Write schema header line with field names and types
- [ ] Encode values with proper type formatting
- [ ] Support empty values and proper escaping
- [ ] Maintain field ordering from schema
- [ ] Generate compliant BPSV output

### Key Service Extensions

**Location:** `ngdp-crypto/src/key_service.rs`

- [ ] Key generation using cryptographically secure RNG
- [ ] Key ID generation algorithm
- [ ] Export functionality for CSV/TSV/TXT formats
- [ ] Key database persistence with atomic writes
- [ ] Support for custom key naming schemes

### FileDataID Manager

**Location:** `tact-parser/src/fdid_manager.rs`

- [ ] FileDataID assignment algorithm
- [ ] Reserved ranges for different asset types
- [ ] Listfile import/export for community compatibility
- [ ] Collision detection and resolution
- [ ] Integration with root manifest generation

### CASC Index Writers

#### IDX File Writer

**Location:** `casc-storage/src/index/writers/idx_writer.rs`

- [ ] Write .idx header with proper field sizes
- [ ] Calculate header checksum
- [ ] Write entries with 9-byte truncated EKeys
- [ ] Maintain 16-byte alignment for data segments
- [ ] Support version 7 format
- [ ] Generate proper bucket assignment

#### Group Index Writer

**Location:** `casc-storage/src/index/writers/group_writer.rs`

- [ ] Generate Jenkins hash for EKeys
- [ ] Build binary format with proper endianness
- [ ] Support variable field sizes
- [ ] Create .index files compatible with game

#### Archive Builder

**Location:** `casc-storage/src/archive/builder.rs`

- [ ] Build CASC archive files (data.XXX)
- [ ] Compress files with BLTE
- [ ] Track file offsets and sizes
- [ ] Generate index entries for added files
- [ ] Support 1GB archive size limit
- [ ] Handle archive rollover

## Priority 2: Content Management System 🔴

### Core Components

#### Asset Pipeline

**Location:** `content-manager/src/pipeline.rs`

- [ ] Model importers (OBJ → M2, FBX → WMO)
- [ ] Texture converters (PNG/TGA → BLP)
- [ ] Audio processors (WAV/OGG → game format)
- [ ] Asset optimization (compression, LOD generation)
- [ ] Metadata extraction and cataloging
- [ ] Validation against game requirements

#### Database Management

**Location:** `content-manager/src/database.rs`

- [ ] DBC/DB2 file parsing and writing
- [ ] Schema validation for all tables
- [ ] Referential integrity checking
- [ ] Query engine for content developers
- [ ] Bulk import/export operations
- [ ] Change tracking and audit logs

#### Version Control

**Location:** `content-manager/src/versioning.rs`

- [ ] Branch management for different environments
- [ ] Atomic commits with rollback support
- [ ] Conflict detection and resolution
- [ ] Change tracking with audit trail
- [ ] Diff generation for review
- [ ] Tag system for releases

### Tool Integrations

#### Blender Integration

**Location:** `content-manager/src/integrations/blender.rs`

- [ ] Blender addon with Python API client
- [ ] Model validation on import
- [ ] Auto-generate collision mesh
- [ ] Generate LODs with quality settings

#### Quest Designer Integration

**Location:** `content-manager/src/integrations/quest.rs`

- [ ] Import quest chain definitions
- [ ] Validate quest requirements and rewards
- [ ] Compile quest scripts to bytecode
- [ ] Generate quest text localizations

#### Map Editor Integration

**Location:** `content-manager/src/integrations/map.rs`

- [ ] Import ADT (map tile) data
- [ ] Process heightmaps and textures
- [ ] Place doodads and WMOs on map
- [ ] Generate navigation mesh for pathfinding

## Priority 3: Build System 🔴

### NGDP Builder

**Location:** `ngdp-builder/`

#### Product Builder Core

- [ ] Integration with ContentManager for asset retrieval
- [ ] Pull game databases from DatabaseManager
- [ ] Generate unique CKeys for all files (MD5 hash)
- [ ] Create EKeys for encoded content
- [ ] Build complete encoding file with all mappings
- [ ] Generate all required manifests
- [ ] Create build and CDN configurations
- [ ] Output complete CASC storage structure

#### Content Processing Pipeline

- [ ] Auto-detect file types for optimal compression
- [ ] Apply appropriate BLTE compression
- [ ] Generate encryption keys for sensitive files
- [ ] Integrate with KeyService for key management
- [ ] Generate ESpec strings for encoding
- [ ] Calculate all checksums (MD5, XXH64)

#### CASC Storage Generator

- [ ] Create data.XXX archive files
- [ ] Build .idx files for all buckets (00-0F)
- [ ] Generate .index group indices
- [ ] Create loose file structure
- [ ] Generate .build.info file
- [ ] Create complete Data/config structure

## Priority 4: Server Implementation 🔴

### Ribbit Server - Central Orchestrator

**Location:** `ngdp-server/src/ribbit.rs`

#### Build Management

- [ ] Build ingestion pipeline from ngdp-builder
- [ ] Build staging and validation before distribution
- [ ] CDN distribution orchestration
- [ ] Atomic version updates (all-or-nothing)
- [ ] Region-specific build promotion
- [ ] Build rollback capabilities

#### Protocol Handling

- [ ] Parse Ribbit protocol commands
- [ ] Serve product version information
- [ ] Support region-specific responses
- [ ] Handle subscription-based updates
- [ ] Generate properly formatted Ribbit responses

### CDN Server - Distribution Endpoint

**Location:** `ngdp-server/src/cdn.rs`

#### Build Reception

- [ ] Build reception protocol from Ribbit
- [ ] Build validation and integrity checking
- [ ] Atomic build deployment (staging → live)

#### Content Serving

- [ ] Implement CDN path structure
- [ ] Support HTTP range requests for partial downloads
- [ ] Implement caching layer for frequently accessed files
- [ ] Support multiple CDN hosts for load balancing
- [ ] Generate proper CDN responses with headers
- [ ] Status reporting back to Ribbit orchestrator

### TACT HTTP API Proxy

**Location:** `ngdp-server/src/tact_proxy.rs`

- [ ] Convert HTTP paths to Ribbit commands
- [ ] Transform Ribbit pipe-delimited responses to JSON
- [ ] Cache JSON transformations for performance
- [ ] Handle region and version query parameters
- [ ] Maintain compatibility with Battle.net client expectations
- [ ] Support both Ribbit protocol and HTTP API from single data source

## Priority 5: Advanced Features 🔵

### BLTE Enhancements

#### ESpec Parser

**Location:** `blte/src/espec.rs`

- [ ] Parse ESpec strings (z,9,{512*1024})
- [ ] Size expression evaluation
- [ ] Strategy application to data
- [ ] Fallback handling when compression increases size
- [ ] Integration with BLTEBuilder

#### Parallel Compression

**Location:** `blte/src/parallel.rs`

- [ ] Split data into chunks for parallel compression
- [ ] Thread pool management with configurable size
- [ ] Maintain chunk order in final output
- [ ] CPU core detection for optimal thread count

#### Write Trait Implementation

**Location:** `blte/src/writer.rs`

- [ ] Implement Write trait for streaming compression
- [ ] Automatic chunking when size limits reached
- [ ] Memory-efficient for large file creation

### CLI Enhancements

#### Compression Commands

**Location:** `ngdp-client/src/commands/compress.rs`

- [ ] `ngdp compress file` command
- [ ] `ngdp compress batch` for directories
- [ ] `ngdp compress analyze` for statistics
- [ ] Support for all compression modes
- [ ] Encryption support with key management

#### Build Commands

**Location:** `ngdp-client/src/commands/build.rs`

- [ ] `ngdp build create` from directory
- [ ] `ngdp build from-manifest` with YAML
- [ ] `ngdp build rebuild` from installation
- [ ] `ngdp build validate` for verification

### Research Areas 🔍

#### Build System Research

- [ ] Asset format conversion strategies
- [ ] FileDataID assignment algorithms
- [ ] Content dependency resolution
- [ ] Version management approaches

#### CDN Infrastructure Research

- [ ] File upload and distribution processes
- [ ] Path generation strategies
- [ ] Cache invalidation mechanisms
- [ ] Geographic distribution patterns

#### Protocol Analysis

- [ ] Battle.net client communication
- [ ] Update detection mechanisms
- [ ] Progress reporting protocols
- [ ] Error handling strategies

## Testing Requirements

### Unit Tests

- [ ] Writers for all TACT formats
- [ ] Content management components
- [ ] Build system functionality
- [ ] Server implementation

### Integration Tests

- [ ] Round-trip write/read for all formats
- [ ] Complete build generation
- [ ] Server-client communication
- [ ] Content pipeline workflow

### Performance Tests

- [ ] Large file handling (10GB+)
- [ ] Concurrent client support
- [ ] Build generation speed
- [ ] CDN throughput

## Documentation Needs

### API Documentation

- [ ] Writer APIs for all formats
- [ ] Content management interfaces
- [ ] Server protocol specifications
- [ ] Build system workflows

### User Guides

- [ ] Content creation workflow
- [ ] Build generation tutorial
- [ ] Server deployment guide
- [ ] Migration from existing systems

### Examples

- [ ] Writing each TACT format
- [ ] Creating custom builds
- [ ] Setting up servers
- [ ] Content pipeline integration

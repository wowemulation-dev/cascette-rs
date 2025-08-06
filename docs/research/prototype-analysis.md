# Cascette-RS Prototype Analysis

## Overview

The cascette-rs prototype located at `/home/danielsreichenbach/Downloads/wow/cascette-rs` represents a more functionally complete implementation than initially documented. This analysis captures all critical discoveries from the prototype that should be implemented in the main cascette-rs project.

## Prototype Structure

The prototype uses a multi-crate workspace with 11 crates:
- `cascette-core` - Core types and traits
- `cascette-formats` - File format parsers (MOST COMPLETE)
- `cascette-crypto` - Cryptographic utilities
- `cascette-ribbit` - Ribbit protocol client
- `cascette-casc` - CASC storage (partial)
- `cascette-cdn-client` - CDN client with caching
- `cascette-cdn-server` - CDN server implementation
- `cascette-builder` - Archive builder
- `cascette-repository` - Repository management
- `cascette-client` - Client library
- `cascette-server` - Server implementation
- `cascette-cli` - Command-line interface

## Critical Implementations Found

### 1. Complete BLTE Encryption Support

**Location**: `crates/cascette-formats/src/blte_decrypt.rs`

The prototype implements BOTH Salsa20 and ARC4 encryption:

#### Salsa20 Implementation
- Extends 16-byte BLTE keys to 32 bytes by duplication
- Extends 4-byte IV to 8 bytes by duplication
- XORs block index with first 4 bytes of extended IV
- Uses `salsa20 = "0.10"` crate

#### ARC4 Implementation
- Creates combined key: base_key + IV + block_index (as LE bytes)
- Pads to exactly 32 bytes with zeros
- Uses `rc4 = "0.1"` crate
- **This is the ONLY implementation found with ARC4 support**

#### Encryption Block Format
```
1 byte:  encoding type (0x45 'E')
8 bytes: key name size (always 8)
8 bytes: key name (little-endian u64)
4 bytes: IV size (always 4)
4 bytes: IV data
1 byte:  encryption type (0x53 = Salsa20, 0x41 = ARC4)
N bytes: encrypted data
```

### 2. Comprehensive Key Service

**Location**: `crates/cascette-formats/src/key_service.rs`

Features:
- 13+ hardcoded WoW encryption keys
- Multi-format key file support (CSV, space-separated, tab-separated, equals-separated)
- Standard directory searching:
  - Config directories (`~/.config/cascette`, `~/.config/TactKeys`)
  - Data directories
  - Home directory (`~/.cascette`, `~/.tactkeys`)
  - Current directory
- Graceful fallback mechanisms

Known keys included:
```
0xFA505078126ACB3E: BDC51862ABED79B2DE48C8E7E66C6200
0xFF813F7D062AC0BC: AA0B5C77F088CCC2D39049BD267F066D
0xD1E9B5EDF9283668: 8E4A2579684E341081FFF96BC5B0FDFA
... (10+ more keys)
```

### 3. TVFS Parser Implementation

**Location**: `crates/cascette-formats/src/tvfs.rs`

Complete TVFS implementation with:
- Header parsing (magic: 0x54564653 "TVFS")
- Path table entries with flags
- VFS table with span information
- Container File Table (CFT)
- Support for patches and encoding specifications

Key structures:
- `TVFSHeader` - 44+ byte header with offsets
- `PathTableEntry` - Variable-length path entries
- `VfsRootEntry` - Root entries with encoding keys

### 4. Complete File Format Parsers

#### Encoding File Parser
**Location**: `crates/cascette-formats/src/encoding.rs`
- Parses EN header (magic: 0x45 0x4E)
- Content key to encoding key mappings
- Page-based structure support
- Reverse lookup tables

#### Build Config Parser
**Location**: `crates/cascette-formats/src/build_config.rs`
- Key-value pair parsing
- Hash pair extraction
- Validation and error handling
- Support for all standard fields

#### CDN Config Parser
**Location**: `crates/cascette-formats/src/cdn_config.rs`
- Archive list parsing
- Index information extraction
- Path configuration

#### Install Manifest Parser
**Location**: `crates/cascette-formats/src/install_manifest.rs`
- Complete tag system implementation
- Platform/locale/architecture tags
- File entry parsing with bitmasks
- Size and hash tracking

### 5. Product Installer

**Location**: `crates/cascette-formats/src/installer.rs`

Complete installation system with:
- Progress tracking via `indicatif`
- Multi-stage download process
- Manifest-based file selection
- Tag filtering (platform, locale)
- Cache-aware downloading
- Error recovery

### 6. Advanced Features

#### Listfile Support
**Location**: `crates/cascette-formats/src/listfile.rs`
- Community listfile integration
- File type detection (DB2, BLP, etc.)
- Path normalization
- Efficient lookups

#### VFS Implementation
**Location**: `crates/cascette-formats/src/vfs.rs`
- Modern WoW build support
- Virtual file system abstraction
- Content resolution

#### Cache-Aware CDN Client
**Location**: `crates/cascette-cdn-client/src/cached_cdn_client.rs`
- Local cache checking
- Progress tracking with cache awareness
- Bandwidth optimization

## Dependencies Required

From prototype's `Cargo.toml`:
```toml
# Encryption
salsa20 = "0.10"
rc4 = "0.1"
cipher = "0.4"
generic-array = "0.14"

# Parsing
byteorder = "1.5"
nom = "8.0"
nom-derive = "0.10"

# Progress/UI
indicatif = "0.17"

# Utilities
dirs = "6.0"
chrono = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Testing Coverage

The prototype includes extensive tests:
- BLTE encryption/decryption tests
- Key service validation tests
- Install manifest parsing tests
- TVFS integration tests
- Build config parsing tests
- End-to-end installation tests

## Implementation Priority

Based on the prototype analysis, implementation priority should be:

1. **Critical** (Blocks all functionality):
   - BLTE decompression with encryption support
   - Key service with hardcoded keys

2. **High** (Core functionality):
   - Encoding file parser
   - Build/CDN config parsers
   - Install manifest parser

3. **Medium** (Modern support):
   - TVFS parser
   - VFS implementation
   - Listfile support

4. **Low** (Nice-to-have):
   - Product installer
   - Cache-aware CDN
   - Progress tracking

## Architectural Differences

### Prototype Structure
- Multi-crate workspace with fine-grained separation
- Heavy use of async/await throughout
- Trait-based abstractions in core

### Main Project Structure
- Cleaner, more user-friendly organization
- Clear module separation
- Better documentation structure

### Recommendation
Use the main project's structure but implement the prototype's functionality, using the prototype as a reference implementation rather than directly merging code.

## Key Implementation Insights

1. **Encryption Key Extension**: The prototype reveals that BLTE uses 16-byte keys but Salsa20 needs 32 bytes - solved by duplication
2. **ARC4 Key Construction**: Combines base key + IV + block index, then pads to 32 bytes
3. **TVFS Complexity**: Full implementation requires handling path tables, VFS tables, and CFT tables
4. **Tag System**: Install manifests use bitmasks for efficient tag assignment
5. **Progress Tracking**: Essential for user experience during large downloads

## Missing from Main Project

Comparing to the main cascette-rs project, the prototype adds:
- Complete BLTE encryption (Salsa20 + ARC4)
- Comprehensive key management
- TVFS support
- Install manifest parsing
- Product installation logic
- Listfile integration
- VFS for modern builds
- Cache-aware downloading

## Validation Approach

The prototype can serve as a reference for validating our implementation:
1. Compare BLTE decryption output
2. Verify key service behavior
3. Cross-check parsed file structures
4. Validate installation file lists
5. Test encryption/decryption round-trips
# TACT File Formats - Complete Binary Specifications

## Overview

This document provides exact binary specifications for all TACT file formats, including header structures, field layouts, and parsing algorithms.

## Encoding File

### Header Structure (22 bytes)

```c
struct EncodingHeader {
    char     magic[2];                    // "EN" (0x45 0x4E)
    uint8_t  version;                      // Must be 1
    uint8_t  CHashSize;                    // Content hash size (typically 16)
    uint8_t  EHashSize;                    // Encoding hash size (typically 16)
    uint16_t CEKeyPageTablePageSizeKB;    // Big-endian! CEKey page size in KB
    uint16_t EKeySpecPageTablePageSizeKB; // Big-endian! ESpec page size in KB
    uint32_t CEKeyPageTablePageCount;     // Big-endian! Number of CEKey pages
    uint32_t EKeySpecPageTablePageCount;  // Big-endian! Number of ESpec pages
    uint8_t  flags;                        // Must be 0
    uint32_t ESpecBlockSize;              // Big-endian! Size of ESpec block
};
```

### CEKey Page Table

```c
struct CEKeyPageEntry {
    uint8_t  firstKey[CHashSize];  // First CKey in page (for binary search)
    uint8_t  checksum[16];          // MD5 of page content
};

struct CEKeyEntry {
    uint8_t  keyCount;              // Number of EKeys (encoding variants)
    uint40_t fileSize;              // 5 bytes! Uncompressed file size
    uint8_t  CKey[CHashSize];       // Content hash
    uint8_t  EKeys[EHashSize * keyCount]; // Encoding keys array
};
```

### EKeySpec Page Table

```c
struct EKeySpecPageEntry {
    uint8_t  firstKey[EHashSize];  // First EKey in page
    uint8_t  checksum[16];          // MD5 of page content
};

struct EKeySpecEntry {
    uint8_t  EKey[EHashSize];       // Encoding key
    uint32_t ESpecIndex;            // Index into ESpec block
    uint40_t fileSize;              // 5 bytes! Compressed file size
};
```

### Parsing Algorithm

```python
def parse_encoding_file(data):
    header = EncodingHeader.from_bytes(data[0:22])
    offset = 22
    
    # Parse CEKey page table
    cekey_pages = []
    for i in range(header.CEKeyPageTablePageCount):
        page_entry = data[offset:offset+header.CHashSize+16]
        cekey_pages.append(page_entry)
        offset += header.CHashSize + 16
    
    # Parse CEKey pages
    page_size = header.CEKeyPageTablePageSizeKB * 1024
    for page in cekey_pages:
        page_data = data[offset:offset+page_size]
        # Verify checksum
        assert md5(page_data) == page.checksum
        # Parse entries in page
        parse_cekey_entries(page_data)
        offset += page_size
    
    # Similar for EKeySpec pages...
    
    # Parse ESpec block
    espec_block = data[offset:offset+header.ESpecBlockSize]
    espec_strings = parse_null_terminated_strings(espec_block)
```

## Root File

### Version 1 Format (WoW Classic)

```c
struct RootHeader_v1 {
    uint32_t magic;              // 'MFST' (0x5453464D)
    uint32_t version;            // 1
    uint32_t headerSize;
    uint32_t entryCount;
    uint32_t stringBlockSize;
};

struct RootEntry_v1 {
    uint32_t localeFlags;
    uint32_t contentFlags;
    uint32_t fileDataId;
    uint8_t  CKey[16];
    uint64_t nameHash;           // Jenkins hash of normalized path
};
```

### Version 2 Format (Modern WoW)

```c
struct RootHeader_v2 {
    char     magic[4];           // 'TSFM' (0x4D465354 reversed)
    uint32_t totalFileCount;
    uint32_t namedFileCount;
};

struct RootBlock {
    uint32_t localeFlags;
    uint32_t contentFlags;
    uint32_t fileDataIdCount;
    
    // Followed by array of:
    struct FileEntry {
        uint32_t fileDataIdDelta;  // Delta-encoded from previous
        uint8_t  CKey[16];
    } entries[fileDataIdCount];
};
```

### Content Flags

```c
enum ContentFlags {
    CF_WINDOWS    = 0x00000001,  // Install on Windows
    CF_MACOS      = 0x00000002,  // Install on macOS  
    CF_LOW_VIO    = 0x00000004,  // Low violence version
    CF_X86        = 0x00000008,  // x86 architecture
    CF_X64        = 0x00000010,  // x64 architecture
    CF_ARM64      = 0x00000020,  // ARM64 architecture
    CF_ENCRYPTED  = 0x00000040,  // File is encrypted
    CF_NOCOMPRESS = 0x00000080,  // No compression
    CF_CHINESE    = 0x00000100,  // Chinese localization
    CF_ENGLISH_SPEECH = 0x00000200,
    CF_ENGLISH_TEXT   = 0x00000400,
    CF_FRENCH         = 0x00000800,
    CF_GERMAN         = 0x00001000,
    CF_SPANISH        = 0x00002000,
    CF_RUSSIAN        = 0x00004000,
    CF_JAPANESE       = 0x00008000,
    CF_PORTUGUESE     = 0x00010000,
    CF_ITALIAN        = 0x00020000,
};
```

## Install File

### Header Structure

```c
struct InstallHeader {
    char     magic[2];           // 'IN' (0x4E49)
    uint8_t  version;            // 1
    uint8_t  hashSize;           // Hash size (16 for MD5)
    uint16_t tagCount;           // Big-endian! Number of tags
    uint32_t entryCount;         // Big-endian! Number of entries
};

struct InstallTag {
    char     name[];             // Null-terminated string
    uint16_t type;               // Big-endian! Tag type
    // Followed by ceil(entryCount / 8) bytes of bitmask
};

struct InstallEntry {
    char     name[];             // Null-terminated file path
    uint8_t  CKey[hashSize];     // Content hash
    uint32_t size;               // Uncompressed size
};
```

### Tag Types

```c
enum TagType {
    TAG_PLATFORM     = 1,  // Windows, Mac, Linux
    TAG_ARCHITECTURE = 2,  // x86, x64, ARM
    TAG_LOCALE       = 3,  // Language/region
    TAG_REGION       = 4,  // Game region
    TAG_CATEGORY     = 5,  // Content category
};
```

## Download File

### Version 1 Header

```c
struct DownloadHeader_v1 {
    char     magic[2];           // 'DL' (0x4C44)
    uint8_t  version;            // 1
    uint8_t  EKeySize;           // Encoding key size
    uint8_t  hasChecksum;        // Checksum flag
    uint32_t entryCount;         // Number of entries
    uint16_t tagCount;           // Number of tags
};

struct DownloadEntry_v1 {
    uint8_t  EKey[EKeySize];     // Encoding key
    uint40_t fileSize;           // 5 bytes! Compressed size
    uint8_t  priority;           // Download priority
};
```

### Version 2/3 Changes

```c
struct DownloadHeader_v2 {
    // ... v1 fields ...
    uint8_t  flagSize;           // Size of flags field
};

struct DownloadEntry_v2 {
    // ... v1 fields ...
    uint8_t  flags[flagSize];    // Additional flags
    uint8_t  checksum[16];       // Optional MD5
};
```

## Size File

### Header Structure

```c
struct SizeHeader {
    char     magic[2];           // 'SP' (0x5053)
    uint8_t  version;            // 1 or 2
    uint8_t  EKeySize;           // Encoding key size
    uint32_t entryCount;         // Number of entries
};

struct SizeEntry_v1 {
    uint8_t  EKey[EKeySize];     // Encoding key
    uint32_t fileSize;           // Compressed size
};

struct SizeEntry_v2 {
    uint8_t  EKey[EKeySize];     // Encoding key
    uint32_t compressedSize;     // Compressed size
    uint32_t uncompressedSize;   // Uncompressed size
};
```

## Patch File

### Header Structure

```c
struct PatchHeader {
    char     magic[2];           // 'PA' (0x4150)
    uint8_t  version;            // 1
    uint8_t  EKeySize;           // Encoding key size
    uint8_t  patchKeySize;       // Patch key size
    uint32_t entryCount;         // Number of patches
    uint8_t  blockSizeBits;      // Block size = 2^bits
};

struct PatchEntry {
    uint8_t  oldEKey[EKeySize];  // Source file EKey
    uint8_t  newEKey[EKeySize];  // Target file EKey
    uint8_t  patchKey[patchKeySize]; // Patch data key
    uint32_t patchSize;          // ZBSDIFF patch size
};
```

### ZBSDIFF Format

```c
struct ZBSDIFF1Header {
    char     magic[8];           // "ZBSDIFF1"
    uint64_t ctrlBlockLength;    // Control block size
    uint64_t diffBlockLength;    // Diff block size
    uint64_t newFileLength;      // Target file size
};

struct ControlEntry {
    uint64_t diffBytes;          // Bytes from diff block
    uint64_t extraBytes;         // Bytes from extra block
    int64_t  seekAdjustment;     // Seek offset in old file
};
```

## TVFS (TACT Virtual File System)

### Header Structure

```c
struct TVFSHeader {
    char     magic[4];           // 'TVFS'
    uint8_t  version;            // 1
    uint8_t  headerSize;         // >= 0x26 (38 bytes)
    uint8_t  EKeySize;           // Encoding key size
    uint8_t  PKeySize;           // Patch key size
    uint8_t  flags;              // FileManifestFlags
    uint8_t  pathTableOffset;    // 5-byte offset
    uint8_t  pathTableSize;      // 5-byte size
    uint8_t  vfsTableOffset;     // 5-byte offset
    uint8_t  vfsTableSize;       // 5-byte size
    uint8_t  cftTableOffset;     // 5-byte offset
    uint8_t  cftTableSize;       // 5-byte size
    uint16_t maxMetafileSize;    // Max metafile size
    uint32_t buildVersion;       // Build version
    uint8_t  rootCKey[16];       // Optional root CKey
    uint8_t  patchEKey[9];       // Optional patch EKey
};

enum FileManifestFlags {
    FMF_INCLUDE_CKEY   = 0x01,  // Include CKey in records
    FMF_WRITE_SUPPORT  = 0x02,  // Enable write support
    FMF_PATCH_SUPPORT  = 0x04,  // Include patch records
    FMF_LOWERCASE      = 0x08,  // Lowercase paths
};
```

### Path Table Entry

```c
struct PathEntry {
    uint8_t  type;               // Entry type
    uint8_t  pathLength;         // Variable-length encoded
    char     path[pathLength];   // Path string
    uint32_t nodeIndex;          // Variable-length encoded
};

enum EntryType {
    ENTRY_NONE     = 0,
    ENTRY_FILE     = 1,
    ENTRY_DELETED  = 2,
    ENTRY_INLINE   = 3,
    ENTRY_LINK     = 4,
};
```

## Index Files

### Version 5 Format (Legacy)

```c
struct IndexHeader_v5 {
    uint32_t signature;          // 0x96C5DCA5
    uint32_t version;            // 5
    uint8_t  bucketIndex;        // Bucket number (0-15)
    uint8_t  reserved[3];
    uint32_t entriesSize;
    uint32_t entriesHash;        // Jenkins hash
};

struct IndexEntry_v5 {
    uint8_t  EKey[9];            // Truncated encoding key
    uint8_t  indexHigh;          // High byte of archive index
    uint32_t indexLow:14;        // Low 14 bits of archive index
    uint32_t offset:30;          // File offset in archive
    uint32_t size;               // Compressed size
};
```

### Version 7 Format (Modern)

```c
struct IndexHeader_v7 {
    uint32_t signature;          // Different per version
    uint32_t version;            // 7
    uint8_t  bucketIndex;
    uint8_t  storageType;        // Archive, loose, etc.
    uint32_t archiveSize[8];     // Size information
    uint64_t offsetBytes;        // Padding information
};

struct IndexEntry_v7 {
    uint8_t  EKey[9];            // Truncated key
    uint40_t offset;             // 5 bytes! Archive offset
    uint32_t size;               // Compressed size
};
```

## Parsing Utilities

### Variable-Length Integer

```python
def read_variable_int(data, offset):
    """Read variable-length integer (1-5 bytes)"""
    result = 0
    bytes_read = 0
    
    while True:
        byte = data[offset + bytes_read]
        result |= (byte & 0x7F) << (7 * bytes_read)
        bytes_read += 1
        
        if (byte & 0x80) == 0:
            break
            
        if bytes_read >= 5:
            raise ValueError("Variable int too long")
    
    return result, bytes_read
```

### 40-bit Integer

```python
def read_uint40(data, offset):
    """Read 5-byte (40-bit) integer"""
    return (data[offset] |
            (data[offset+1] << 8) |
            (data[offset+2] << 16) |
            (data[offset+3] << 24) |
            (data[offset+4] << 32))
```

### Jenkins Hash

```python
def jenkins_hash(data):
    """Compute Jenkins hash (lookup3)"""
    a = b = c = 0xdeadbeef + len(data)
    
    # Process 12-byte chunks
    i = 0
    while i + 12 <= len(data):
        a += unpack('<I', data[i:i+4])[0]
        b += unpack('<I', data[i+4:i+8])[0]
        c += unpack('<I', data[i+8:i+12])[0]
        
        # Mix
        a, b, c = jenkins_mix(a, b, c)
        i += 12
    
    # Handle remaining bytes
    if i < len(data):
        a += unpack_partial(data[i:])
        # Final mix
        a, b, c = jenkins_final(a, b, c)
    
    return ((c << 32) | b) & 0xFFFFFFFFFFFFFFFF
```

## Rust Implementation Guidelines

```rust
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io::{Read, Cursor};

pub trait TactFormat: Sized {
    fn parse(data: &[u8]) -> Result<Self, ParseError>;
    fn validate(&self) -> Result<(), ValidationError>;
}

impl TactFormat for EncodingFile {
    fn parse(data: &[u8]) -> Result<Self, ParseError> {
        let mut cursor = Cursor::new(data);
        
        // Read header
        let mut magic = [0u8; 2];
        cursor.read_exact(&mut magic)?;
        
        if magic != b"EN" {
            return Err(ParseError::InvalidMagic);
        }
        
        let version = cursor.read_u8()?;
        if version != 1 {
            return Err(ParseError::UnsupportedVersion(version));
        }
        
        // Continue parsing...
        Ok(EncodingFile { /* fields */ })
    }
}
```
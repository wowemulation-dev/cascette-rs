# Reference Implementations - Critical Insights

## Overview

This document captures unique insights and implementation details from ALL reference implementations that enhance our understanding of NGDP/TACT/CASC.

## CascLib v2 (Ladislav Zezula) - C++ Reference

### Unique Insights

#### 1. Extensive Key Database (100+ keys)
CascLib contains the most comprehensive key collection:
```c++
// From CascDecrypt.cpp
static CASC_ENCRYPTION_KEY CascEncryptionKeys[] = {
    {0xFA505078126ACB3E, "BDC51862ABED79B2DE48C8E7E66C6200"},  // BfA
    {0xFF813F7D062AC0BC, "AA0B5C77F088CCC2D39049BD267F066D"},  // SL
    // ... 100+ more keys including unreleased builds
    {0x2C547F26A2613E01, "37C50C102D4C9E3A5AC069F072B1417D"},  // 10.0.7
};
```

#### 2. Game-Specific Optimizations
```c++
// Different handling for different games
if(IsWoWBuild(BuildNumber)) {
    // WoW-specific root file parsing
    ParseWoWRootFile();
} else if(IsOverwatchBuild(BuildNumber)) {
    // Overwatch uses different format
    ParseOverwatchRootFile();
} else if(IsDiabloBuild(BuildNumber)) {
    // Diablo has unique requirements
    ParseDiabloRootFile();
}
```

#### 3. Index File Version Handling
```c++
// Multiple index formats supported
switch(IndexVersion) {
    case 0x05:  // Legacy format
        ParseIndexV5();
        break;
    case 0x07:  // Modern format
        ParseIndexV7();
        break;
    case 0x09:  // Latest format
        ParseIndexV9();
        break;
}
```

#### 4. Error Recovery Mechanisms
```c++
// Retry with different methods on failure
if(!TryMethodA()) {
    if(!TryMethodB()) {
        // Fall back to brute force
        BruteForceSearch();
    }
}
```

#### 5. Platform-Specific Code
```c++
#ifdef _WIN32
    // Windows: Use native file APIs for better performance
    hFile = CreateFile(...);
#else
    // Unix: Use memory-mapped files
    mmap(...);
#endif
```

## TACT.Net - C# Reference

### Unique Insights

#### 1. Async/Await Pattern Throughout
```csharp
public async Task<byte[]> DownloadFileAsync(string hash) {
    // All I/O operations are async
    var data = await httpClient.GetByteArrayAsync(url);
    var decrypted = await DecryptAsync(data);
    return await DecompressAsync(decrypted);
}
```

#### 2. Span<T> for Performance
```csharp
// Modern C# using Span for zero-copy operations
public static void ParseHeader(ReadOnlySpan<byte> data) {
    var magic = data.Slice(0, 4);
    var version = data[4];
    // No allocations
}
```

#### 3. Certificate Validation Chain
```csharp
// Complete X.509 validation
X509Chain chain = new X509Chain();
chain.ChainPolicy.RevocationMode = X509RevocationMode.Online;
chain.ChainPolicy.RevocationFlag = X509RevocationFlag.EntireChain;
bool isValid = chain.Build(certificate);
```

#### 4. Structured Logging
```csharp
logger.LogInformation("Downloading file {Hash} from {CDN}", 
    hash, cdnUrl);
// Structured data for analysis
```

## TACTSharp - High-Performance C#

### Unique Insights

#### 1. Memory-Mapped Files for Large Data
```csharp
using var mmf = MemoryMappedFile.CreateFromFile(path);
using var accessor = mmf.CreateViewAccessor();
// Direct memory access without loading entire file
```

#### 2. Unsafe Code for Speed
```csharp
unsafe {
    fixed (byte* ptr = data) {
        // Direct pointer manipulation
        var header = *(BLTEHeader*)ptr;
    }
}
```

#### 3. ArrayPool for Memory Reuse
```csharp
byte[] buffer = ArrayPool<byte>.Shared.Rent(size);
try {
    // Use buffer
} finally {
    ArrayPool<byte>.Shared.Return(buffer);
}
```

## blizztools - Rust Implementation

### Unique Insights

#### 1. Strong Type System Usage
```rust
#[derive(Debug, Clone, Copy)]
pub struct ContentKey([u8; 16]);

#[derive(Debug, Clone, Copy)]
pub struct EncodingKey([u8; 16]);

// Prevents mixing up key types
```

#### 2. Error Handling with Context
```rust
data.get(offset..offset + 4)
    .ok_or(Error::TruncatedData)
    .and_then(|slice| slice.try_into()
        .map_err(|_| Error::InvalidSize))
    .map(u32::from_be_bytes)?;
```

#### 3. Iterator-Based Processing
```rust
entries.into_iter()
    .filter(|e| e.has_tag(Tag::Windows))
    .map(|e| e.decode())
    .collect::<Result<Vec<_>, _>>()?
```

## rustycasc - Rust CASC Implementation

### Unique Insights

#### 1. Trait-Based Abstraction
```rust
trait Storage {
    fn read(&self, key: &EncodingKey) -> Result<Vec<u8>>;
    fn write(&mut self, key: &EncodingKey, data: &[u8]) -> Result<()>;
}

// Multiple implementations
struct LocalStorage;
struct RemoteStorage;
struct CachedStorage<S: Storage>(S);
```

#### 2. Zero-Copy Parsing with nom
```rust
use nom::{
    IResult,
    bytes::complete::tag,
    number::complete::be_u32,
};

fn parse_header(input: &[u8]) -> IResult<&[u8], Header> {
    let (input, _) = tag(b"BLTE")(input)?;
    let (input, size) = be_u32(input)?;
    Ok((input, Header { size }))
}
```

## Ribbit.NET - Protocol Focus

### Unique Insights

#### 1. Retry with Exponential Backoff
```csharp
int retryCount = 0;
TimeSpan delay = TimeSpan.FromSeconds(1);

while (retryCount < MaxRetries) {
    try {
        return await SendRequest();
    } catch {
        await Task.Delay(delay);
        delay = TimeSpan.FromSeconds(Math.Pow(2, ++retryCount));
    }
}
```

#### 2. MIME Multipart Parsing
```csharp
var multipart = await MultipartReader.ReadAsync(stream);
foreach (var section in multipart.Sections) {
    if (section.ContentType == "application/octet-stream") {
        // Process signature
    }
}
```

## CascLib v1 (Original) - Historical Insights

### Unique Features

#### 1. MPQ Compatibility Layer
```c
// Support for legacy MPQ files
if (IsMPQFile(filename)) {
    return OpenMPQArchive(filename);
} else {
    return OpenCASCStorage(filename);
}
```

#### 2. Direct WinAPI Usage
```c
// Windows-specific optimizations
HANDLE hFile = CreateFileW(
    filename,
    GENERIC_READ,
    FILE_SHARE_READ,
    NULL,
    OPEN_EXISTING,
    FILE_FLAG_SEQUENTIAL_SCAN | FILE_FLAG_OVERLAPPED,
    NULL
);
```

## Cross-Implementation Patterns

### Common Patterns Found

1. **Checksum Verification at Every Level**
   - All implementations verify MD5 at multiple points
   - BLTE chunk checksums always validated
   - Page checksums in encoding files

2. **Fallback Mechanisms**
   - Try primary CDN → fallback CDN → cache
   - Try fast lookup → binary search → linear scan
   - Try with key → try without → fail gracefully

3. **Memory Management Strategies**
   - C++: Manual with RAII
   - C#: Span<T> and ArrayPool
   - Rust: Ownership and borrowing

4. **Async I/O**
   - All modern implementations use async
   - Parallel chunk processing common
   - Stream-based processing for large files

## Critical Implementation Details

### 1. Block Index Modification (All Implementations)

Every implementation modifies IV/nonce with block index:
```
// Consistent across all:
IV[0] ^= (block_index >> 0) & 0xFF
IV[1] ^= (block_index >> 8) & 0xFF
IV[2] ^= (block_index >> 16) & 0xFF
IV[3] ^= (block_index >> 24) & 0xFF
```

### 2. Key Extension Methods

**Salsa20**: All implementations duplicate 16→32 bytes
**ARC4**: Only prototype has working implementation

### 3. Error Codes

Common error conditions across implementations:
- `KEY_NOT_FOUND`: Encryption key missing
- `INVALID_SIGNATURE`: Signature verification failed
- `TRUNCATED_DATA`: Unexpected end of data
- `CHECKSUM_MISMATCH`: MD5 verification failed
- `UNSUPPORTED_VERSION`: Unknown format version

### 4. Performance Optimizations

All high-performance implementations use:
- Memory-mapped files for >10MB files
- Connection pooling for HTTP
- Parallel decompression for multi-chunk
- LRU cache for frequently accessed files

## Implementation Recommendations

### Based on All References

1. **Use Strong Types** (from Rust implementations)
   - Separate types for different keys
   - Newtype pattern for safety

2. **Implement Fallback Chain** (from CascLib)
   - Multiple CDN attempts
   - Cache fallback
   - Graceful degradation

3. **Add Game-Specific Handlers** (from CascLib)
   - WoW root format
   - Overwatch variations
   - Diablo specifics

4. **Use Modern Async** (from TACT.Net)
   - Async throughout
   - Cancellation support
   - Progress reporting

5. **Optimize with Unsafe** (from TACTSharp)
   - When performance critical
   - With safe wrappers
   - Only after profiling

6. **Comprehensive Key Database** (from CascLib)
   - Include all known keys
   - Regular updates
   - Multiple sources

7. **Platform Optimizations** (from CascLib)
   - Windows: Native APIs
   - Linux: splice/sendfile
   - All: Memory-mapped files

## Testing Strategies

### From Reference Implementations

1. **Known Test Vectors** (TACT.Net)
   - Specific file hashes with expected output
   - Cross-implementation validation

2. **Fuzzing** (rustycasc)
   - Malformed input handling
   - Truncated data recovery

3. **Performance Benchmarks** (TACTSharp)
   - Large file handling
   - Parallel processing gains

4. **Integration Tests** (CascLib)
   - Full game installation
   - Update scenarios
   - Repair operations

## Unique Features by Implementation

| Implementation | Unique Feature | Should Adopt? |
|---------------|----------------|---------------|
| CascLib v2 | 100+ hardcoded keys | Yes |
| CascLib v2 | Game-specific handlers | Yes |
| TACT.Net | Full async/await | Yes |
| TACTSharp | Memory-mapped files | Yes |
| TACTSharp | Unsafe optimizations | Maybe |
| blizztools | Strong type system | Yes |
| rustycasc | Trait abstractions | Yes |
| Prototype | ARC4 encryption | Yes |
| Prototype | Complete installers | Yes |

## Conclusion

Each reference implementation offers unique insights:
- **CascLib**: Most complete, production-tested
- **TACT.Net**: Best async patterns
- **TACTSharp**: Best performance optimizations
- **Rust impls**: Best type safety
- **Prototype**: Most complete Rust implementation

Combining the best from each creates an optimal implementation.
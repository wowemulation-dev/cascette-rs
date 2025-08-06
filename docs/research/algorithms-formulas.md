# Algorithms and Formulas - Complete Technical Reference

## Jenkins Hash (Lookup3)

### Complete Implementation

```c
#define rot(x,k) (((x)<<(k)) | ((x)>>(32-(k))))

#define mix(a,b,c) \
{ \
  a -= c;  a ^= rot(c, 4);  c += b; \
  b -= a;  b ^= rot(a, 6);  a += c; \
  c -= b;  c ^= rot(b, 8);  b += a; \
  a -= c;  a ^= rot(c,16);  c += b; \
  b -= a;  b ^= rot(a,19);  a += c; \
  c -= b;  c ^= rot(b, 4);  b += a; \
}

#define final(a,b,c) \
{ \
  c ^= b; c -= rot(b,14); \
  a ^= c; a -= rot(c,11); \
  b ^= a; b -= rot(a,25); \
  c ^= b; c -= rot(b,16); \
  a ^= c; a -= rot(c,4);  \
  b ^= a; b -= rot(a,14); \
  c ^= b; c -= rot(b,24); \
}
```

### TACT Path Normalization

```python
def normalize_path_for_hash(path):
    """
    Normalize path for Jenkins hash computation
    
    Rules:
    1. Convert to uppercase
    2. Replace forward slashes with backslashes
    3. Remove leading/trailing slashes
    """
    normalized = path.upper()
    normalized = normalized.replace('/', '\\')
    normalized = normalized.strip('\\')
    return normalized

def jenkins_hash_path(path):
    """Compute Jenkins hash for file path"""
    # Normalize path
    normalized = normalize_path_for_hash(path)
    data = normalized.encode('utf-8')
    
    # Initialize
    length = len(data)
    a = b = c = 0xdeadbeef + length
    
    offset = 0
    
    # Main loop - process 12-byte chunks
    while length > 12:
        a += (data[offset+0] | (data[offset+1] << 8) | 
              (data[offset+2] << 16) | (data[offset+3] << 24))
        b += (data[offset+4] | (data[offset+5] << 8) | 
              (data[offset+6] << 16) | (data[offset+7] << 24))
        c += (data[offset+8] | (data[offset+9] << 8) | 
              (data[offset+10] << 16) | (data[offset+11] << 24))
        
        # Mix
        a, b, c = mix(a, b, c)
        
        offset += 12
        length -= 12
    
    # Handle remaining bytes
    if length == 12:
        c += (data[offset+8] | (data[offset+9] << 8) | 
              (data[offset+10] << 16) | (data[offset+11] << 24))
        b += (data[offset+4] | (data[offset+5] << 8) | 
              (data[offset+6] << 16) | (data[offset+7] << 24))
        a += (data[offset+0] | (data[offset+1] << 8) | 
              (data[offset+2] << 16) | (data[offset+3] << 24))
    elif length == 11:
        c += (data[offset+8] | (data[offset+9] << 8) | 
              (data[offset+10] << 16))
        b += (data[offset+4] | (data[offset+5] << 8) | 
              (data[offset+6] << 16) | (data[offset+7] << 24))
        a += (data[offset+0] | (data[offset+1] << 8) | 
              (data[offset+2] << 16) | (data[offset+3] << 24))
    # ... handle all cases 1-10 ...
    
    # Final mixing
    a, b, c = final(a, b, c)
    
    # Return 64-bit hash
    return ((c & 0xFFFFFFFF) << 32) | (b & 0xFFFFFFFF)
```

### Hash Table Lookup

```python
def lookup_by_hash(hash_table, target_hash):
    """
    Binary search in sorted hash table
    
    Used in root files for name resolution
    """
    left, right = 0, len(hash_table) - 1
    
    while left <= right:
        mid = (left + right) // 2
        entry = hash_table[mid]
        
        if entry.name_hash == target_hash:
            return entry
        elif entry.name_hash < target_hash:
            left = mid + 1
        else:
            right = mid - 1
    
    return None
```

## XOR-Based Bucket Assignment

### Index Bucket Calculation

```python
def get_bucket_index(ekey):
    """
    Calculate bucket index for CASC index files
    
    Uses XOR of all key bytes
    Results in 0-15 (16 buckets)
    """
    bucket = 0
    for byte in ekey:
        bucket ^= byte
    return bucket & 0x0F

def get_index_filename(ekey):
    """Get index file name for a given EKey"""
    bucket = get_bucket_index(ekey)
    return f"{bucket:02x}.idx"
```

## File Size Encoding

### 40-bit Integer Encoding

```python
def encode_uint40(value):
    """Encode 40-bit (5-byte) integer"""
    if value >= (1 << 40):
        raise ValueError("Value too large for 40 bits")
    
    return bytes([
        (value >> 0) & 0xFF,
        (value >> 8) & 0xFF,
        (value >> 16) & 0xFF,
        (value >> 24) & 0xFF,
        (value >> 32) & 0xFF,
    ])

def decode_uint40(data):
    """Decode 40-bit (5-byte) integer"""
    return (data[0] | 
            (data[1] << 8) |
            (data[2] << 16) |
            (data[3] << 24) |
            (data[4] << 32))
```

### Variable-Length Integer Encoding

```python
def encode_varint(value):
    """
    Encode variable-length integer (1-5 bytes)
    
    Used in TVFS and other formats
    """
    result = bytearray()
    
    while value >= 0x80:
        result.append((value & 0x7F) | 0x80)
        value >>= 7
    
    result.append(value & 0x7F)
    return bytes(result)

def decode_varint(data, offset=0):
    """Decode variable-length integer"""
    result = 0
    shift = 0
    
    while True:
        byte = data[offset]
        result |= (byte & 0x7F) << shift
        offset += 1
        
        if (byte & 0x80) == 0:
            break
            
        shift += 7
        if shift >= 35:  # Max 5 bytes
            raise ValueError("Varint too long")
    
    return result, offset
```

## Delta Encoding

### File ID Delta Compression

```python
def encode_file_ids_delta(file_ids):
    """
    Delta-encode sorted file IDs
    
    Used in root files to compress file ID lists
    """
    if not file_ids:
        return []
    
    # Sort first
    sorted_ids = sorted(file_ids)
    
    # Encode as deltas
    encoded = [sorted_ids[0]]  # First ID as-is
    
    for i in range(1, len(sorted_ids)):
        delta = sorted_ids[i] - sorted_ids[i-1]
        encoded.append(delta)
    
    return encoded

def decode_file_ids_delta(deltas):
    """Decode delta-encoded file IDs"""
    if not deltas:
        return []
    
    file_ids = [deltas[0]]
    
    for i in range(1, len(deltas)):
        file_id = file_ids[-1] + deltas[i]
        file_ids.append(file_id)
    
    return file_ids
```

## Binary Search in Pages

### Encoding File Page Lookup

```python
def find_in_encoding_file(encoding_file, target_ckey):
    """
    Binary search in paged encoding file
    
    Optimized for large files with sorted pages
    """
    # Binary search for correct page
    page_index = binary_search_pages(
        encoding_file.ce_key_pages,
        target_ckey
    )
    
    if page_index < 0:
        return None
    
    # Load page
    page = load_page(encoding_file, page_index)
    
    # Linear search within page
    for entry in page.entries:
        if entry.ckey == target_ckey:
            return entry.ekeys
    
    return None

def binary_search_pages(pages, target_key):
    """Binary search in page table"""
    left, right = 0, len(pages) - 1
    
    while left <= right:
        mid = (left + right) // 2
        page = pages[mid]
        
        # Check if target is in this page
        if page.first_key <= target_key:
            if mid == len(pages) - 1 or pages[mid + 1].first_key > target_key:
                return mid
            left = mid + 1
        else:
            right = mid - 1
    
    return -1
```

## Compression Algorithms

### ZLib Configuration

```python
def compress_zlib_tact(data, level=9):
    """
    ZLib compression with TACT settings
    
    Window bits: -15 (raw deflate)
    Level: 9 (maximum)
    Strategy: DEFAULT_STRATEGY
    """
    import zlib
    
    compressor = zlib.compressobj(
        level=level,
        method=zlib.DEFLATED,
        wbits=-15,  # Raw deflate (no header)
        memLevel=9,
        strategy=zlib.Z_DEFAULT_STRATEGY
    )
    
    compressed = compressor.compress(data)
    compressed += compressor.flush()
    
    return compressed

def decompress_zlib_tact(data):
    """ZLib decompression with TACT settings"""
    return zlib.decompress(data, -15)
```

### LZ4 High Compression

```python
def compress_lz4hc(data):
    """
    LZ4HC compression for BLTE
    
    Uses high compression variant
    """
    import lz4.block
    
    compressed = lz4.block.compress(
        data,
        mode='high_compression',
        compression=12,  # Max compression level
        store_size=False  # Size stored separately in BLTE
    )
    
    return compressed
```

## Patch Algorithm (ZBSDIFF)

### Binary Diff Application

```python
def apply_zbsdiff_patch(old_data, patch_data):
    """
    Apply ZBSDIFF1 binary patch
    
    Based on bsdiff algorithm with zlib compression
    """
    # Parse header
    magic = patch_data[0:8]
    if magic != b'ZBSDIFF1':
        raise ValueError("Invalid ZBSDIFF magic")
    
    ctrl_len = struct.unpack('<Q', patch_data[8:16])[0]
    diff_len = struct.unpack('<Q', patch_data[16:24])[0]
    new_len = struct.unpack('<Q', patch_data[24:32])[0]
    
    # Decompress blocks
    offset = 32
    ctrl_block = zlib.decompress(patch_data[offset:offset+ctrl_len])
    offset += ctrl_len
    
    diff_block = zlib.decompress(patch_data[offset:offset+diff_len])
    offset += diff_len
    
    extra_block = zlib.decompress(patch_data[offset:])
    
    # Apply patch
    new_data = bytearray()
    old_pos = 0
    diff_pos = 0
    extra_pos = 0
    ctrl_pos = 0
    
    while ctrl_pos < len(ctrl_block):
        # Read control entry
        diff_bytes = struct.unpack('<Q', ctrl_block[ctrl_pos:ctrl_pos+8])[0]
        ctrl_pos += 8
        
        extra_bytes = struct.unpack('<Q', ctrl_block[ctrl_pos:ctrl_pos+8])[0]
        ctrl_pos += 8
        
        seek_offset = struct.unpack('<q', ctrl_block[ctrl_pos:ctrl_pos+8])[0]
        ctrl_pos += 8
        
        # Add diff bytes
        for i in range(diff_bytes):
            if old_pos < len(old_data) and diff_pos < len(diff_block):
                new_data.append((old_data[old_pos] + diff_block[diff_pos]) & 0xFF)
                old_pos += 1
                diff_pos += 1
        
        # Add extra bytes
        new_data.extend(extra_block[extra_pos:extra_pos+extra_bytes])
        extra_pos += extra_bytes
        
        # Seek in old file
        old_pos += seek_offset
    
    return bytes(new_data)
```

## Content Addressing

### Hash Chain Calculation

```python
def calculate_content_chain(data):
    """
    Calculate complete hash chain for content
    
    CKey -> EKey -> Archive location
    """
    # Content key (uncompressed data)
    ckey = hashlib.md5(data).digest()
    
    # Compress and encode
    encoded = encode_blte(data)
    
    # Encoding key (compressed data)
    ekey = hashlib.md5(encoded).digest()
    
    # Truncated key for indices (9 bytes)
    ekey_truncated = ekey[:9]
    
    # Bucket for index lookup
    bucket = get_bucket_index(ekey)
    
    return {
        'ckey': ckey,
        'ekey': ekey,
        'ekey_truncated': ekey_truncated,
        'bucket': bucket,
        'encoded_data': encoded
    }
```

## Performance Optimizations

### Memory-Mapped File Access

```python
import mmap

def parse_large_file_mmap(filepath):
    """
    Parse large files using memory mapping
    
    Used by TACTSharp for encoding files
    """
    with open(filepath, 'rb') as f:
        with mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ) as mmapped:
            # Parse header
            header = parse_header(mmapped[0:22])
            
            # Access pages without loading entire file
            for page_index in range(header.page_count):
                page_offset = calculate_page_offset(page_index)
                page_data = mmapped[page_offset:page_offset+header.page_size]
                process_page(page_data)
```

### Parallel Processing

```python
import concurrent.futures

def decompress_chunks_parallel(chunks):
    """
    Decompress BLTE chunks in parallel
    
    Significantly faster for multi-chunk files
    """
    with concurrent.futures.ThreadPoolExecutor() as executor:
        futures = []
        
        for i, chunk in enumerate(chunks):
            future = executor.submit(decompress_chunk, chunk, i)
            futures.append(future)
        
        results = []
        for future in concurrent.futures.as_completed(futures):
            results.append(future.result())
    
    # Sort by chunk index and concatenate
    results.sort(key=lambda x: x[0])
    return b''.join(r[1] for r in results)
```

## Rust Implementation Patterns

### Zero-Copy Parsing

```rust
use nom::{
    IResult,
    bytes::complete::{tag, take},
    number::complete::{be_u32, le_u32},
};

fn parse_blte_header(input: &[u8]) -> IResult<&[u8], BLTEHeader> {
    let (input, _) = tag(b"BLTE")(input)?;
    let (input, header_size) = be_u32(input)?;
    
    Ok((input, BLTEHeader { header_size }))
}
```

### SIMD Optimizations

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

unsafe fn jenkins_hash_simd(data: &[u8]) -> u64 {
    // Use SIMD instructions for faster hashing
    let mut a = _mm_set1_epi32(0xdeadbeef);
    let mut b = a;
    let mut c = a;
    
    // Process 16-byte chunks with SSE
    let chunks = data.chunks_exact(16);
    for chunk in chunks {
        let data = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
        // SIMD operations...
    }
    
    // Handle remainder
    // ...
    
    0 // Return hash
}
```

## Mathematical Constants

### Size Calculations

```python
# Common block sizes
ENCODING_PAGE_SIZE = 4 * 1024         # 4 KB
DEFAULT_CHUNK_SIZE = 256 * 1024       # 256 KB
MAX_CHUNK_SIZE = 16 * 1024 * 1024     # 16 MB
ARCHIVE_MAX_SIZE = 1024 * 1024 * 1024 # 1 GB

# Compression ratios (typical)
ZLIB_RATIO_TEXT = 0.3      # 70% reduction
ZLIB_RATIO_BINARY = 0.7    # 30% reduction
LZ4_RATIO_AVERAGE = 0.5     # 50% reduction

# Hash sizes
MD5_SIZE = 16               # 128 bits
SHA256_SIZE = 32            # 256 bits
TRUNCATED_KEY_SIZE = 9      # 72 bits
```

### Performance Metrics

```python
# Throughput targets
TARGET_DECOMPRESS_SPEED = 500 * 1024 * 1024  # 500 MB/s
TARGET_HASH_SPEED = 1000 * 1024 * 1024       # 1 GB/s
TARGET_DOWNLOAD_SPEED = 100 * 1024 * 1024    # 100 MB/s

# Latency targets
MAX_INDEX_LOOKUP_TIME = 0.001    # 1ms
MAX_CACHE_LOOKUP_TIME = 0.0001   # 0.1ms
MAX_NETWORK_LATENCY = 0.100      # 100ms
```
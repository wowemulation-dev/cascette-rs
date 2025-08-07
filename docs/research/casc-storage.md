# CASC Storage System

## Overview

CASC (Content Addressable Storage Container) is Blizzard's local storage system for game content. It provides efficient content-addressed storage with deduplication, compression, and fast access to game files.

## Directory Structure

### Standard Layout

```
Game Directory/
├── Data/
│   ├── data/                 # Content archives
│   │   ├── data.000         # Archive files
│   │   ├── data.001
│   │   └── ...
│   ├── indices/             # Index files
│   │   ├── {hash}.index     # Group indices
│   │   └── *.idx            # File indices
│   ├── config/              # Configuration cache
│   │   └── {hash}/          # Config by hash
│   └── patch/               # Patch archives
│       └── patch.000        # Patch files
├── Cache/                   # Temporary cache
└── Logs/                    # Debug logs
```

### Platform Variations

| Platform | Path | Case Sensitivity |
|----------|------|------------------|
| Windows | `C:\Program Files\World of Warcraft\Data` | No |
| macOS | `/Applications/World of Warcraft/Data` | Optional |
| Linux | `~/Games/WorldOfWarcraft/Data` | Yes |

## Index System

### Index Types

#### 1. Group Indices

Map encoding keys to archive locations.

**File Format**:
```c
struct GroupIndexHeader {
    uint16_t indexVersion;      // Version (usually 7)
    uint8_t bucketIndex;        // Bucket for this index
    uint8_t extraBytes;         // Extra bytes per entry
    uint8_t spanSizeBytes;      // Span size encoding
    uint8_t spanOffsBytes;      // Span offset encoding
    uint8_t ekeyBytes;         // EKey size (usually 9)
    uint8_t archiveBytes;       // Archive file encoding
    uint64_t archiveTotalSize;  // Total size of archives
};

struct GroupIndexEntry {
    uint8_t ekey[9];            // Truncated encoding key
    uint8_t archiveIndex;       // Archive file number
    uint32_t offset;            // Offset in archive
    uint32_t size;              // File size
};
```

**Bucket Assignment**:
```python
def get_bucket_index(ekey):
    # XOR-based hash for bucket distribution
    hash = ekey[0]
    for i in range(1, len(ekey)):
        hash ^= ekey[i]
    return hash & 0x0F  # 16 buckets
```

#### 2. File Indices

Direct file mapping for specific files.

**File Format**:
```c
struct FileIndexHeader {
    uint16_t version;           // Format version
    uint32_t dataVersion;       // Data version
    uint32_t fileCount;         // Number of files
};

struct FileIndexEntry {
    uint8_t ekey[16];          // Full encoding key
    uint64_t offset;           // Archive offset
    uint32_t size;             // File size
};
```

#### 3. Loose Files Index

Tracks individual files not in archives.

**Structure**:
```c
struct LooseFilesIndex {
    uint32_t version;
    uint32_t fileCount;
    struct {
        uint8_t ekey[16];
        char filename[256];     // Relative path
        uint32_t size;
    } entries[];
};
```

## Archive Format

### Archive Structure

Archives contain multiple BLTE-encoded files.

```c
struct ArchiveHeader {
    char magic[4];              // 'BLTE'
    uint32_t headerSize;        // Header size
    uint32_t fileCount;         // Number of files
};

struct ArchiveIndex {
    uint32_t blockCount;        // Number of blocks
    struct {
        uint8_t hash[16];       // Block hash
        uint32_t size;          // Block size
        uint32_t offset;        // Block offset
    } blocks[];
};
```

### Archive Naming

- Data archives: `data.{000-999}`
- Patch archives: `patch.{000-999}`
- Size limit: 1GB per archive (configurable)

## Content Storage

### Storage Strategy

1. **Content Addressing**:
   - Files identified by content hash
   - Deduplication across all files
   - Single storage of identical content

2. **Compression**:
   - BLTE encoding for all files
   - Multiple compression algorithms
   - Chunk-based for large files

3. **Archive Packing**:
   - Small files grouped in archives
   - Large files may span archives
   - Optimized for sequential access

### Write Process

```python
def store_file(content, ekey):
    # 1. Check if already exists
    if index.has_key(ekey):
        return  # Already stored
    
    # 2. Compress content
    compressed = blte_encode(content)
    
    # 3. Find or create archive
    archive = get_current_archive()
    if archive.size + len(compressed) > MAX_ARCHIVE_SIZE:
        archive = create_new_archive()
    
    # 4. Write to archive
    offset = archive.write(compressed)
    
    # 5. Update index
    bucket = get_bucket_index(ekey)
    index[bucket].add_entry(ekey, archive.id, offset, len(compressed))
```

### Read Process

```python
def read_file(ekey):
    # 1. Determine bucket
    bucket = get_bucket_index(ekey)
    
    # 2. Load index
    index = load_index(bucket)
    
    # 3. Find entry
    entry = index.find(ekey)
    if not entry:
        raise FileNotFoundError
    
    # 4. Read from archive
    archive = open_archive(entry.archive_id)
    compressed = archive.read(entry.offset, entry.size)
    
    # 5. Decompress
    return blte_decode(compressed)
```

## Shared Memory File

### Purpose

Coordinates access between game client and agent.

### Structure

```c
struct SharedMemoryHeader {
    uint32_t version;           // Format version
    uint32_t buildNumber;       // Game build
    char region[4];             // Game region
    uint32_t flags;             // Status flags
};

struct SharedMemoryData {
    uint32_t archiveCount;      // Number of archives
    uint32_t indexCount;        // Number of indices
    uint64_t totalSize;         // Total storage size
    uint32_t freeSpace;         // Available space
    char dataPath[256];         // Data directory path
};
```

## Version Differences

### CASC v1 (Heroes of the Storm Era)

- Package-based organization
- Direct filename mapping
- Simpler index structure

### CASC v2 (World of Warcraft)

- Content-addressed storage
- Name hash resolution
- Requires external listfile

### CASC v3 (Modern)

- Improved compression
- Better deduplication
- Streaming support

## Optimization Techniques

### 1. Deduplication

```python
def deduplicate():
    seen_hashes = set()
    for file in all_files:
        hash = compute_hash(file)
        if hash not in seen_hashes:
            store_file(file)
            seen_hashes.add(hash)
        # Else: already stored, create reference
```

### 2. Prefetching

```python
def prefetch_related(file_id):
    # Predict related files
    related = get_related_files(file_id)
    
    # Load into cache
    for file in related:
        cache.preload(file)
```

### 3. Memory Mapping

```c
// Map archive into memory for fast access
void* map_archive(int archive_id) {
    char path[256];
    sprintf(path, "data/data.%03d", archive_id);
    
    int fd = open(path, O_RDONLY);
    struct stat st;
    fstat(fd, &st);
    
    return mmap(NULL, st.st_size, PROT_READ, MAP_PRIVATE, fd, 0);
}
```

## Cache Management

### Cache Hierarchy

1. **Memory Cache**: Hot files in RAM
2. **SSD Cache**: Frequently accessed files
3. **Archive Storage**: Complete game data

### Eviction Policy

```python
class LRUCache:
    def evict(self):
        # Remove least recently used
        while self.size > self.max_size:
            oldest = self.lru_queue.pop()
            self.remove(oldest)
            
    def access(self, key):
        # Move to front on access
        self.lru_queue.remove(key)
        self.lru_queue.push_front(key)
```

## Integrity Verification

### Checksum Validation

```python
def verify_file(ekey, content):
    # Compute actual hash
    actual_hash = md5(content)
    
    # Compare with expected
    expected = ekey_to_ckey(ekey)
    
    if actual_hash != expected:
        raise IntegrityError(f"Hash mismatch for {ekey}")
```

### Repair Process

```python
def repair_installation():
    errors = []
    
    # Verify all files
    for ekey in all_files:
        try:
            content = read_file(ekey)
            verify_file(ekey, content)
        except IntegrityError as e:
            errors.append(ekey)
    
    # Re-download corrupted files
    for ekey in errors:
        download_file(ekey)
        
    return len(errors)
```

## Space Management

### Storage Calculation

```python
def calculate_storage():
    total = 0
    
    # Archives
    for archive in glob("data/data.*"):
        total += os.path.getsize(archive)
    
    # Indices
    for index in glob("indices/*.index"):
        total += os.path.getsize(index)
    
    # Loose files
    for file in loose_files:
        total += os.path.getsize(file)
    
    return total
```

### Cleanup Operations

```python
def cleanup_orphaned():
    referenced = set()
    
    # Collect all referenced files
    for index in all_indices:
        for entry in index:
            referenced.add(entry.ekey)
    
    # Remove unreferenced
    for archive in all_archives:
        for file in archive:
            if file.ekey not in referenced:
                archive.remove(file)
```

## Implementation in Cascette-RS

### Current Status

CASC storage is planned for version 0.2.0 of cascette-rs.

### Planned Architecture

```rust
pub struct CascStorage {
    indices: HashMap<u8, GroupIndex>,
    archives: Vec<Archive>,
    cache: LruCache<EKey, Vec<u8>>,
    config: CascConfig,
}

impl CascStorage {
    pub fn read(&self, ekey: &EKey) -> Result<Vec<u8>>;
    pub fn write(&mut self, ekey: &EKey, data: &[u8]) -> Result<()>;
    pub fn verify(&self) -> Result<Vec<EKey>>;
    pub fn repair(&mut self, errors: &[EKey]) -> Result<()>;
}
```

### Integration Points

- Will integrate with `ngdp-cdn` for downloads
- Will use `tact-parser` for file formats
- Will provide storage for `ngdp-client`

## Performance Metrics

### Typical Performance

| Operation | Time | Throughput |
|-----------|------|------------|
| Index lookup | <1ms | - |
| Small file read | <10ms | - |
| Large file read | <100ms | 100MB/s |
| Archive scan | <1s | - |
| Full verify | ~5min | 20GB/min |

### Optimization Targets

- Sub-millisecond index lookups
- 500MB/s+ sequential read
- <100ms game startup
- <10GB memory usage
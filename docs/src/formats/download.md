# Download Manifest Format

The Download manifest manages content streaming and prioritization during game
installation and updates. It defines which files are essential for gameplay
and their download order.

## Overview

The Download manifest enables:

- Playable game state before full download

- Priority-based content streaming

- Download size estimation

- Bandwidth optimization

- Progressive installation

## File Structure

The Download manifest is BLTE-encoded and contains:

```text
[BLTE Container]
  [Header]
  [File Entries]
  [Tag Section]
```

## Binary Format

### Header

```c
struct DownloadHeader {
    char     magic[2];           // "DL" (0x44, 0x4C)
    uint8_t  version;            // Version (1, 2, or 3)
    uint8_t  ekey_size;          // Encoding key size in bytes (16)
    uint8_t  has_checksum;       // Checksum presence flag
    uint32_t entry_count;        // Number of entries (big-endian)
    uint16_t tag_count;          // Number of tags (big-endian)

    // Version 2+ fields
    uint8_t  flag_size;          // Number of flag bytes per entry

    // Version 3+ fields
    int8_t   base_priority;      // Base priority offset
    uint8_t  _reserved[3];       // Must be zero
};
```

### Entry Order

The download manifest stores data in this order:

1. Header
2. All file entries
3. All tags (appear after entries)

### File Entry

```c
struct DownloadEntry {
    uint8_t  ekey[16];           // Encoding key (variable size from header)
    uint8_t  file_size[5];       // 40-bit file size (big-endian)
    int8_t   priority;           // Download priority (adjusted by base_priority)

    // Optional fields
    uint32_t checksum;           // If has_checksum is true (big-endian)
    uint8_t  flags[N];           // If version >= 2, N = flag_size
};
```

### Tag Entry

Tags appear after all file entries in the manifest:

```c
struct DownloadTag {
    char     name[];             // Null-terminated tag name
    uint16_t type;               // Tag type (big-endian)
    uint8_t  bitmap[];           // Bit mask ((entry_count + 7) / 8 bytes)
};
```

Each bit in the bitmap corresponds to a file entry index. If bit N is set,
entry N has this tag.

## Priority System

### Priority Calculation

In version 3+, priorities are adjusted:

```text
final_priority = entry.priority - header.base_priority
```

### Priority Levels

Lower values indicate higher priority:

| Priority | Description | Typical Content |
|----------|-------------|-----------------|
| 0 | Essential | Required to start game |
| 1 | Critical | Core gameplay files |
| 2 | Standard | Common game content |
| 3+ | Optional | Downloaded as needed |

### Priority-Based Download

```rust
fn get_download_order(entries: &[DownloadFileEntry]) -> Vec<&DownloadFileEntry>
{
    let mut sorted = entries.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|e| (e.priority, e.file_size));
    sorted
}
```

## Streaming Strategy

### Minimum Playable Set

Calculate minimum download for gameplay:

```rust
fn get_minimum_download(
    download_file: &DownloadFile
) -> (Vec<DownloadFileEntry>, u64) {
    let essential: Vec<_> = download_file.entries
        .iter()
        .filter(|e| e.priority <= 1)  // Essential + Critical
        .cloned()
        .collect();

    let total_size = essential.iter()
        .map(|e| e.file_size as u64)
        .sum();

    (essential, total_size)
}
```

### Progressive Download

Download in priority order while game runs:

```rust
struct DownloadManager {
    queue: VecDeque<DownloadItem>,
    active: Vec<DownloadTask>,
    completed: HashSet<[u8; 16]>,
}

impl DownloadManager {
    pub fn start_progressive_download(&mut self) {
        // Sort by priority
        self.queue.sort_by_key(|item| item.priority);

        // Start downloading highest priority
        while self.active.len() < MAX_CONCURRENT {
            if let Some(item) = self.queue.pop_front() {
                self.start_download(item);
            }
        }
    }
}
```

## Tag-Based Filtering

### Platform-Specific Downloads

```rust
fn filter_by_platform(
    entries: &[DownloadFileEntry],
    platform_tags: u16
) -> Vec<&DownloadFileEntry> {
    entries.iter()
        .filter(|e| (e.tag_mask & platform_tags) != 0)
        .collect()
}
```

### Language Packs

```rust
fn get_language_pack(
    download_file: &DownloadFile,
    locale: &str
) -> Vec<DownloadFileEntry> {
    let locale_tag = download_file.get_tag_id(locale);

    download_file.entries
        .iter()
        .filter(|e| (e.tag_mask & (1 << locale_tag)) != 0)
        .cloned()
        .collect()
}
```

## Download Optimization

### Bandwidth Management

```rust
struct BandwidthManager {
    max_bandwidth: u64,      // Bytes per second
    current_usage: u64,
    priority_limits: Vec<u64>, // Per-priority limits
}

impl BandwidthManager {
    pub fn allocate_bandwidth(&mut self, priority: u8) -> u64 {
        let priority_limit = self.priority_limits[priority as usize];
        let available = self.max_bandwidth - self.current_usage;

        std::cmp::min(priority_limit, available)
    }
}
```

### Chunk-Based Downloads

For large files, download in chunks:

```rust
struct ChunkedDownload {
    encoding_key: [u8; 16],
    total_size: u64,
    chunk_size: u64,
    chunks_completed: Vec<bool>,
}

impl ChunkedDownload {
    pub fn get_next_chunk(&self) -> Option<(u64, u64)> {
        for (idx, &completed) in self.chunks_completed.iter().enumerate() {
            if !completed {
                let offset = idx as u64 * self.chunk_size;
                let size = std::cmp::min(
                    self.chunk_size,
                    self.total_size - offset
                );
                return Some((offset, size));
            }
        }
        None
    }
}
```

## Progress Tracking

### Download Statistics

```rust
struct DownloadProgress {
    total_files: u32,
    completed_files: u32,
    total_bytes: u64,
    downloaded_bytes: u64,
    current_speed: f64,
    eta_seconds: u64,
}

impl DownloadProgress {
    pub fn update(&mut self, bytes_downloaded: u64) {
        self.downloaded_bytes += bytes_downloaded;
        self.current_speed = self.calculate_speed();
        self.eta_seconds = self.calculate_eta();
    }

    pub fn completion_percentage(&self) -> f32 {
        (self.downloaded_bytes as f32 / self.total_bytes as f32) * 100.0
    }
}
```

## Implementation Example

```rust
struct DownloadFile {
    header: DownloadHeader,
    priorities: Vec<DownloadPriority>,
    tags: Vec<DownloadTag>,
    entries: Vec<DownloadFileEntry>,
}

impl DownloadFile {
    pub fn get_download_plan(
        &self,
        tags: &[String],
        max_priority: u8
    ) -> DownloadPlan {
        let tag_mask = self.build_tag_mask(tags);

        let files: Vec<_> = self.entries
            .iter()
            .filter(|e| e.priority <= max_priority)
            .filter(|e| (e.tag_mask & tag_mask) != 0)
            .cloned()
            .collect();

        let total_size = files.iter()
            .map(|f| f.file_size as u64)
            .sum();

        DownloadPlan {
            files,
            total_size,
            estimated_time: self.estimate_time(total_size),
        }
    }
}
```

## On-Demand Streaming

### Asset Request Handling

```rust
struct OnDemandManager {
    download_file: DownloadFile,
    cache: LruCache<[u8; 16], Vec<u8>>,
}

impl OnDemandManager {
    pub async fn get_asset(&mut self, encoding_key: &[u8; 16]) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(data) = self.cache.get(encoding_key) {
            return Ok(data.clone());
        }

        // Find in download manifest
        if let Some(entry) = self.find_entry(encoding_key) {
            // Download with high priority
            let data = self.download_immediate(entry).await?;
            self.cache.put(*encoding_key, data.clone());
            return Ok(data);
        }

        Err("Asset not found")
    }
}
```

## Verification

### Checksum Validation

```rust
fn verify_download(
    data: &[u8],
    entry: &DownloadFileEntry
) -> bool {
    if entry.checksum != [0; 16] {
        let computed = md5::compute(data);
        computed.0 == entry.checksum
    } else {
        true // No checksum to verify
    }
}
```

## Common Issues

1. **Priority conflicts**: Multiple systems requesting same file
2. **Bandwidth throttling**: ISP or network limitations
3. **Incomplete downloads**: Handle partial file recovery
4. **Cache corruption**: Verify cached files periodically
5. **Tag mismatches**: Platform detection errors

## Special Features

### Differential Downloads

Download only changed portions:

```rust
struct DifferentialDownload {
    old_version: [u8; 16],
    new_version: [u8; 16],
    patches: Vec<PatchInfo>,
}
```

### Peer-to-Peer Support

Share downloaded content locally:

```rust
struct P2PManager {
    local_peers: Vec<PeerInfo>,
    shared_files: HashSet<[u8; 16]>,
}
```

## Parser Implementation Status

### Python Parser (cascette-py)

**Status**: Complete

**Capabilities**:

- Version 1-3 header parsing with DL magic detection

- 40-bit big-endian compressed size parsing

- Priority system with base priority adjustment (v3)

- Tag parsing with bitmap support (tags stored after all entries)

- Platform/architecture tag identification with type classification

- Sample entry display (first 100 entries)

- Format evolution tracking across versions

- BLTE decompression for compressed manifests

- Correct entry/tag ordering (entries first, then tags)

**Verified Against**:

- WoW 11.0.5.57689 (2.4M entries, 28 tags)

- WoW 9.0.2.37176 (Shadowlands)

- WoW 7.3.5.25848 (Legion)

- WoW Classic builds

**Known Issues**: None

See <https://github.com/wowemulation-dev/cascette-py> for the Python
implementation.

## Version History

The Download manifest format has evolved through 3 versions:

### Version 1 (Initial)

- **Header Size**: 10 bytes
- **Features**: Basic download prioritization with encoding keys, file sizes, optional checksums
- **Fields**: magic, version, ekey_size, has_checksum, entry_count, tag_count

### Version 2 (Flag Support)

- **Header Size**: 11 bytes
- **Added Features**: Entry-level flags for additional metadata
- **New Fields**: flag_size (number of flag bytes per entry)
- **Use Cases**: Platform-specific flags, content type markers

### Version 3 (Priority System)

- **Header Size**: 15 bytes
- **Added Features**: Base priority adjustment for dynamic prioritization
- **New Fields**: base_priority (signed adjustment), reserved (3 bytes, must be zero)
- **Priority Calculation**: `final_priority = entry.priority - header.base_priority`
- **Validation**: Reserved field must be [0, 0, 0]

### Version Detection

Parsers detect version by reading the version field at offset 2 in the header. All versions use
the same "DL" magic bytes and big-endian encoding.

### Implementation Status

- **cascette-formats**: Full support for versions 1-3 with version-aware parsing
- **cascette-py**: Complete parsing for versions 1-3 with validation

## References

- See [Install Manifest](install.md) for installation management

- See [Encoding Documentation](encoding.md) for key resolution

- See [CDN Architecture](cdn.md) for download sources

- See [Format Transitions](format-transitions.md) for version evolution timeline

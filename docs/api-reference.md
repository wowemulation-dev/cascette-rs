# API Reference Guide

## Overview

This guide provides comprehensive API documentation for the key features implemented in cascette-rs, including streaming BLTE decompression, HTTP range requests, and the file download command.

## Streaming BLTE Decompression

### BLTEStream API

The `BLTEStream` struct provides memory-efficient streaming decompression of BLTE-encoded files.

#### Creating a Stream

```rust
use blte::{BLTEStream, create_streaming_reader};
use std::io::Read;

// Method 1: Using the convenience function
let mut stream = create_streaming_reader(blte_data, None)?;

// Method 2: Manual construction
let blte_file = BLTEFile::parse(&blte_data)?;
let mut stream = BLTEStream::new(blte_file, None);

// With encryption key service
let key_service = KeyService::new();
let mut stream = create_streaming_reader(blte_data, Some(key_service))?;
```

#### Reading Data

```rust
// Read into a buffer
let mut buffer = vec![0u8; 4096];
let bytes_read = stream.read(&mut buffer)?;

// Read all data (memory-intensive for large files)
let mut all_data = Vec::new();
stream.read_to_end(&mut all_data)?;

// Process in chunks
loop {
    let mut chunk = vec![0u8; 8192];
    match stream.read(&mut chunk)? {
        0 => break, // EOF
        n => process_chunk(&chunk[..n]),
    }
}
```

#### Supported Compression Modes

- **Mode 'N' (0x4E)**: No compression - data passed through unchanged
- **Mode 'Z' (0x5A)**: ZLib compression - decompressed on-the-fly
- **Mode '4' (0x34)**: LZ4 compression - fast decompression
- **Mode 'F' (0x46)**: Frame mode - recursive BLTE (falls back to regular decompression)
- **Mode 'E' (0x45)**: Encrypted blocks - requires key service

#### Example: Streaming Large Files

```rust
use blte::create_streaming_reader;
use std::io::{Read, Write};
use std::fs::File;

fn decompress_large_file(input_path: &str, output_path: &str) -> Result<()> {
    let blte_data = std::fs::read(input_path)?;
    let mut stream = create_streaming_reader(blte_data, None)?;
    let mut output = File::create(output_path)?;

    // Stream in 64KB chunks
    let mut buffer = vec![0u8; 65536];
    loop {
        match stream.read(&mut buffer)? {
            0 => break,
            n => output.write_all(&buffer[..n])?,
        }
    }

    Ok(())
}
```

## HTTP Range Requests

### Partial Content Downloads

The `tact-client` now supports HTTP range requests for downloading specific byte ranges from CDN files.

#### Single Range Request

```rust
use tact_client::{HttpClient, Region, ProtocolVersion};

let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;

// Download first 1KB of a file
let response = client.download_file_range(
    "blzddist1-a.akamaihd.net",
    "tpr/wow/data",
    "abc123def456789",
    (0, Some(1023))
).await?;

// Check if server supports range requests
if response.status() == 206 {
    println!("Partial content received");
} else if response.status() == 200 {
    println!("Full content returned - range not supported");
}
```

#### Open-Ended Range

```rust
// Download from byte 1024 to end of file
let response = client.download_file_range(
    cdn_host,
    path,
    hash,
    (1024, None)
).await?;
```

#### Multiple Ranges

```rust
// Download multiple non-contiguous ranges
let ranges = [
    (0, Some(255)),      // Header
    (1024, Some(2047)),  // Metadata section
    (8192, None),        // Rest of file
];

let response = client.download_file_multirange(
    cdn_host,
    path,
    hash,
    &ranges
).await?;

// Note: Multi-range responses return multipart/byteranges
// which requires special parsing
```

#### Use Cases

1. **Header Inspection**: Download file headers to determine format before full download
2. **Progressive Loading**: Stream large files by downloading chunks on demand
3. **Resume Support**: Continue interrupted downloads from last byte
4. **Bandwidth Optimization**: Only download needed portions of large archives

## File Download Command

### CLI Download Interface

The `ngdp-client` provides comprehensive download functionality through its CLI.

#### Download Build Files

```bash
# Download all build configuration files
ngdp-client download build wow_classic_era 1.15.5.57638 --output ./build-files/

# Files downloaded:
# - BuildConfig
# - CDNConfig
# - ProductConfig
# - KeyRing (if available)
```

#### Download by Content Key

```bash
# Download a specific file by its content key
ngdp-client download ckey abc123def456789abc123def456789ab --output ./file.dat

# Automatically:
# - Resolves CDN location
# - Downloads BLTE-encoded data
# - Decompresses to output file
```

#### Download by Encoding Key

```bash
# Download using encoding key (ekey)
ngdp-client download ekey 0123456789abcdef0123456789abcdef --output ./file.dat
```

#### Download by File Path

```bash
# Download a specific game file
ngdp-client download file "Interface/AddOns/Blizzard_AuctionUI/Blizzard_AuctionUI.toc" \
    --product wow --output ./auction_ui.toc
```

### Programmatic Download API

```rust
use ngdp_client::download::{download_build, download_file_by_key};
use ngdp_cdn::CachedCdnClient;
use ribbit_client::CachedRibbitClient;

async fn download_example() -> Result<()> {
    // Initialize clients
    let ribbit = CachedRibbitClient::new(Region::US).await?;
    let cdn = CachedCdnClient::new().await?;

    // Download build files
    download_build(
        "wow_classic_era",
        "1.15.5.57638",
        Path::new("./output"),
        Region::US
    ).await?;

    // Download specific file
    let content_key = "abc123def456789";
    download_file_by_key(
        &cdn,
        content_key,
        Path::new("./file.dat")
    ).await?;

    Ok(())
}
```

## Cache Management

### Cache Statistics API

The caching layer provides detailed statistics about cache performance:

```rust
use ngdp_cache::CacheStats;

let cache = Cache::new();

// Get cache statistics
let stats = cache.statistics();
println!("Cache hits: {}", stats.hits);
println!("Cache misses: {}", stats.misses);
println!("Hit ratio: {:.2}%", stats.hit_ratio() * 100.0);
println!("Bytes saved: {}", stats.bytes_saved);

// Cache warming
let important_keys = vec![
    "buildconfig_hash",
    "encoding_hash",
    "root_hash",
];
cache.warm_up(&important_keys).await?;
```

### TTL Management

```rust
use std::time::Duration;
use ngdp_cache::Cache;

let cache = Cache::builder()
    .ttl(Duration::from_secs(3600))  // 1 hour TTL
    .max_size(1024 * 1024 * 100)     // 100MB max
    .build();

// Check if entry is expired
if cache.is_expired("key") {
    // Refresh from source
}

// Force refresh
cache.refresh("key").await?;
```

## Error Handling

### Comprehensive Error Types

```rust
use tact_client::Error;

match download_file().await {
    Ok(data) => process(data),
    Err(Error::Http(e)) if e.is_timeout() => {
        // Handle timeout
    },
    Err(Error::CdnExhausted { resource }) => {
        // All CDN hosts failed
    },
    Err(Error::ChecksumMismatch { expected, actual }) => {
        // Data corruption detected
    },
    Err(e) => {
        // Generic error handling
    }
}
```

## Performance Considerations

### Memory Usage

- **BLTEStream**: Constant memory usage regardless of file size
- **Range Requests**: Download only needed portions
- **Cache**: Configurable size limits with LRU eviction

### Network Optimization

- **Retry Logic**: Exponential backoff with jitter
- **CDN Fallback**: Automatic failover to alternate CDN hosts
- **Parallel Downloads**: Support for concurrent chunk downloads

### Example: Optimized Large File Download

```rust
async fn download_large_file_optimized(
    client: &HttpClient,
    cdn_host: &str,
    path: &str,
    hash: &str,
    output_path: &Path
) -> Result<()> {
    // First, get file size using HEAD request or small range
    let header_response = client.download_file_range(
        cdn_host, path, hash, (0, Some(0))
    ).await?;

    let total_size = header_response
        .headers()
        .get("content-range")
        .and_then(|v| parse_total_size(v))
        .unwrap_or(0);

    // Download in 1MB chunks
    const CHUNK_SIZE: u64 = 1024 * 1024;
    let mut output = File::create(output_path)?;

    for offset in (0..total_size).step_by(CHUNK_SIZE as usize) {
        let end = (offset + CHUNK_SIZE - 1).min(total_size - 1);

        let response = client.download_file_range(
            cdn_host, path, hash, (offset, Some(end))
        ).await?;

        let chunk = response.bytes().await?;
        output.write_all(&chunk)?;

        // Update progress
        println!("Downloaded {}/{} bytes", offset + chunk.len() as u64, total_size);
    }

    Ok(())
}
```

## Best Practices

### When to Use Streaming

- Files larger than 10MB
- Processing data on-the-fly
- Limited memory environments
- Real-time decompression needs

### When to Use Range Requests

- Checking file headers/metadata
- Implementing pause/resume
- Partial file updates
- Bandwidth-limited scenarios

### Cache Strategy

- Cache build configurations indefinitely
- Cache encoding/root files with 1-hour TTL
- Don't cache large content files
- Warm cache with frequently accessed keys

## Migration Guide

### Upgrading from Non-Streaming BLTE

```rust
// Old approach (loads entire file)
let decompressed = blte::decompress(&blte_data)?;

// New streaming approach
let mut stream = blte::create_streaming_reader(blte_data, None)?;
let mut decompressed = Vec::new();
stream.read_to_end(&mut decompressed)?;
```

### Adding Range Support

```rust
// Old: Always download full file
let response = client.download_file(cdn_host, path, hash).await?;

// New: Download only what's needed
let response = client.download_file_range(
    cdn_host, path, hash,
    (offset, Some(offset + needed_bytes - 1))
).await?;
```

## Troubleshooting

### Common Issues

1. **Range requests return 200 instead of 206**
   - Server doesn't support range requests
   - Fall back to full download

2. **BLTEStream fails with encrypted content**
   - Ensure KeyService is provided
   - Check key database is loaded

3. **Cache misses despite recent access**
   - Check TTL settings
   - Verify cache size limits

4. **Multirange requests fail**
   - Not all CDN servers support multipart
   - Use multiple single-range requests instead

## Future Enhancements

- Parallel chunk downloading for BLTEStream
- Automatic resume support in download command
- Cache compression for disk storage
- WebSocket support for real-time updates

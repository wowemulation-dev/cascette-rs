# cascette-cache

Multi-layer caching infrastructure for NGDP/CDN content.

## Status

Working implementation with support for both native and WASM platforms.

## Platform Support

| Platform | Status | Cache Backends |
|----------|--------|----------------|
| Native (Linux, macOS, Windows) | Full support | Memory, Disk, Multi-layer, NGDP-specific |
| WASM (Browser) | Basic support | LocalStorage, IndexedDB |

## Features

### Cross-Platform

- `AsyncCache` trait for unified cache interface
- Type-safe cache keys for different NGDP content types
- TTL-based expiration policies
- Cache statistics and metrics
- Memory pooling optimized for NGDP file size classes

### Native Only

- L1 memory cache with LRU eviction and size-based limits
- L2 disk cache with fsync durability and atomic writes
- Multi-layer cache combining L1 memory and L2 disk
- NGDP-specific caches: resolution, content-addressed, BLTE block, archive range
- Validation hooks for MD5, Jenkins96, and TACT key verification
- Zero-copy data structures with reference counting
- Streaming interfaces for large file handling
- SIMD-optimized hash operations (SSE2, SSE4.1, AVX2, AVX-512)
- CDN integration with retry logic and range requests
- Atomic metrics for hit rates and performance tracking

### WASM Only

- LocalStorage cache for small protocol data (~5-10MB browser limit)
- IndexedDB cache for larger content (~50MB+, unlimited with permission)
- Base64 encoding for binary data storage
- Lazy expiration checking on reads

## Cache Backends

### Native Backends

| Backend | Use Case | Capacity |
|---------|----------|----------|
| `MemoryCache` | L1 fast access | Configurable, RAM-limited |
| `DiskCache` | L2 persistent storage | Configurable, disk-limited |
| `MultiLayerCacheImpl` | Combined L1/L2 | Both layers |
| `NgdpResolutionCache` | FileDataID resolution | Memory-based |
| `ContentAddressedCache` | Content-keyed storage | Memory-based |
| `BlteBlockCache` | BLTE block caching | Memory-based |
| `ArchiveCache` | Archive range caching | Memory-based |

### WASM Backends

| Backend | Use Case | Capacity |
|---------|----------|----------|
| `LocalStorageCache` | Protocol responses, configs | ~5-10MB (browser limit) |
| `IndexedDbCache` | Larger content files | ~50MB+ (with user permission) |

## Modules

### Cross-Platform

- `key` - Type-safe cache keys for NGDP content types
- `config` - Cache configuration and eviction policies
- `stats` - Cache statistics (uses timestamps for cross-platform compatibility)
- `traits` - Async cache traits
- `error` - Error types
- `pool` - Memory pooling with NGDP size classes
- `simd` - SIMD operations (stubs on WASM)
- `game_optimized` - Cache access pattern analysis

### Native Only

- `memory_cache` - L1 memory cache with LRU eviction
- `disk_cache` - L2 disk cache with atomic writes
- `multi_layer` - Combined L1/L2 caching
- `ngdp` - NGDP resolution, content-addressed, BLTE block, and archive caches
- `validation` - Content validation hooks (MD5, Jenkins96, TACT keys)
- `streaming` - Chunk-based streaming for large files
- `zerocopy` - Zero-copy data structures and buffer pools
- `cdn` - CDN client integration with retry and range requests
- `memory` - Memory pool management
- `integration` - Format integration utilities

### WASM Only

- `local_storage_cache` - Browser LocalStorage backend
- `indexed_db_cache` - Browser IndexedDB backend

## Usage

### Native

```rust
use cascette_cache::{MemoryCache, DiskCache, AsyncCache};
use cascette_cache::key::RibbitKey;
use bytes::Bytes;

// Create memory cache
let cache = MemoryCache::new(100 * 1024 * 1024); // 100MB

// Use with typed keys
let key = RibbitKey::new("versions", "us");
cache.put(key.clone(), Bytes::from("data")).await?;
let data = cache.get(&key).await?;
```

### WASM

```rust
use cascette_cache::{LocalStorageCache, IndexedDbCache, AsyncCache};
use cascette_cache::key::RibbitKey;
use bytes::Bytes;
use std::time::Duration;

// LocalStorage for small protocol data
let ls_cache = LocalStorageCache::new(
    Duration::from_secs(300),  // 5 min TTL
    5 * 1024 * 1024,           // 5MB limit
)?;

// IndexedDB for larger content
let idb_cache = IndexedDbCache::new(
    Duration::from_secs(3600), // 1 hour TTL
    100 * 1024 * 1024,         // 100MB limit
).await?;

// Same trait interface
let key = RibbitKey::new("versions", "us");
ls_cache.put(key.clone(), Bytes::from("data")).await?;
```

## Dependencies

### All Platforms

- `async-trait` - Async trait support
- `bytes` - Zero-copy byte buffers
- `serde` - Serialization
- `cascette-crypto` - Hash functions (MD5, Jenkins96)

### Native Only

- `tokio` - Async runtime (full features)
- `dashmap` - Concurrent hashmap
- `cascette-formats` - NGDP format types

### WASM Only

- `tokio` - Async runtime (sync feature only)
- `web-sys` - Browser API bindings
- `wasm-bindgen` - JS interop
- `wasm-bindgen-futures` - Async JS interop
- `js-sys` - JavaScript types

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

---

**Note**: This project is not affiliated with Blizzard Entertainment. It is
an independent implementation based on reverse engineering by the World of
Warcraft emulation community.

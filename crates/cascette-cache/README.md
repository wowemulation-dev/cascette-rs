# cascette-cache

Multi-layer caching infrastructure for NGDP/CDN content.

## Status

Working implementation with memory and disk caching layers.

## Features

- L1 memory cache with LRU eviction and size-based limits
- L2 disk cache with fsync durability and atomic writes
- Multi-layer cache combining L1 memory and L2 disk
- NGDP-specific caches: resolution, content-addressed, BLTE block, archive range
- Validation hooks for MD5, Jenkins96, and TACT key verification
- Zero-copy data structures with reference counting
- Streaming interfaces for large file handling
- SIMD-optimized hash operations (SSE2, SSE4.1, AVX2, AVX-512)
- Memory pooling optimized for NGDP file size classes
- CDN integration with retry logic and range requests
- TTL-based expiration policies
- Atomic metrics for hit rates and performance tracking

## Modules

- `memory_cache` - L1 memory cache with LRU eviction
- `disk_cache` - L2 disk cache with atomic writes
- `multi_layer` - Combined L1/L2 caching
- `ngdp` - NGDP resolution, content-addressed, BLTE block, and archive caches
- `pool` - Memory pooling with NGDP size classes (Small/Medium/Large/Huge)
- `validation` - Content validation hooks (MD5, Jenkins96, TACT keys)
- `streaming` - Chunk-based streaming for large files
- `zerocopy` - Zero-copy data structures and buffer pools
- `simd` - SIMD-optimized hash and memory operations
- `cdn` - CDN client integration with retry and range requests
- `key` - Type-safe cache keys for NGDP content types
- `config` - Cache configuration and eviction policies
- `stats` - Atomic metrics for cache performance
- `traits` - Async cache traits

## Dependencies

- `tokio` - Async runtime
- `async-trait` - Async trait support
- `bytes` - Zero-copy byte buffers
- `dashmap` - Concurrent hashmap
- `serde` - Serialization
- `cascette-crypto` - Hash functions (MD5, Jenkins96)
- `cascette-formats` - NGDP format types

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

# cascette-cache Examples

## cache_verification

Verifies MemoryCache round-trip storage and LRU eviction behavior. Stores entries, retrieves them, and confirms that older entries are evicted when the cache exceeds its configured limit.

```bash
cargo run -p cascette-cache --example cache_verification
```

## memory_cache

Demonstrates MemoryCache with typed cache keys (RibbitKey, ConfigKey, BlteKey, ContentCacheKey, ManifestKey), TTL-based expiration, eviction policies, fast hashing, and cache statistics.

```bash
cargo run -p cascette-cache --example memory_cache
```

## multi_layer_cache

Demonstrates MultiLayerCacheConfig construction with memory and disk layers, promotion strategies, NGDP size classes, memory pool allocation and deallocation, pool warm-up, pool statistics, and thread-local pool usage.

```bash
cargo run -p cascette-cache --example multi_layer_cache
```

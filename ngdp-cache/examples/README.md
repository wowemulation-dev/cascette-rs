# ngdp-cache Examples

This directory contains examples demonstrating various caching strategies and usage patterns with the `ngdp-cache` crate.

## Basic Usage

### `01_basic_cache_types.rs`

Introduction to different cache types:

- GenericCache for arbitrary key-value storage
- RibbitCache for protocol responses with TTL
- CdnCache for CDN content with hash-based paths
- Basic operations (read, write, exists, delete)

## Client Integration Examples

### `cached_ribbit_client.rs`

Demonstrates CachedRibbitClient as a drop-in replacement:

- Transparent caching for Ribbit responses
- Performance comparison with/without caching
- TTL behavior for different endpoint types
- Cache hit/miss statistics

### `cached_cdn_client.rs`

Shows CachedCdnClient usage patterns:

- CDN content caching with automatic hash paths
- Content type detection and caching
- Streaming operations with caching
- Cache management (clear, stats)

### `cached_tact_client.rs`

TACT protocol caching demonstration:

- Metadata endpoint caching
- Sequence number handling
- TTL strategies for different data types
- Cache isolation between regions/products

### `drop_in_replacement.rs`

Complete API compatibility demonstration:

- Shows how cached clients maintain full API compatibility
- No code changes required when switching to cached versions
- Performance benefits with identical interfaces

## Advanced Usage

### `cdn_cache_structure.rs`

CDN cache directory organization:

- Hash-based path structure (`{hash[0:2]}/{hash[2:4]}/{hash}`)
- Product-specific organization
- Content type separation (config, data, patch, indices)

### `cdn_helper_methods.rs`

Utility methods for CDN operations:

- Path construction helpers
- Content type detection
- Cache key generation
- Cleanup and maintenance operations

### `streaming_demo.rs`

Memory-efficient streaming operations:

- Large file handling with constant memory usage
- Chunked reading and writing
- Buffered streaming with custom buffer sizes
- Progress tracking for large operations

### `cdn_path_example.rs`

CDN path and URL construction:

- Blizzard CDN path conventions
- Hash-based directory structures
- URL building for different content types

## Integration Examples

### `full_ngdp_pipeline.rs`

Complete NGDP workflow demonstration:

- Product version queries with Ribbit caching
- CDN configuration with TACT caching
- Content downloads with CDN caching
- End-to-end caching strategy

### `ribbit_cdn_download.rs`

Integration between Ribbit and CDN caching:

- Use Ribbit to get CDN information
- Cache CDN configurations
- Download content with caching
- Cross-protocol data flow

## Running Examples

To run any example:

```bash
cargo run --example <example_name> -p ngdp-cache
```

For example:

```bash
cargo run --example 01_basic_cache_types -p ngdp-cache
cargo run --example cached_ribbit_client -p ngdp-cache
cargo run --example full_ngdp_pipeline -p ngdp-cache
```

## Cache Locations

Examples use platform-specific cache directories:

- **Linux**: `~/.cache/ngdp/`
- **macOS**: `~/Library/Caches/ngdp/`
- **Windows**: `%LOCALAPPDATA%\ngdp\cache\`

The cache structure follows Blizzard's CDN conventions for compatibility with existing tools.

## Performance Notes

The examples demonstrate various performance aspects:

- **Memory efficiency**: Streaming operations use constant memory
- **Network efficiency**: Cached responses eliminate redundant requests
- **Storage efficiency**: Hash-based deduplication prevents duplicate content
- **Parallel operations**: Batch operations for improved throughput

Run examples with timing output to see performance benefits:

```bash
time cargo run --example cached_ribbit_client -p ngdp-cache
```

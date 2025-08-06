# ngdp-cdn Examples

This directory contains examples demonstrating how to use the `ngdp-cdn` crate for downloading content from Blizzard's CDN infrastructure.

## Available Examples

### `basic_usage.rs`
Introduction to CDN client usage:
- Creating a CDN client with default configuration
- Basic file downloads from CDN
- Error handling for common scenarios
- Content verification basics

### `parallel_download.rs`
Demonstrates parallel download capabilities:
- Concurrent downloads with configurable limits
- Progress tracking for bulk operations
- Performance comparison vs sequential downloads
- Error handling in parallel scenarios

### `fallback_usage.rs`
Shows CDN fallback functionality:
- Primary CDN configuration
- Automatic fallback to backup CDN servers
- Community mirror integration (arctium.tools, reliquaryhq.com)
- Custom CDN configuration

## CDN Operations

The examples cover all CDN content types:

### Configuration Files
- Build configurations
- CDN server lists
- Product configurations

### Data Files
- Game assets and content
- Patch data
- Archive files

### Patch Files
- Incremental updates
- Delta patches
- Version differences

### Index Files
- Content indices
- File manifests
- Directory structures

## Running Examples

To run any example:
```bash
cargo run --example <example_name> -p ngdp-cdn
```

For example:
```bash
cargo run --example basic_usage -p ngdp-cdn
cargo run --example parallel_download -p ngdp-cdn
cargo run --example fallback_usage -p ngdp-cdn
```

## Configuration Options

Examples demonstrate various client configurations:

### Connection Settings
- Connection timeouts
- Request timeouts  
- Connection pool sizing
- Keep-alive settings

### Retry Behavior
- Maximum retry attempts
- Exponential backoff timing
- Jitter factor for load distribution
- Rate limit handling

### Performance Tuning
- Parallel download limits
- Chunk size optimization
- Memory usage control
- Progress reporting

## Error Handling

Examples show comprehensive error handling:
- Network connectivity issues
- Content not found (404) errors
- Rate limiting (429) responses
- Content verification failures
- Timeout scenarios

## CDN Infrastructure

The examples work with Blizzard's CDN infrastructure:
- Primary Blizzard CDN servers
- Community backup mirrors
- Regional CDN distribution
- Load balancing considerations

## Performance Notes

Examples demonstrate performance optimization:
- Parallel downloads can achieve 3-5x speedup
- Connection pooling reduces overhead
- Retry logic handles transient failures
- Streaming operations minimize memory usage

Measure performance with timing:
```bash
time cargo run --example parallel_download -p ngdp-cdn
```

## Integration

These examples integrate well with other crates:
- Use with `tact-client` for CDN configuration discovery
- Combine with `ngdp-cache` for transparent caching
- Integrate with `ribbit-client` for version information
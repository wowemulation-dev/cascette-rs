# casc-storage Examples

This directory contains examples demonstrating how to use the `casc-storage` crate for CASC (Content Addressable Storage Container) operations.

## Available Examples

### `progressive_loading_demo.rs`
Demonstrates progressive loading of CASC files:
- Load files in chunks on demand
- Memory-efficient streaming for large files
- Lazy loading of archive contents
- Progress tracking for load operations

```bash
cargo run --example progressive_loading_demo
```

### `texture_streaming.rs`
Shows texture streaming from CASC storage:
- Stream texture data from CASC archives
- Handle mipmap levels progressively
- Memory management for texture data
- Real-time loading patterns

```bash
cargo run --example texture_streaming
```

## Running Examples

To run all examples:
```bash
cargo run --example progressive_loading_demo -p casc-storage
cargo run --example texture_streaming -p casc-storage
```

## Prerequisites

These examples require:
- A valid CASC storage directory (typically from a WoW installation)
- Sufficient permissions to read game files
- Understanding of CASC storage structure

## Example Data

The examples demonstrate:
- Loading files from CASC archives
- Progressive/streaming patterns for large assets
- Memory-efficient processing techniques
- Real-world usage patterns from game clients

## Notes

- CASC storage is used by modern Blizzard games for content storage
- These examples show best practices for efficient file access
- Progressive loading reduces memory usage for large files
- Streaming patterns enable real-time asset loading
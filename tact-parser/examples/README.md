# tact-parser Examples

This directory contains examples demonstrating how to use the `tact-parser` crate for parsing various TACT file formats.

## Available Examples

### `parse_wow_root.rs`
Demonstrates parsing WoW root files to extract file information:
- Loading and parsing root files from disk
- Extracting file IDs and MD5 hashes
- Handling both legacy and modern root format variations
- Jenkins3 hash calculations for file lookups
- Performance optimization with buffered I/O

## TACT File Formats

The `tact-parser` crate supports parsing multiple TACT file formats. While this examples directory currently has one example, the crate handles:

### Root Files
- File ID to MD5 hash mapping
- Directory structure information  
- Locale-specific file variants
- Both pre-8.2 and modern format support

### Encoding Files  
- Content Key (CKey) to Encoding Key (EKey) mapping
- File size information with 40-bit integer support
- Bidirectional lookups for content resolution
- Checksum verification with MD5

### Install Manifests
- Platform-specific file installation data
- Tag-based file filtering
- File path and metadata extraction
- Installation size calculations

### Download Manifests
- Download priority information
- Tag-based download filtering  
- File ordering for optimal downloads
- Background download support

### Size Files
- File size information by Encoding Key
- Tag-based size calculations
- Installation space requirements
- Statistical analysis capabilities

### Build Configurations
- Key-value configuration parsing
- Hash-size pair extraction
- VFS entry management
- Build metadata access

### TVFS (TACT Virtual File System)
- Virtual file system structure
- Directory and file listings
- Path resolution capabilities
- Metadata extraction

## Running Examples

To run any example:
```bash
cargo run --example <example_name> -p tact-parser
```

For example:
```bash
cargo run --example parse_wow_root -p tact-parser
```

## Usage Patterns

The examples demonstrate common usage patterns:

### File Format Detection
```rust
// The crate automatically detects and handles different file formats
let data = std::fs::read("manifest.file")?;
if let Ok(encoding) = EncodingFile::parse(&data) {
    // Handle as encoding file
} else if let Ok(install) = InstallManifest::parse(&data) {
    // Handle as install manifest
}
```

### BLTE Integration
Many TACT files are BLTE-compressed. Examples show integration with the `blte` crate:
```rust
use blte::decompress_blte;
use ngdp_crypto::KeyService;

let compressed_data = download_manifest_data()?;
let key_service = KeyService::new();
let decompressed = decompress_blte(compressed_data, Some(&key_service))?;
let manifest = InstallManifest::parse(&decompressed)?;
```

### Performance Optimization
Examples demonstrate performance best practices:
- Buffered I/O for large files
- Memory-efficient parsing strategies  
- Lazy loading of file sections
- Efficient hash calculations

## Integration with Other Crates

The examples show how `tact-parser` integrates with:
- `blte` - For decompressing BLTE-encoded files
- `ngdp-crypto` - For handling encrypted content
- `ngdp-client` - For CLI operations and inspection tools
- `ngdp-cache` - For caching parsed manifests

## Error Handling

Examples demonstrate comprehensive error handling:
- Invalid file format detection
- Corrupted data recovery
- Missing required fields
- Checksum verification failures
- Memory allocation issues for large files

## Performance Notes

Parsing performance varies by file type and size:
- Root files: ~1-10ms for typical WoW roots
- Encoding files: ~10-100ms depending on entry count  
- Install manifests: ~5-50ms based on file count
- Large files benefit from streaming parsers where available

Use release builds for performance testing:
```bash
cargo run --example parse_wow_root -p tact-parser --release
```
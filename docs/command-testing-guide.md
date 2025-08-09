# NGDP Command Testing Guide

This guide provides step-by-step instructions for testing all `ngdp` commands in
a logical order using real data. We use `wow_classic_era` as the primary test
product throughout this guide.

## 🎯 Testing Overview

All commands have been verified to work correctly with real data from Blizzard's
CDN. This guide walks through testing them systematically to ensure your local
setup is working properly.

**Test Product**: `wow_classic_era` (World of Warcraft Classic Era)
**Local Storage Reference**: WoW Classic client version `1.14.2`

## 📋 Prerequisites

Before starting, ensure you have:

- Built the ngdp binary: `cargo build --bin ngdp`
- Network access to Blizzard's CDN servers
- (Optional) Local WoW Classic 1.14.2 installation for storage testing

## 🔄 Testing Order

We'll test commands in dependency order - starting with basic product information
and working up to complex analysis commands.

---

## 1. Basic Product Information Commands

These commands query Blizzard's Ribbit protocol for product metadata.

### 1.1 List All Products

```bash
# Test: Get complete product list
cargo run --bin ngdp -- products list

# Test: JSON output for programmatic use
cargo run --bin ngdp -- products list -o json
```

**Expected**: List including `wow_classic_era`, `wow`, `agent`, `bna`, etc.

### 1.2 Product Version Information

```bash
# Test: Get version history for WoW Classic Era
cargo run --bin ngdp -- products versions wow_classic_era

# Test: JSON output
cargo run --bin ngdp -- products versions wow_classic_era -o json

# Test: Limit results
cargo run --bin ngdp -- products builds wow_classic_era --limit 5
```

**Expected**: Version entries with build IDs like `61582`, `61548`, etc.

### 1.3 CDN Configuration

```bash
# Test: Get CDN server information
cargo run --bin ngdp -- products cdns wow_classic_era

# Test: JSON format
cargo run --bin ngdp -- products cdns wow_classic_era -o json
```

**Expected**: CDN hosts like `level3.blizzard.com`, `us.cdn.blizzard.com`

### 1.4 Product Summary

```bash
# Test: Complete product information
cargo run --bin ngdp -- products info wow_classic_era

# Test: JSON output for all fields
cargo run --bin ngdp -- products info wow_classic_era -o json
```

**Expected**: Combined versions, CDNs, and metadata for WoW Classic Era

---

## 2. Configuration Management

Test the configuration system that stores settings and cache data.

### 2.1 Configuration Display

```bash
# Test: Show current configuration
cargo run --bin ngdp -- config show
```

### 2.2 Configuration Modification

```bash
# Test: Set configuration values
cargo run --bin ngdp -- config set test.product wow_classic_era
cargo run --bin ngdp -- config set test.region us

# Test: Retrieve specific values
cargo run --bin ngdp -- config get test.product
cargo run --bin ngdp -- config get test.region
```

**Expected**: Values are stored and retrieved correctly

---

## 3. Encryption Key Management

Test the encryption key system used for BLTE decompression.

### 3.1 Key Status

```bash
# Test: Check available encryption keys
cargo run --bin ngdp -- keys status

# Test: JSON output for key counts
cargo run --bin ngdp -- keys status -o json
```

**Expected**: Should show 19,000+ keys loaded from various sources

---

## 4. Content Inspection Commands

These commands download and analyze game content files from the CDN.

### 4.1 BPSV File Inspection

```bash
# Test: Create sample BPSV file
echo -e "name!STRING:0|value!DEC:0\ntest|123\nhello|456" > /tmp/test.bpsv

# Test: Parse BPSV file
cargo run --bin ngdp -- inspect bpsv /tmp/test.bpsv

# Test: JSON output
cargo run --bin ngdp -- inspect bpsv /tmp/test.bpsv -o json

# Cleanup
rm /tmp/test.bpsv
```

**Expected**: Parsed schema and data rows displayed correctly

### 4.2 Build Configuration Analysis

First, get a valid build ID:

```bash
# Get latest build ID for wow_classic_era
cargo run --bin ngdp -- products builds wow_classic_era --limit 1 -o json
```

Use the build ID in subsequent tests (example uses `61582`):

```bash
# Test: Inspect build configuration
cargo run --bin ngdp -- inspect build-config wow_classic_era 61582

# Test: JSON output
cargo run --bin ngdp -- inspect build-config wow_classic_era 61582 -o json

# Test: BPSV raw format
cargo run --bin ngdp -- inspect build-config wow_classic_era 61582 -o bpsv
```

**Expected**: Build configuration with encoding, install, download, and size hashes

### 4.3 CDN Configuration Inspection

```bash
# Test: Inspect CDN configuration
cargo run --bin ngdp -- inspect cdn-config wow_classic_era

# Test: JSON output
cargo run --bin ngdp -- inspect cdn-config wow_classic_era -o json

# Test: Different region
cargo run --bin ngdp -- inspect cdn-config wow_classic_era --region eu
```

**Expected**: CDN hosts, paths, and endpoint URLs for the product

### 4.4 Encoding File Analysis

```bash
# Test: Basic encoding file inspection
cargo run --bin ngdp -- inspect encoding wow_classic_era

# Test: With statistics
cargo run --bin ngdp -- inspect encoding wow_classic_era --stats

# Test: JSON output with stats
cargo run --bin ngdp -- inspect encoding wow_classic_era --stats -o json

# Test: Search for specific key (example - replace with actual CKey)
cargo run --bin ngdp -- inspect encoding wow_classic_era --search a1b2c3d4e5f6
```

**Expected**: Encoding file statistics, CKey/EKey mappings

### 4.5 Install Manifest Analysis

```bash
# Test: Basic install manifest
cargo run --bin ngdp -- inspect install wow_classic_era

# Test: Show all entries (be prepared for long output)
cargo run --bin ngdp -- inspect install wow_classic_era --all

# Test: Filter by tags
cargo run --bin ngdp -- inspect install wow_classic_era --tags Windows,enUS

# Test: JSON output
cargo run --bin ngdp -- inspect install wow_classic_era -o json
```

**Expected**: File installation information with tags and paths

### 4.6 Download Manifest Analysis

```bash
# Test: Basic download manifest
cargo run --bin ngdp -- inspect download-manifest wow_classic_era

# Test: Show more priority files
cargo run --bin ngdp -- inspect download-manifest wow_classic_era --priority-limit 20

# Test: Filter by tags
cargo run --bin ngdp -- inspect download-manifest wow_classic_era --tags Windows

# Test: JSON output
cargo run --bin ngdp -- inspect download-manifest wow_classic_era -o json
```

**Expected**: Priority file information for downloads

### 4.7 Size File Analysis

```bash
# Test: Basic size analysis
cargo run --bin ngdp -- inspect size wow_classic_era

# Test: Show largest files
cargo run --bin ngdp -- inspect size wow_classic_era --largest 10

# Test: Tag-based size calculation
cargo run --bin ngdp -- inspect size wow_classic_era --tags Windows,enUS

# Test: JSON output
cargo run --bin ngdp -- inspect size wow_classic_era -o json
```

**Expected**: File size statistics and largest files list

---

## 5. Local Storage Commands

These commands work with locally installed CASC storage. You need a WoW Classic
1.14.2 installation.

**Setup**: Ensure you have a WoW Classic client installed. Common paths:

- Windows: `C:\Program Files (x86)\World of Warcraft\_classic_\Data`
- macOS: `/Applications/World of Warcraft/_classic_/Data`
- Linux: `~/.wine/drive_c/Program Files (x86)/World of Warcraft/_classic_/Data`

Replace `<WOW_PATH>` with your actual WoW installation path in the commands below.

### 5.1 Storage Initialization

```bash
# Test: Initialize new storage (in temporary directory)
mkdir /tmp/test-storage
cargo run --bin ngdp -- storage init /tmp/test-storage

# Test: Check what was created
ls -la /tmp/test-storage

# Cleanup
rm -rf /tmp/test-storage
```

**Expected**: Creates basic storage structure

### 5.2 Storage Configuration

```bash
# Test: Read WoW installation configuration
cargo run --bin ngdp -- storage config <WOW_PATH>

# Test: JSON output
cargo run --bin ngdp -- storage config <WOW_PATH> -o json
```

**Expected**: Build configuration from .build.info file

### 5.3 Storage Information

```bash
# Test: Get storage statistics
cargo run --bin ngdp -- storage info <WOW_PATH>

# Test: JSON output
cargo run --bin ngdp -- storage info <WOW_PATH> -o json
```

**Expected**: Archive and index file counts, storage size

### 5.4 Storage Statistics

```bash
# Test: Detailed storage statistics
cargo run --bin ngdp -- storage stats <WOW_PATH>

# Test: JSON output
cargo run --bin ngdp -- storage stats <WOW_PATH> -o json
```

**Expected**: Detailed statistics about archives and indices

### 5.5 File Listing

```bash
# Test: List files in storage (first 10)
cargo run --bin ngdp -- storage list <WOW_PATH> | head -10

# Test: Count total files
cargo run --bin ngdp -- storage list <WOW_PATH> | wc -l
```

**Expected**: List of file EKeys in the storage

### 5.6 File Reading

```bash
# Test: Read specific file by EKey (replace with actual EKey from list command)
cargo run --bin ngdp -- storage read <WOW_PATH> <EKEY> > /tmp/extracted-file

# Check file was extracted
ls -la /tmp/extracted-file
rm /tmp/extracted-file
```

**Expected**: File extracted successfully

---

## 6. Installation Commands

Test game installation functionality with .build.info and Data/config structure.

### 6.1 Metadata-Only Installation

```bash
# Test: Create metadata-only installation (safe, fast)
cargo run --bin ngdp -- install game wow_classic_era --install-type metadata-only --path /tmp/wow-meta-test

# Test: Check what was created
ls -la /tmp/wow-meta-test/
ls -la /tmp/wow-meta-test/Data/config/

# Test: View .build.info file
cat /tmp/wow-meta-test/.build.info
```

**Expected**: Creates .build.info and Data/config/ with CDN-style subdirectories containing build and CDN configurations

### 6.2 Minimal Installation

```bash
# Test: Minimal installation (dry run - safe)
cargo run --bin ngdp -- install game wow_classic_era --install-type minimal --path /tmp/wow-minimal --dry-run

# Test: Full dry run to see complete installation plan
cargo run --bin ngdp -- install game wow_classic_era --install-type full --path /tmp/wow-full --dry-run

# Test: JSON output for installation plan
cargo run --bin ngdp -- install game wow_classic_era --install-type minimal --path /tmp/wow-minimal --dry-run -o json
```

**Expected**: Shows installation plan with file counts and sizes

### 6.3 Resume Installation

```bash
# Test: Resume from metadata-only installation
cargo run --bin ngdp -- install game wow_classic_era --path /tmp/wow-meta-test --resume --dry-run

# Test: Resume after partial installation (if you have one)
cargo run --bin ngdp -- install game wow_classic_era --path /tmp/partial-install --resume
```

**Expected**: Detects existing .build.info, loads configuration, and resumes missing files

### 6.4 Installation Repair

```bash
# Test: Repair existing installation (dry run)
cargo run --bin ngdp -- install repair --path /tmp/wow-meta-test --dry-run

# Test: Repair with checksum verification
cargo run --bin ngdp -- install repair --path /tmp/wow-meta-test --verify-checksums --dry-run
```

**Expected**: Checks for missing or corrupted files and shows repair plan

---

## 7. Download Commands

Test downloading content from CDN servers with .build.info compatibility.

### 7.1 Resume Check

```bash
# Test: Check for resumable downloads in directory
cargo run --bin ngdp -- download resume /tmp

# Test: Resume from installation directory (with .build.info)
cargo run --bin ngdp -- download resume /tmp/wow-meta-test

# Test: Resume specific .download file
# cargo run --bin ngdp -- download resume /path/to/file.download
```

**Expected**: Reports resumable downloads or installation status

### 7.2 Build Download

⚠️ **Warning**: This will actually download data! Use a temporary directory and be prepared for large downloads.

```bash
# Test: Dry run build download (safe to run)
cargo run --bin ngdp -- download build wow_classic_era 61582 --output /tmp/wow-build-test --dry-run

# Test: Build download with tags filter
cargo run --bin ngdp -- download build wow_classic_era 61582 --output /tmp/wow-build-test --dry-run --tags Windows

# Test: Different region with dry run
cargo run --bin ngdp -- download build wow_classic_era 61582 --region eu --output /tmp/wow-build-test --dry-run
```

**Expected**: Creates Data/config/ structure, .build.info, and downloads configurations with CDN-style subdirectories

### 7.3 File Download

```bash
# Test: Download specific files by pattern (Note: patterns are positional arguments)
cargo run --bin ngdp -- download files wow_classic_era "*.exe"

# Test: Download with multiple patterns
cargo run --bin ngdp -- download files wow_classic_era "*.dll" "*.exe"
```

**Expected**: Downloads matching files

---

## 8. Certificate Management

Test certificate operations for CDN verification.

### 8.1 Certificate Download

```bash
# Test: Download certificate by SKI (Subject Key Identifier)
cargo run --bin ngdp -- certs download ribbit.version

# Test: Different format
cargo run --bin ngdp -- certs download ribbit.version --cert-format der

# Test: With details
cargo run --bin ngdp -- certs download ribbit.version --details
```

**Expected**: Certificate downloaded and optionally displayed

---

## 9. Listfile Management

Test community listfile operations for filename resolution.

### 9.1 Listfile Information

```bash
# Test: Show listfile info (will show error if not downloaded)
cargo run --bin ngdp -- listfile info

# Test: JSON output
cargo run --bin ngdp -- listfile info -o json
```

**Expected**: Either shows listfile stats or reports file not found

### 9.2 Listfile Download

```bash
# Test: Download community listfile
cargo run --bin ngdp -- listfile download

# Test: Check it was downloaded
cargo run --bin ngdp -- listfile info
```

**Expected**: Downloads community listfile and shows statistics

### 9.3 Listfile Search

```bash
# Test: Search for files (after downloading listfile)
cargo run --bin ngdp -- listfile search --pattern "*.exe"

# Test: Case insensitive search
cargo run --bin ngdp -- listfile search --pattern "WORLD" --case-insensitive

# Test: Limit results
cargo run --bin ngdp -- listfile search --pattern "*.dll" --limit 10
```

**Expected**: Shows matching filenames from the listfile

---

## 10. Complete Installation Workflow

Test the complete end-to-end installation workflow with .build.info and resume functionality.

### 10.1 Complete Installation Workflow

```bash
# Step 1: Create metadata-only installation
cargo run --bin ngdp -- install game wow_classic_era --install-type metadata-only --path /tmp/wow-complete-test
echo "✅ Created metadata-only installation"

# Step 2: Verify .build.info and Data/config structure
ls -la /tmp/wow-complete-test/
ls -la /tmp/wow-complete-test/Data/config/
cat /tmp/wow-complete-test/.build.info | head -2
echo "✅ Verified installation structure"

# Step 3: Resume installation to minimal
cargo run --bin ngdp -- install game wow_classic_era --path /tmp/wow-complete-test --resume --dry-run
echo "✅ Tested resume from metadata-only"

# Step 4: Test download resume compatibility
cargo run --bin ngdp -- download resume /tmp/wow-complete-test
echo "✅ Tested download resume with .build.info"

# Step 5: Test download build with .build.info creation
cargo run --bin ngdp -- download build wow_classic_era 61582 --output /tmp/wow-download-test --dry-run
echo "✅ Tested download build with .build.info generation"

# Step 6: Test repair functionality
cargo run --bin ngdp -- install repair --path /tmp/wow-complete-test --dry-run
echo "✅ Tested repair functionality"

# Cleanup
rm -rf /tmp/wow-complete-test /tmp/wow-download-test
echo "✅ Cleaned up test installations"
```

**Expected**: All commands work together seamlessly with .build.info and Data/config structure

### 10.2 Cross-Command Compatibility

```bash
# Create installation with download command
cargo run --bin ngdp -- download build wow_classic_era latest --output /tmp/wow-cross-test --dry-run

# Resume with install command
cargo run --bin ngdp -- install game wow_classic_era --path /tmp/wow-cross-test --resume --dry-run

# Resume with download command
cargo run --bin ngdp -- download resume /tmp/wow-cross-test

# Repair with install command
cargo run --bin ngdp -- install repair --path /tmp/wow-cross-test --dry-run
```

**Expected**: Commands are interoperable through shared .build.info format

---

## 🧪 Validation Tests

### Test Different Products

Verify commands work with other important products:

```bash
# Test with agent (Battle.net client)
cargo run --bin ngdp -- products info agent
cargo run --bin ngdp -- inspect cdn-config agent

# Test with bna (Battle.net app)
cargo run --bin ngdp -- products info bna
cargo run --bin ngdp -- inspect encoding bna --stats
```

### Test Different Regions

```bash
# Test European region
cargo run --bin ngdp -- products versions wow_classic_era --region eu
cargo run --bin ngdp -- inspect cdn-config wow_classic_era --region eu

# Test Asian regions
cargo run --bin ngdp -- products versions wow_classic_era --region kr
cargo run --bin ngdp -- products versions wow_classic_era --region cn
```

### Test Output Formats

```bash
# Test all output formats for a command
cargo run --bin ngdp -- products info wow_classic_era -o text
cargo run --bin ngdp -- products info wow_classic_era -o json
cargo run --bin ngdp -- products info wow_classic_era -o json-pretty
cargo run --bin ngdp -- inspect build-config wow_classic_era 61582 -o bpsv
```

---

## 🚨 Troubleshooting

### Common Issues

1. **Network Errors**: Check internet connection and CDN availability
2. **Build Not Found**: Use `products builds` to get valid build IDs
3. **Local Storage Errors**: Verify WoW installation path is correct
4. **Permission Errors**: Ensure read access to WoW directory and write access for downloads

### Debug Output

Add `-v` flag for verbose output on any command:

```bash
cargo run --bin ngdp -- -v products info wow_classic_era
```

### Cache Issues

Clear cache if you encounter stale data:

```bash
cargo run --bin ngdp -- --clear-cache products info wow_classic_era
```

---

## ✅ Success Criteria

All tests are successful if:

- ✅ No compilation errors
- ✅ No runtime panics or crashes
- ✅ Network commands return valid data
- ✅ JSON output is valid JSON
- ✅ File operations complete without errors
- ✅ All output formats work correctly

## 📊 Expected Performance

- **Product queries**: < 2 seconds
- **CDN configuration**: < 1 second
- **Build config download**: < 3 seconds
- **Encoding file analysis**: < 10 seconds (varies by build size)
- **Local storage operations**: < 5 seconds

---

## 🔗 Related Documentation

- [API Reference](api-reference.md) - Detailed API documentation
- [TACT Protocol](tact-protocol.md) - Understanding the underlying protocol
- [Ribbit Protocol](ribbit-protocol.md) - Product information protocol
- [Command Status Report](../COMMAND_STATUS.md) - Current implementation status

---

**Note**: This testing guide verifies the complete NGDP command suite. All commands have been tested and confirmed working with real Blizzard CDN data as of the latest implementation.

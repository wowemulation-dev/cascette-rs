# Glossary

Key terms used throughout this documentation. If you're coming from the
MPQ/3.3.5a modding scene, pay attention to the "MPQ Equivalent" notes.

## Content Identification

### Content Key (CKey)

MD5 hash of a file's uncompressed content. Used to identify files regardless
of how they're compressed or stored.

- **Size**: 16 bytes (128 bits)
- **MPQ Equivalent**: Similar to how MPQ uses filenames, but content-based
- **Example**: `a1b2c3d4e5f6...` (32 hex characters)

### Encoding Key (EKey)

MD5 hash of a file's compressed/encoded BLTE data. Used to locate files on
CDN and in archives.

- **Size**: 16 bytes (128 bits)
- **Relationship**: CKey → Encoding File → EKey
- **Example**: Files with identical content share a CKey but may have different EKeys

### FileDataID (FDID)

Numeric identifier for a file, persistent across game versions. Replaced
filename-based lookups in WoW 8.0+.

- **Size**: 4 bytes (32-bit integer)
- **Range**: 0 to ~4 million (as of 2024)
- **MPQ Equivalent**: None - MPQ used filenames exclusively
- **Example**: `1234567` refers to a specific texture, model, or data file

### Name Hash

Jenkins96 hash of a file's path. Used in older builds (pre-8.0) to look up
files by name.

- **Algorithm**: Jenkins96 (lookup3)
- **MPQ Equivalent**: Similar to MPQ's hash table for filename lookup
- **Note**: Deprecated in favor of FileDataID in modern builds

## File Formats

### BLTE (Block Table Encoded)

Container format that wraps all CASC content. Provides compression and
optional encryption.

- **MPQ Equivalent**: Similar to MPQ's sector-based compression
- **Key difference**: BLTE supports multiple compression algorithms per file
- **Compression**: None, zlib, LZMA, LZ4, Zstd
- **Encryption**: Salsa20, ARC4 (older builds)

### Encoding File

Maps CKeys to EKeys. The central lookup table for content resolution.

- **Purpose**: Find where a file's compressed data lives
- **MPQ Equivalent**: None - MPQ stored files directly by name

### Root File

Maps FileDataIDs (or name hashes) to CKeys. The entry point for file lookup.

- **Purpose**: Find what content hash a file has
- **MPQ Equivalent**: Combines MPQ's hash table and block table functions
- **Contains**: FileDataID, locale flags, content flags, CKey

### Install Manifest

Lists files required for a minimal installation (enough to launch the game).

- **Purpose**: Prioritize essential files for streaming installs
- **MPQ Equivalent**: None - MPQ required full downloads

### Download Manifest

Prioritizes files for background downloading after initial install.

- **Purpose**: Order non-essential downloads by importance
- **MPQ Equivalent**: None

## Storage Concepts

### Archive

Large file containing many compressed files, identified by EKey.

- **CDN archives**: ~256 MB bundles served via HTTP
- **Local archives**: `data.xxx` files in the Data folder
- **MPQ Equivalent**: Similar to .mpq files, but content-addressed

### Archive Index

Maps EKeys to offsets within an archive file.

- **CDN index**: `.index` file paired with each archive
- **Local index**: `.idx` files in `Data/indices/`
- **MPQ Equivalent**: Similar to MPQ's block table

### Archive Group

Combined index covering multiple archives. Optimization for faster lookups.

- **Location**: Generated locally by the client from downloaded archive indices
- **Purpose**: Single lookup instead of checking each archive index
- **Note**: Never downloaded from CDN - always client-generated

### CASC (Content Addressable Storage Container)

The local storage system. Everything is identified by content hash.

- **MPQ Equivalent**: Replaces MPQ archives entirely
- **Key difference**: Files found by hash, not by name

## Network Concepts

### CDN (Content Delivery Network)

Servers that host game content. Blizzard uses Akamai, Level3, and others.

- **Structure**: `https://{cdn}/{product}/{type}/{hash[:2]}/{hash[2:4]}/{hash}`
- **Types**: config, data, patch

### Ribbit

Protocol for querying product versions and CDN information.

- **Port**: 1119 (TCP) or HTTP
- **Purpose**: Discover what versions exist and where to download them
- **MPQ Equivalent**: None - MPQ versions were distributed manually

### Agent

Local HTTP service (port 1120) that manages downloads and installations.

- **Purpose**: Background downloading, installation management
- **MPQ Equivalent**: None - MPQ required manual patching

## Configuration

### Build Config

Per-build settings including root/encoding file hashes and encryption keys.

- **Location**: CDN `/config/{hash}`
- **Contains**: Root CKey, encoding CKey, patch info, VFS info

### CDN Config

Lists available CDN servers and archive hashes.

- **Location**: CDN `/config/{hash}`
- **Contains**: Archive list, server URLs, file groups

### Product Config

Product-wide settings spanning multiple builds.

- **Location**: CDN `/config/{hash}`
- **Contains**: Decryption keys, feature flags

## Encryption

### TACT Key

Encryption key for protected content. Named keys are published, unnamed are secret.

- **Size**: 16 bytes
- **Algorithm**: Used with Salsa20 stream cipher
- **Source**: Community-maintained key databases

### Salsa20

Stream cipher used for content encryption in modern builds.

- **Key size**: 256 bits (16-byte key + 16-byte name as nonce)
- **Replaces**: ARC4 (used in older builds)

## MPQ to CASC Quick Reference

| MPQ Concept | CASC Equivalent |
|-------------|-----------------|
| .mpq file | Archive (data.xxx) |
| Filename | FileDataID or CKey |
| Hash table | Root file |
| Block table | Archive index |
| Sector compression | BLTE blocks |
| Patch MPQ | Patch archives + encoding |
| listfile.txt | Community listfiles |
| Manual patching | Agent + CDN |

## See Also

- [NGDP on wowdev.wiki](https://wowdev.wiki/NGDP)
- [CASC on wowdev.wiki](https://wowdev.wiki/CASC)
- [TACT on wowdev.wiki](https://wowdev.wiki/TACT)

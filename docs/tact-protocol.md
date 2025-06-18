# TACT Protocol Documentation

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [HTTP Communication (Protocol Version 1)](#http-communication-protocol-version-1)
  - [Version Server Endpoints (v1)](#version-server-endpoints-v1)
- [HTTPS Communication (Protocol Version 2)](#https-communication-protocol-version-2)
  - [Version Server Endpoints (v2)](#version-server-endpoints-v2)
- [CDN Content URLs](#cdn-content-urls)
- [Data Formats](#data-formats)
  - [Manifest File Formats](#manifest-file-formats)
  - [Configuration File Formats](#configuration-file-formats)
  - [Binary File Formats](#binary-file-formats)
- [System Files](#system-files)
- [Archive System](#archive-system)
- [BLTE Encoding](#blte-encoding)
- [Hash System](#hash-system)
- [Encryption](#encryption)
- [Example URLs](#example-urls)
- [Important Notes](#important-notes)

## Overview

TACT (Trusted Application Content Transfer) is Blizzard's content transfer protocol
used as part of NGDP (Next Generation Data Pipeline). TACT handles the distribution
and downloading of game content from CDN servers. It works alongside CASC (Content
Addressable Storage Container) for local storage.

## Key Features

### Content Distribution

- HTTP/CDN-based file distribution
- Region and locale-specific content delivery
- Multi-platform support (Windows, Mac, etc.)
- Supports multiple Blizzard games

### File Management

- Archive and loose file handling
- File indexing system (.index files)
- Fragment and compressed storage support
- Content hash-based addressing

### Compression & Encryption

- BLTE encoding for compression
- Salsa20 encryption support
- Keychain-based encryption key management
- Encrypted/unencrypted file variants

### Versioning & Patching

- Multiple version and build support
- Patch manifest system
- Partial file downloads
- Priority-based file updates

### Technical Features

- Tag-based file selection
- Shared storage containers
- Configuration files (build, CDN, patch)
- Hash-based file lookups

## HTTP Communication (Protocol Version 1)

TACT originally used HTTP for retrieving version information and content from CDN
servers. This HTTP-based communication on port 1119 is considered version 1 of the
protocol.

### Version Server Endpoints (v1)

Base URL: `http://{region}.patch.battle.net:1119/{product}/`

Where `{region}` can be:

- `us` - United States
- `eu` - Europe
- `cn` - China
- `kr` - Korea
- `tw` - Taiwan
- `sg` - Singapore

Where `{product}` can be:

### Battle.net Products

- `agent` - Battle.net Agent/Launcher

### World of Warcraft Products

- `wow` - World of Warcraft (Retail)
- `wow_beta` - World of Warcraft Beta
- `wow_classic` - World of Warcraft Classic
- `wow_classic_beta` - World of Warcraft Classic Beta
- `wow_classic_era` - World of Warcraft Classic Era
- `wow_classic_era_ptr` - World of Warcraft Classic Era PTR
- `wow_classic_ptr` - World of Warcraft Classic PTR
- `wowlivetest` - World of Warcraft Live Test
- `wowt` - World of Warcraft Public Test Realm (PTR)
- `wowxptr` - World of Warcraft Experimental PTR
- `wowz` - World of Warcraft Internal/Development

Available endpoints:

| Endpoint | Description |
|----------|-------------|
| `/cdns` | Returns CDN domains and paths for each region |
| `/versions` | Returns current version information including build configs |
| `/bgdl` | Returns background downloader information (often empty) |
| `/blobs` | ❌ Non-functional - returns "Bad Request: Request contains invalid file type" |
| `/blob/game` | ❌ Non-functional - returns "Not Found" |
| `/blob/install` | ❌ Non-functional - returns "Not Found" |

## HTTPS Communication (Protocol Version 2)

Since 2024, version 2 of the protocol uses HTTPS with a new base URL structure.

### Version Server Endpoints (v2)

Base URL: `https://{region}.version.battle.net/v2/products/{product}/`

Where `{region}` uses the same values as v1: `us`, `eu`, `cn`, `kr`, `tw`, `sg`

Where `{product}` uses the same identifiers as v1 (see above)

The v2 protocol supports the same endpoints as v1:

- `/cdns`
- `/versions`
- `/bgdl`

## CDN Content URLs

CDN content is organized using a specific URL pattern:

```text
http://{cdnsHost}/{cdnsPath}/{pathType}/{firstTwoHex}/{secondTwoHex}/{fullHash}
```

Where:

- `{cdnsHost}` - CDN hostname (e.g., `blzddist1-a.akamaihd.net`)
- `{cdnsPath}` - Product-specific path (e.g., `tpr/wow`)
- `{pathType}` - Type of content:
  - `config` - Configuration files (build, CDN, patch configs)
  - `data` - Game data (archives, indexes, loose files)
  - `patch` - Patch data (manifests, patch archives)
- `{firstTwoHex}` - First two characters of the content hash
- `{secondTwoHex}` - Third and fourth characters of the content hash
- `{fullHash}` - Complete hash of the content

### CDN File Types

1. **Configuration Files** (`/config/`):
   - Build Config: Contains keys for root, encoding, install, download files
   - CDN Config: Lists all data archives and patch archives
   - Patch Config: Contains patch-specific information

2. **Data Files** (`/data/`):
   - Archives: Compressed game data (max 256MB per archive)
   - Index files: `.index` suffix, maps encoding keys to archive locations
   - System files: Encoding, root, install, download files

3. **Patch Files** (`/patch/`):
   - Patch archives: Delta updates
   - Patch indices: `.index` suffix for patch content maps

## Data Formats

### Manifest File Formats

Manifest files use PSV (Pipe-Separated Values) format with typed columns. All responses include a sequence number comment (e.g., `## seqn = 3014093`) for version tracking.

#### CDNs Response Format

```text
Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4&fallback=1 https://level3.ssl.blizzard.com/?maxhosts=4&fallback=1|tpr/configs/data
eu|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4&fallback=1 https://eu.cdn.blizzard.com/?maxhosts=4&fallback=1|tpr/configs/data
```

Note: The `Servers` column contains full URLs with query parameters for each host.

#### Versions Response Format

```text
Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16
us|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
eu|dcfc289eea032df214ebba097dc2880d|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61265|11.1.5.61265|53020d32e1a25648c8e1eafd5771935f
```

Note: `VersionsName` has inconsistent type casing (`String:0` instead of `STRING:0`).

### Configuration File Formats

Configuration files use a key-value format with optional multi-value support.

#### Build Config Format

```text
root = 9e3dfbafb41949c8cb14e0bc0055d225 70c91468bb187cc2b3d045d476c6899f
encoding = e468c86f90cd051195a3c5f8b08d7bd7 12ad2799f3e1ee9a9b5620e43a0d2b75
install = 17adc9e821c34e06ba6f4568aab0c040 9a127c8076a2c1b24fa3a97b0f5346d8
download = f2c3b74f3c51db3a5c4e2d87c52a0c82 24e1cd9ec87419dd826e991fa141c6e0
size = b3032861e246c30c6e26581030053f87 2fb554d26bb6edd6339a03e9d6faabf8
patch = 87c4820cfb7479176dd2155aed518994
patch-config = 4c086cce3c9b7e11956435aea1e0d77f
```

Note: Each value contains two MD5 hashes - first is CKey (content), second is EKey (encoding).

#### CDN Config Format

```text
archives = 8a41b9e8bf2d85ad73e087c446c655fb f3cfbcc740f8d638b6e42c6c5fd95163 ...
archives-index-size = 73613720 73920280 ...
patch-archives = 6a506d3eb8e6dc7e22ac434428ad3b73 ...
patch-archives-index-size = 94742 ...
file-index = 0000000000000000000000000000000000000000000
file-index-size = 0 0
```

### Binary File Formats

All binary files are stored in BLTE-encoded format (see BLTE Encoding section).

## System Files

TACT uses several system files to manage content:

1. **Encoding File** (`.encoding`):
   - Maps Content Keys (CKeys) to Encoding Keys (EKeys)
   - Contains encoding specifications (ESpec)
   - Binary format with header and page tables

2. **Root File** (`.root`):
   - Maps file paths/IDs to Content Keys
   - Supports locale and content flags
   - Version 1 or 2 format

3. **Install File** (`.install`):
   - Installation manifest with file tags
   - Binary format with header and entries

4. **Download File** (`.download`):
   - Download priorities and tags
   - Versions 1, 2, or 3 format

5. **Download Size File** (`.size`):
   - File size information
   - Only for builds > 27547

6. **Patch File** (`.patch`):
   - Delta update information
   - Binary format

## Archive System

### Index Files

Index files (`.index`) map encoding keys to archive locations:

- **Header**: Version, offset bytes, entry count, checksums
- **Entries**: `[EKey(9 bytes)][Archive][Offset][Size]`
- **Types**: Data, Patch, Loose, Group

### Archive Files

- Maximum size: 256MB (256,000,000 bytes)
- Contain BLTE-encoded file data
- Referenced by index entries

## BLTE Encoding

BLTE (Block Table Encoding) is TACT's compression format:

- **Magic Number**: `0x45544C42` ("BLTE")
- **Encoding Types**:
  - `N` (0x4E): None - Uncompressed
  - `Z` (0x5A): ZLib - Standard compression
  - `F` (0x46): Frame - Frame-based encoding
  - `E` (0x45): Encrypted - Encrypted blocks

### ESpec Format

Encoding specifications: `<size><type>[:{parameters}]`

Examples:

- `256k*z` - 256KB file with zlib compression
- `4k*n` - 4KB uncompressed file
- `z:{9}` - ZLib with compression level 9

### Compression Strategy

- **No compression**: PNG, MP3, OGG, AVI, TTF
- **MPQ level 6**: BLP, M2, MDX, WMO, ADT
- **ZLib level 9**: Everything else

## Hash System

TACT uses MD5 hashes (16 bytes, 32 hex characters) for content addressing:

- **CKey (Content Key)**: Identifies raw file content
- **EKey (Encoding Key)**: Identifies encoded/compressed data
- **Hash Format**: Lowercase hexadecimal string

Path hashing uses Jenkins/Lookup3 algorithm:

- Normalize paths: uppercase, backslashes
- 64-bit output hash

## Encryption

### Armadillo Encryption

- Optional CDN response encryption
- Uses `.ak` key files
- Applied at transport level

### Salsa20 Encryption

- File-level encryption
- Key sizes: 128 or 256 bits
- IV: 8 bytes (derived from CDN hash)
- Hardcoded keys in KeyService

## Example URLs

Version endpoints (v1):

```text
http://us.patch.battle.net:1119/wow/versions
http://eu.patch.battle.net:1119/wow/versions
http://kr.patch.battle.net:1119/wow/versions
```

Version endpoints (v2):

```text
https://us.version.battle.net/v2/products/wow/versions
https://eu.version.battle.net/v2/products/wow/versions
https://kr.version.battle.net/v2/products/wow/versions
```

CDN config file:

```text
http://blzddist1-a.akamaihd.net/tpr/wow/config/22/38/2238ab9c57b672457a2fa6fe2107b388
```

CDN data file:

```text
http://blzddist1-a.akamaihd.net/tpr/wow/data/00/52/0052ea9a56fd7b3b6fe7d1d906e6cdef.index
```

## Implementation Considerations

### Endpoint Availability

1. **Working Endpoints**:
   - `/cdns` - Returns CDN configuration for all regions
   - `/versions` - Returns version information with build configs
   - `/bgdl` - Returns headers but often no data rows

2. **Non-functional Endpoints**:
   - `/blobs` - Returns "Bad Request: Request contains invalid file type"
   - `/blob/game` - Returns "Not Found"
   - `/blob/install` - Returns "Not Found"

### Response Variations

1. **Column Additions**: Current responses include extra columns not in older documentation:
   - Versions: `KeyRing!HEX:16` and `ProductConfig!HEX:16`
   - CDNs: `Servers!STRING:0` with full URLs including query parameters

2. **Empty Responses**: The bgdl endpoint may return only headers without data
   - Not all products have bgdl data (e.g., wow_classic_era returns 404)

3. **Type Inconsistencies**: Watch for `VersionsName!String:0` vs standard `STRING:0`

### Regional Data

- Standard regions in responses: `us`, `eu`, `cn`, `kr`, `tw`
- Additional regions for some products: `sg` (Singapore), `xx` (internal/fallback)
- China (`cn`) often has different builds and versions
- The `xx` region appears in data but is not a valid endpoint

### Path Patterns

- All WoW products share the same CDN path: `tpr/wow`
- Config path is consistently: `tpr/configs/data`
- Path parameter in CDNs response is always `tpr/wow` regardless of product

## Important Notes

- CDN configurations can change frequently. Clients should dynamically fetch current
  CDN information from the `/cdns` endpoint.
- Since 2019, these HTTP endpoints (protocol v1) have been transitioning to Ribbit V2 protocol wrappers.
- The HTTP protocol on port 1119 is considered legacy, with Ribbit V2 being the current standard.
- Content is addressed by hash, allowing for efficient caching and deduplication.
- TACT v2 (HTTPS) endpoints return identical data format to v1, just over HTTPS instead of HTTP.

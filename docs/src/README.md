# NGDP Documentation

## Introduction to NGDP

NGDP (Next Generation Distribution Pipeline) is Blizzard Entertainment's
content distribution system. It provides an API for product information and
updates, with data delivered through regionalized CDNs. NGDP replaced the
MPQ/P2P/Torrent-based distribution system with the release of
[World of Warcraft 6.0](https://warcraft.wiki.gg/wiki/Patch_6.0) in 2014.

For technical details, see [NGDP on wowdev.wiki](https://wowdev.wiki/NGDP).

## System Overview

NGDP consists of two components:

1. **Ribbit API**: Provides product versions, CDN endpoints, and configuration
data
2. **CDN Distribution**: Delivers game content through HTTP/HTTPS

## Key Differences from MPQ

- **Distribution Method**: CDN-based delivery instead of P2P/Torrent

- **Content Addressing**: Files identified by content hashes rather than names

- **Update Mechanism**: Incremental updates through partial file downloads

- **Archive Format**: CASC (Content Addressable Storage Container) replaces MPQ
archives

- **Content Protection**: Encryption support for secure pre-release distribution

## Benefits of NGDP

### For Distribution

- **Reduced Server Load**: CDN infrastructure handles content delivery

- **Faster Downloads**: Users connect to nearest CDN nodes

- **Incremental Updates**: Only changed content needs downloading

- **Parallel Downloads**: Multiple files retrieved simultaneously

- **Pre-release Distribution**: Encrypted content can be distributed before
launch

### For Development

- **Content Deduplication**: Identical files stored once

- **Version Management**: Multiple game versions share common assets

- **Stream Installation**: Games playable before download completes

- **Platform Independence**: Same content system across operating systems

- **Content Protection**: Encryption prevents early access to unreleased content

## Core Concepts

### Content Addressing

Files are identified by cryptographic hashes of their content. This enables:

- Automatic deduplication

- Integrity verification

- Cache efficiency

### System Files

NGDP uses metadata files to manage content:

- **Root File**: Maps game files to content keys

- **Encoding File**: Maps content to compressed versions

- **Install Manifest**: Defines installation requirements

- **Download Manifest**: Sets download priorities

### BLTE Format

BLTE (Block Table Encoded) is the container format for game data. It supports:

- Block-based compression

- Multiple compression algorithms

- Encryption per block

- Chunked processing

### Content Encryption

NGDP supports encryption for:

- Pre-release content distribution

- Protecting unreleased game data

- Secure content delivery before activation

## Technical Specifications

- **Byte Order**: Big-endian (network byte order)

- **Hash Algorithm**: MD5 for content identification

- **Key Size**: 128-bit (16 bytes)

- **Compression**: zlib, lz4, and other algorithms per block

- **Encryption**: Salsa20 stream cipher for content protection

## Format Organization

NGDP/CASC formats are organized by their storage location and usage context:

### 1. CDN Formats (Network/Remote)

Formats served by Blizzard CDN servers via HTTP/HTTPS.

### 2. CASC Formats (Local/Client)

Formats created and managed by the Battle.net client on local storage.

### 3. Shared Formats

Formats used in both CDN and local contexts.

## Component Documentation

### Service Discovery

Service discovery components handle version information, CDN endpoint discovery,
and product configuration metadata:

- [Ribbit Protocol](ribbit.md) - TCP-based discovery and version information API

- [BPSV Format](bpsv.md) - Blizzard Pipe-Separated Values format for API
responses

### CDN Formats

#### Configuration Files (Text)

- [Build Config](config-formats.md#build-configuration) - Build-specific

  settings (`/config/{hash}`)

- [CDN Config](config-formats.md#cdn-configuration) - CDN server and archive

  lists (`/config/{hash}`)

- [Product Config](config-formats.md#product-configuration) - Product settings

  and versions (`/config/{hash}`)

- [Patch Config](config-formats.md#patch-configuration) - Differential patch

  information (`/config/{hash}`)

#### Content Files (Binary)

Immutable, content-addressed files served from CDN:

- [CDN Archives](archives.md) - BLTE containers with game content

  (`/data/{prefix}/{hash}.archive`)

- **CDN Indices** - Maps keys to archive locations
(`/data/{prefix}/{hash}.index`)

- [Encoding File](encoding.md) - Maps content to encoding keys
(`/data/{prefix}/{hash}`)

- [Root File](root.md) - Maps files to content keys (`/data/{prefix}/{hash}`)

- [Install Manifest](install.md) - Installation requirements
(`/data/{prefix}/{hash}`)

- [Download Manifest](download.md) - Download priorities

  (`/data/{prefix}/{hash}`)

- [Patch Archives](patches.md) - Delta patches
(`/patch/{prefix}/{hash}.archive`)

- [Patch Indices](patches.md) - Patch archive index
(`/patch/{prefix}/{hash}.index`)

#### Modern Additions (WoW 8.2+)

- [TVFS](tvfs.md) - Virtual file system manifest (via `vfs-*` fields in
BuildConfig)

### CASC Local Formats

Client-side storage structures created and managed by Battle.net:

#### Local Indices

- **IDX Journal** - Bucket-based local index (`Data/indices/{bucket}.idx`)

- **Archive Groups** - Combined archive index (client-generated optimization)

- **Shadow Memory** - Memory-mapped cache (`Data/shmem`)

#### Local Archives

- **data.###** - Combined CDN archives (`Data/data/data.###`)

- **patch.###** - Combined patch archives (`Data/patch/patch.###`)

#### Local Configuration

- **.build.info** - Local build configuration (root directory)

- **DBCache** - Hotfix database cache (`Cache/ADB/*.bin`)

### Shared Formats

#### Container Formats

- [BLTE Format](blte.md) - Block compression/encryption (all content storage)

- [ESpec Format](espec.md) - Encoding specifications (compression definitions)

#### Cryptographic

- **MD5 Keys** - Content addressing (all key references)

- [Salsa20 Encryption](salsa20.md) - Stream cipher (content protection)

- **TACT Keys** - Key management (decryption keys)

#### Supporting Systems

- [CDN Architecture](cdn.md) - Content distribution network structure

- [CDN Mirroring](mirroring.md) - Historical preservation strategies

- **FileDataId** - Persistent file identification across builds

## Format Relationships

### CDN Download Flow

```text
Ribbit (BPSV) → Product Config → CDN Config → Build Config
                                      ↓
                              CDN Archives + Indices
                                      ↓
                              Encoding File → Root File
                                      ↓
                              Install/Download Manifests
```

### Content Resolution

```text
Filename/FileDataId → Root File → Content Key
Content Key → Encoding File → Encoding Key + ESpec
Encoding Key → CDN Index → Archive Location
Archive Location → CDN Archive → BLTE Data
BLTE Data → Decompression → Raw Content
```

## Implementation Notes

### Key Format Discoveries

1. **CDN Index Format**: Uses 20-byte footer with MD5 hashing, not 32-byte with
Jenkins96
2. **Entry Count**: Little-endian in CDN indices (exception to CASC big-endian
convention)
3. **Archive Groups**: Client-side optimization, not provided by CDN
4. **Page Alignment**: CDN indices use 4KB pages for efficient memory management
5. **Key Truncation**: Some formats use partial keys for space efficiency

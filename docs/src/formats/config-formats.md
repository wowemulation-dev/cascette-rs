# NGDP Configuration File Formats

This document describes the configuration file formats used in NGDP for managing
product versions, CDN endpoints, and content distribution.

## Overview

NGDP uses four primary configuration file types:

1. **Build Configuration** - Defines build metadata and system file references
2. **CDN Configuration** - Lists CDN servers and available archives
3. **Patch Configuration** - Contains delta update information
4. **Product Configuration** - Client installation and platform metadata

## Configuration File Access

Configuration files are accessed through CDN endpoints using content-addressed
paths derived from hashes returned by the Ribbit API.

### Path Structure

Configuration files use a two-level directory structure for efficient CDN
distribution:

```text
http://<cdn-host>/<path>/<type>/<hash[0:2]>/<hash[2:4]>/<full-hash>
```

Where:

- `<cdn-host>`: CDN server hostname

- `<path>`: Base path from CDN response (e.g., `tpr/wow`)

- `<type>`: Content type (`config`, `data`, `patch`)

- `<hash[0:2]>`: First 2 characters of hash

- `<hash[2:4]>`: Characters 3-4 of hash (positions 2-3 in 0-indexed)

- `<full-hash>`: Complete hash value

Example:

```text
# Build config for wow_classic_era 1.15.7.61582
# Hash: ae66faee0ac786fdd7d8b4cf90a8d5b9
# Note: hash[0:2] = "ae", hash[2:4] = "66"
http://cdn.arctium.tools/tpr/wow/config/ae/66/ae66faee0ac786fdd7d8b4cf90a8d5b9
```

## Build Configuration

Build configurations define build-specific metadata and reference all system
files required for a build.

### Format

Key-value pairs, one per line, with `=` delimiter.

### Common Keys

| Key | Description | Example |
|-----|-------------|---------|
| `root` | Root file content key (NOT for direct CDN fetch) | `ea8aefdebdbd6429da905c8c6a2b1813` |
| `install` | Install manifest: content key + encoding key | `54c189d60033f93f42e7b91165e7de1c a9dcee49ab3f952d69441eb3fd91c159` |
| `encoding` | Encoding file: content key + **encoding key** (use 2nd for CDN) | `b07b881f4527bda7cf8a1a2f99e8622e bbf06e7476382cfaa396cff0049d356b` |
| `encoding-size` | Sizes for encoding file versions | `14004322 14003043` |
| `download` | Download manifest: content key + encoding key | `42a7bb33cd1e9a7b72bef6ee14719b58 53ba96f0965adc306d2d0cf3b457949c` |
| `size` | Size file: content key + encoding key | `d1d9e612a645cc7a7e4b42628bde21ce 0d5704735f4985e555907a7e7647099a` |
| `patch` | Patch file content key | `658506593cf1f98a1d9300c418ee5355` |
| `patch-config` | Patch configuration hash (fetch separately) | `17f5bbcb7eae2fc8fb3ea545c65f74d4` |
| `patch-index` | Patch index files | `3806f4c7b1f179ce976d7685f9354025 eb5758bd78805f0aabac15cf44ea767c` |
| `patch-size` | Size of patch file | `22837` |
| `build-name` | Human-readable build identifier | `WOW-55646patch1.15.3_ClassicRetail` |
| `build-uid` | Unique build identifier | `wow_classic_era` |
| `build-product` | Product identifier | `WoW` |
| `build-playbuild-installer` | Installer build number | `ngdp:wow_classic_era:55646` |
| `build-partial-priority` | Partial download priorities | Space-separated list |

### VFS (Virtual File System) Keys

Modern WoW builds (8.2+) include VFS fields that reference TVFS (TACT Virtual
File System) manifests:

| Key | Description | Example |
|-----|-------------|---------|
| `vfs-root` | Main TVFS manifest: content key + encoding key | `fd2ea24073fcf282cc2a5410c1d0baef 14d8c981bb49ed169e8558c1c4a9b5e5` |
| `vfs-root-size` | Sizes for TVFS root manifest | `50071 33487` |
| `vfs-1` through `vfs-N` | Additional TVFS manifests for different products/regions | Same format as vfs-root |
| `vfs-N-size` | Size for corresponding VFS manifest | Same format as vfs-root-size |

**Important**: Each `vfs-*` field points to a TVFS manifest file that contains
the virtual file system structure. These manifests are BLTE-encoded and
fetched using the encoding key (second hash). See [TVFS documentation](tvfs.md)
for manifest format details.

Modern builds can have 1,500+ VFS fields representing different:

- Product variants (retail, PTR, beta)

- Language/region combinations

- Platform-specific configurations

- Feature flags and optional content

### Example

```text
# Build Configuration for wow_classic_era 1.15.7.61582
# URL: http://cdn.arctium.tools/tpr/wow/config/ae/66/ae66faee0ac786fdd7d8b4cf90a8d5b9
root = ea8aefdebdbd6429da905c8c6a2b1813
install = 54c189d60033f93f42e7b91165e7de1c a9dcee49ab3f952d69441eb3fd91c159
install-size = 23038 22281
download = 42a7bb33cd1e9a7b72bef6ee14719b58 53ba96f0965adc306d2d0cf3b457949c
download-size = 5606744 4818287
size = d1d9e612a645cc7a7e4b42628bde21ce 0d5704735f4985e555907a7e7647099a
size-size = 3637629 3076687
encoding = b07b881f4527bda7cf8a1a2f99e8622e bbf06e7476382cfaa396cff0049d356b
encoding-size = 14004322 14003043
patch-index = 5472ee24b5b9d148acfd2a436fc514be 76ce88ecb704dc93849def9fb489a6fb
patch-index-size = 16783 6591
patch = 4f185b4a837d4a363b2490432aaef092
patch-size = 11017
patch-config = 474b9630df5b46df5d98ec27c5f78d07
build-name = WOW-61582patch1.15.7_ClassicRetail
build-uid = wow_classic_era
build-product = WoW
build-playbuild-installer = ngdptool_casc2
```

### Critical Implementation Note

**ENCODING KEY VS CONTENT KEY**:

- Most build config entries have TWO hashes: `<content-key> <encoding-key>`

- The **content key** (first hash) is the unencoded file identifier

- The **encoding key** (second hash) is what you use for CDN fetches

- **EXCEPTION**: The encoding file itself can be fetched directly using its

  encoding key

**File Fetch Process**:

1. Fetch encoding file using its encoding key:
`bbf06e7476382cfaa396cff0049d356b`
2. Parse encoding file to find encoding keys for other files
3. Use those encoding keys to fetch files from CDN
4. The root file CANNOT be fetched using `ea8aefdebdbd6429da905c8c6a2b1813`
directly

### Notes

- Multiple encoding/size entries support different compression levels

- Patch-config reference enables delta updates between builds

- Build-partial-priority lists files for streaming installation

## CDN Configuration

CDN configurations list available CDN servers and archive files.

### CDN Configuration Format

Key-value pairs with special handling for multi-value keys.

### Keys

| Key | Description | Format |
|-----|-------------|--------|
| `archives` | List of archive hashes | Space-separated |
| `archive-group` | Group identifier for archives | Single hash |
| `patch-archives` | List of patch archive hashes | Space-separated |
| `patch-archive-group` | Group identifier for patch archives | Single hash |
| `file-index` | File index hash | Single hash |
| `file-index-size` | Size of file index | Integer |
| `patch-file-index` | Patch file index hash | Single hash |
| `patch-file-index-size` | Size of patch file index | Integer |
| `builds` | Reference to builds using this CDN config | Space-separated |

### CDN Configuration Example

```text
# CDN Configuration for wow_classic_era 1.15.7.61582
# URL: http://cdn.arctium.tools/tpr/wow/config/63/ee/63eee50d456a6ddf3b630957c024dda0
# (Showing first 10 archives of 1000+)
archives = 0017a402f556fbece46c38dc431a2c9b 003b147730a109e3a480d32a54280955 \
  00b79cc0eebdd26437c7e92e57ac7f5c 00e43d6a55fe497ebaecece75c464913 \
  00f71443fef647344027dd37beda651f 0105f03cb8b8faceda8ea099c2f2f476 \
  0128ec2c42df9e7ac7b58a54ad902147 01794f476dce0d0adeb975eaff4ff850 \
  01df479cca2ad2a8991bac020db5287e 01f0908f6ece2f26d918d1665f919222
archive-group = 58a3c9e02c964b0ec9dd6c085df99a77
patch-archives = 01c87e5f5e87ffc088c3fe20a7e332ce
0239bc973b31a4e52e8c96652a14b9e0 \
  034e2e6e0e5cdecb0f0bc07e87f0e074 04f8e6c8cbfbd6e9fd3e9ccbcd95e53a \
  0662e1cf69dbd0c6c10e7e3e6303b8cf 0bffd45f01e8ad33731f973bb96f3db1 \
  0d17c61fa98e6db91e14e0b24c8bc9f9 0d47f019c36e88c00fc43b3fe973f3d1 \
  101e4f7b592c12bf3c436d3b95e38b8f 1027ab37f63c039a8a3dd8a039e43e81
patch-archive-group = de09c9cd5f93c4e4f6f1f0f4a8edb9c0
file-index = fb37bc7303bae99d6c57e96a079e2c77
file-index-size = 34236152
patch-file-index = eb99f93d5c8dbdbb652f1d71da9c7de6
patch-file-index-size = 5015068
builds = ae66faee0ac786fdd7d8b4cf90a8d5b9
```

### Archive Management

- Archives are immutable once created

- New content creates new archives

- Archive-group combines multiple archives for efficient access

- File-index provides fast lookups across all archives

## Patch Configuration

Patch configurations define delta updates between builds. They are referenced
within build configurations using the `patch-config` field and contain detailed
patch entry definitions.

### Access Pattern

Patch configs are accessed through:

1. Fetch build config
2. Extract `patch-config` hash from build config
3. Fetch patch config using standard config path structure

### Patch Configuration Format

Text format with metadata and multiple `patch-entry` lines.

### Patch Entry Format

```text
patch-entry = <type> <content-key> <size> <encoding-key> <encoded-size>
[compression-info] [additional-keys...]
```

### Fields

| Field | Description |
|-------|-------------|
| `type` | File type (encoding, install, download, size, vfs:*) |
| `content-key` | Target content key |
| `size` | Target file size |
| `encoding-key` | Encoded version key |
| `encoded-size` | Encoded file size |
| `compression-info` | Compression blocks (e.g., `b:{11=n,4813402=n,793331=z}`) |
| `additional-keys` | Alternative encoding keys and sizes |

### Patch Configuration Example

```text
# Patch Configuration for wow_classic 1.13.7.38631
# URL: http://cdn.arctium.tools/tpr/wow/config/17/f5/17f5bbcb7eae2fc8fb3ea545c65f74d4
# (Showing metadata and sample entries)

# Patch Configuration

patch = 658506593cf1f98a1d9300c418ee5355
patch-size = 22837

patch-entry = download 6d616efdfd334916898276805f043927 6113132 \
  64332f9899b6d42a939fa3e02080bf33 5528795 b:{16=n,5524659=n,588457=z} \
  0a45352357be8ddca09749ec421bbb48 6112126 50ac209d796a11818da1429d6cb69c60
12502
patch-entry = encoding fcf166e21580ee48497b4d85e433b900 13084283 \
  716906f960db61ea62f07f7e9697127d 13082541
b:{22=n,2574=z,61216=n,7835648=n,40192=n,5144576=n,*=z} \
  5905362dbda48cebbea7c80d05ef6c60 13084283 ce2c3294ca7e37aa3be1f227bdc9072a
89156
patch-entry = install 179088c6b3495b1a9dec3715e77834e1 15565 \
  a75d4aa7e38dff6a1ddc59bd80c2ad3c 15197 b:{610=z,14955=n} \
  f66d038c20f580be307f4645c7b5d3f2 15633 072a9339d594a00c884ffea987381883 486
patch-entry = size 5841844a1a1ad48eaeb756c716869bf5 3248493 \
  d06fc7a7e4b5d8fb138a2ee27f54674f 2878957 b:{15=n,588457=z,64K*=n} \
  2061f6427c842d01d9445d1bcc58d65b 3247949 daccd8bf9f2719ea9dbbb57991a03ed7
452303
```

### Compression Info Format

The `b:{...}` notation describes block compression:

- `n` = uncompressed block

- `z` = zlib compressed block

- Numbers indicate block sizes or offsets

- `*` = all remaining blocks

- `64K*` = 64KB blocks

### Entry Types

Patch configs commonly include:

- **System files**: `download`, `encoding`, `install`, `size`, `patch-index`

- **VFS entries**: `vfs:*` with hexadecimal identifiers (e.g.,
`vfs:000000040000::`)

- **Metadata**: `patch` and `patch-size` fields for the patch file itself

### Availability

Patch configs are found in:

- Classic WoW builds (1.13.x through 5.5.x)

- Older retail builds (pre-8.0)

- Rarely in modern builds (mostly replaced by direct patching)

## Product Configuration

Product configurations contain Battle.net client metadata for installation
and platform requirements.

**Note**: Product config hashes are present in Ribbit/Wago data, and the actual
config files are accessible via CDN using the `/tpr/configs/data/` path
structure
as demonstrated in the examples below.

### Product Configuration Format

JSON object with nested configuration sections.

### Structure

```json
{
  "all": {
    "config": {
      // Global configuration
    }
  },
  "platform": {
    "win": { /* Windows-specific */ },
    "mac": { /* macOS-specific */ }
  },
  "<locale>": {
    "config": {
      // Locale-specific configuration
    }
  }
}
```

### Product Configuration Example

```json
// Product Configuration for WoW 11.2.0.62748
// URL: http://cdn.arctium.tools/tpr/configs/data/53/02/53020d32e1a25648c8e1eafd5771935f
{
  "all": {
    "config": {
      "product": "WoW",
      "update_method": "ngdp",
      "data_dir": "Data/",
      "supports_multibox": true,
      "supports_offline": false,
      "supported_locales": ["enUS", "esMX", "ptBR", "deDE", "esES", "frFR"],
      "display_locales": ["enUS", "esMX", "ptBR", "frFR", "deDE", "esES"],
      "shared_container_default_subfolder": "_retail_",
      "enable_block_copy_patch": true
    }
  },
  "platform": {
    "win": {
      "config": {
        "binaries": {
          "game": {
            "relative_path": "WoW.exe",
            "relative_path_arm64": "Wow-ARM64.exe"
          }
        },
        "min_spec": {
          "default_required_cpu_speed": 2600,
          "default_required_ram": 2048,
          "default_requires_64_bit": true
        }
      }
    },
    "mac": {
      "config": {
        "binaries": {
          "game": {
            "relative_path": "World of Warcraft.app"
          }
        },
        "min_spec": {
          "default_required_cpu_speed": 2200,
          "default_required_ram": 2048
        }
      }
    }
  },
  "enus": {
    "config": {
      "install": [{
        "start_menu_shortcut": {
          "link": "%commonstartmenu%World of Warcraft/World of Warcraft.lnk",
          "target": "%shortcutpath%",
          "description": "Click here to play World of Warcraft."
        }
      }]
    }
  }
  // ... additional locales ...
}
```

### Global Configuration Keys

| Key | Description | Type |
|-----|-------------|------|
| `product` | Product identifier | String |
| `update_method` | Update protocol | "ngdp" |
| `data_dir` | Data directory path | String |
| `supported_locales` | Available languages | Array |
| `display_locales` | UI languages | Array |
| `launch_arguments` | Default launch args | Array |
| `supports_multibox` | Multiple instances | Boolean |
| `supports_offline` | Offline play | Boolean |
| `enable_block_copy_patch` | Block-level patching | Boolean |
| `shared_container_default_subfolder` | Shared data path | String |

### Platform Configuration

```json
{
  "platform": {
    "win": {
      "config": {
        "binaries": {
          "game": {
            "relative_path": "WoWClassic.exe",
            "relative_path_arm64": "WowClassic-arm64.exe",
            "launch_arguments": []
          }
        },
        "min_spec": {
          "default_required_cpu_cores": 1,
          "default_required_cpu_speed": 2600,
          "default_required_ram": 2048,
          "default_requires_64_bit": true,
          "required_osspecs": {
            "6.1": { "required_subversion": 0 }
          }
        },
        "form": {
          "game_dir": {
            "default": "Program Files",
            "required_space": 11500000000,
            "space_per_extra_language": 2000000000
          }
        }
      }
    }
  }
}
```

### Locale Configuration

```json
{
  "enus": {
    "config": {
      "install": [{
        "desktop_shortcut": {
          "link": "%desktoppreference%World of Warcraft Classic.lnk",
          "target": "%shortcutpath%",
          "description": "Click here to play World of Warcraft.",
          "args": "--productcode=wow_classic_era"
        }
      }]
    }
  }
}
```

### Installation Variables

Product configs use variables resolved by Battle.net:

| Variable | Description |
|----------|-------------|
| `%installpath%` | Game installation directory |
| `%binarypath%` | Executable path |
| `%shortcutpath%` | Launcher path |
| `%desktoppreference%` | User desktop path |
| `%commonstartmenu%` | Start menu path |
| `%titlepath%` | Product root directory |
| `%game%` | Game data directory |
| `%locale%` | Current locale |
| `%uid%` | Unique installation ID |

## Parser Implementation Status

### Python Parser (cascette-py)

**Status**: Complete

**Capabilities**:

- Fetches patch configs from build config references

- Parses patch entry format with compression info

- Analyzes entry types (system files, VFS entries)

- Supports both patch and product config examination

- Handles standard CDN path structure

**Verified Against**:

- WoW Classic 1.13.7.38631 patch config

- WoW Classic 4.4.2.60142 patch config (205 entries)

- WoW Classic 5.5.0.62655 patch config

**Known Issues**:

- None identified - both product and patch configs successfully fetched

- Requires fetching build config first to get patch-config hash

See <https://github.com/wowemulation-dev/cascette-py> for the Python
implementation.

## Product Configuration Status Summary

ProductConfig contains product-specific metadata and installation parameters.
These are referenced in Ribbit responses and are accessible via CDN.

**Status**: Available via CDN using `/tpr/configs/data/` path structure
**Format**: JSON
**Purpose**: Product metadata, platform settings, feature flags

### Known Fields (from Ribbit)

- Product configuration hash (16 bytes hex)

- Associated with specific product versions

- May be embedded in client or launcher

## Configuration Discovery Flow

1. **Ribbit Query**: Get version and CDN information
2. **Version Lookup**: Find build configuration hash
3. **Build Config**: Fetch build metadata and system files
4. **CDN Config**: Get archive lists and CDN servers
5. **Patch Config**: Retrieve update paths (rarely available)
6. **Product Config**: Client installation metadata (may not be accessible)

## Implementation Considerations

### Parsing

- Build/CDN/Patch configs: Simple key-value parser

- Product config: JSON parser

- Handle comments (lines starting with `#`)

- Support multi-value fields (comma or space separated)

### Caching

- Configuration files are immutable (content-addressed)

- Cache indefinitely once fetched

- Validate using content hash

### Error Handling

- Retry failed fetches with exponential backoff

- Fall back to alternate CDN servers

- Validate configuration completeness

### Security

- Verify content hashes match expected values

- Use HTTPS when available

- Validate file sizes before download

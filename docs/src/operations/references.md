# Reference Implementations

This document lists NGDP/CASC implementations useful for understanding the
system. These projects have informed cascette-rs development and serve as
references for format details and edge cases.

## C++ Implementations

### ladislav-zezula/CascLib

The original C++ CASC library by the author of StormLib (MPQ library).

- **Repository**: <https://github.com/ladislav-zezula/CascLib>
- **Use for**: Binary format details, algorithm verification, edge cases
- **Features**: Complete CASC support, local and online archives, multiple games

### heksesang/CascLib

C++17 header-only library from the WoW 6.0 era.

- **Repository**: <https://github.com/heksesang/CascLib>
- **Use for**: Simplified CASC reading, header-only integration
- **Note**: Early implementation, lacks modern features (LZMA, LZ4, Zstd, encryption)

## C# Implementations

### Marlamin/CascLib

C# fork with WoW-specific enhancements, used by wow.tools.

- **Repository**: <https://github.com/Marlamin/CascLib>
- **Use for**: Encryption keys, root handlers, CDN index parsing, BLTE decoding
- **Features**: Game-specific root handlers for 20+ Blizzard titles

### wowdev/TACTSharp

Memory-mapped C# implementation focused on performance.

- **Repository**: <https://github.com/wowdev/TACTSharp>
- **Use for**: Performance patterns, zero-copy techniques, CDN optimization
- **Features**: Efficient handling of large encoding files

### wowdev/TACT.Net

C# library for TACT extraction operations.

- **Repository**: <https://github.com/wowdev/TACT.Net>
- **Use for**: Extraction patterns, multiple input/output formats
- **Features**: EKey, CKey, FileDataID, and filename-based extraction

### WowDevTools/CASCHost

Server-side CASC hosting for modding.

- **Repository**: <https://github.com/WowDevTools/CASCHost>
- **Use for**: CASC building, CDN structure generation, content serving
- **Note**: Server-focused (produces content), opposite of cascette-rs (consumes content)

### danielsreichenbach/BuildBackup

C# CDN backup tool (maintained fork of TACTAdder).

- **Repository**: <https://github.com/danielsreichenbach/BuildBackup>
- **Use for**: Mirror command reference, CDN failover, parallel downloads
- **Features**: Archive size caching, resume support, multi-product mirroring

## Rust Implementations

### ferronn-dev/rustycasc

Rust CASC types and FrameXML extractor.

- **Repository**: <https://github.com/ferronn-dev/rustycasc>
- **Use for**: Rust type definitions, archive index parsing
- **Note**: Hardcodes 4-byte offsets (doesn't handle archive-groups)

### ohchase/blizztools

Rust CLI for NGDP/TACT operations.

- **Repository**: <https://github.com/ohchase/blizztools>
- **Use for**: Ribbit protocol, install manifest parsing, async download patterns
- **Features**: Version queries, manifest parsing, file downloads

## Other Tools

### Warpten/tactmon

C++ CDN tracker with Ribbit monitoring.

- **Repository**: <https://github.com/Warpten/tactmon>
- **Use for**: Ribbit protocol implementation, CDN monitoring, product tracking
- **Features**: Template-based ORM, database persistence, production monitoring

### funjoker/blizzget

Windows GUI CDN downloader.

- **Repository**: <https://github.com/nickscha/blizzget>
- **Use for**: Download workflow, custom version configs, tag selection
- **Note**: GUI-focused, Windows-only

### Kruithne/wow.export

Node.js/TypeScript export toolkit.

- **Repository**: <https://github.com/Kruithne/wow.export>
- **Use for**: File extraction patterns, M2/WMO handling, BLP conversion
- **Features**: Visual export interface, multiple format support

### Marlamin/wow.tools.local

Local wow.tools implementation.

- **Repository**: <https://github.com/Marlamin/wow.tools.local>
- **Use for**: File history tracking, DB2 diffing, hotfix management
- **Features**: Web-based content browser, model viewer, database browser

## Community Resources

### wowdev.wiki

Community wiki documenting WoW file formats and systems.

- **URL**: <https://wowdev.wiki>
- **Key pages**: [NGDP](https://wowdev.wiki/NGDP), [CASC](https://wowdev.wiki/CASC),
  [TACT](https://wowdev.wiki/TACT)

### wago.tools

Build database with 1,900+ WoW builds.

- **URL**: <https://wago.tools/builds>
- **Use for**: Build history, version information, product tracking (WoW products)

### BlizzTrack

Real-time build tracker covering all Blizzard TACT products with a public REST API.

- **URL**: <https://blizztrack.com>
- **API base**: `https://blizztrack.com/api/`
- **API docs (Swagger)**: <https://blizztrack.com/swagger/doc.json>
- **Use for**: Build history for any TACT product (including `agent`, `bna`, WoW, Overwatch, etc.),
  archived Ribbit manifest snapshots indexed by sequence number

#### API Endpoints

All responses wrap the payload in `{"success": true, "result": {...}}`.

**Get current versions manifest for a product:**

```bash
curl https://blizztrack.com/api/manifest/agent/versions
```

Response fields per region entry: `region`, `name`, `version_name`, `build_id`,
`build_config`, `cdn_config`, `product_config`.

**Get a historical versions manifest by sequence number:**

```bash
curl "https://blizztrack.com/api/manifest/agent/versions?seqn=3524803"
```

**List archived sequence numbers (paginated, max 25 per page):**

```bash
curl "https://blizztrack.com/api/manifest/agent/seqn?file=versions&limit=5"
```

Response fields: `seqn`, `created_at`, `tact`, `type`, `refs.versions` (relative URL to
retrieve that snapshot).

**Get CDN configuration:**

```bash
curl https://blizztrack.com/api/manifest/agent/cdns
```

Response fields per region entry: `region`, `name`, `path`, `hosts`, `servers`, `config_path`.

**Example: `agent` product (Blizzard Agent / Battle.net Agent)**

```json
{
  "seqn": 3568387,
  "tact": "agent",
  "type": "versions",
  "created_at": "2026-02-23T16:03:11.330006Z",
  "name": "Battle.net Agent",
  "data": [
    {
      "name": "Americas",
      "region": "us",
      "version_name": "2.39.4.9390",
      "build_id": 9390,
      "build_config": "4e4525fb80e2c0e58ca0e6d6415a4687",
      "cdn_config": "749c65c34e2990780795343aea69c255",
      "product_config": "e4e39428296f4009228ac3ecc91909fb"
    }
  ]
}
```

The `agent` product uses CDN path `tpr/bnt001` (not `tpr/wow`). Always retrieve
the path from `/api/manifest/{tact}/cdns` rather than assuming it.

**Get global summary (all products, latest changes):**

```bash
curl "https://blizztrack.com/api/summary"
```

Returns a paginated list of recent manifest changes across all tracked products,
each entry containing `product`, `seqn`, `flags`, `name`, and `refs` for
navigating to the full manifest snapshot.

## Community CDN Mirrors

Community-operated mirrors preserving historical WoW builds. These provide
access to game data after Blizzard removes it from official CDNs.

### cdn.arctium.tools

- **URL**: <https://cdn.arctium.tools>
- **Coverage**: WoW 6.x onwards (2014+)
- **Products**: World of Warcraft (all variants)

### casc.wago.tools

- **URL**: <https://casc.wago.tools>
- **Coverage**: Recent WoW builds
- **Products**: World of Warcraft

### archive.wow.tools

- **URL**: <https://archive.wow.tools>
- **Coverage**: Various WoW builds
- **Products**: World of Warcraft, historical data

cascette-rs supports automatic fallback between these mirrors when official
Blizzard CDNs are unavailable.

# Supported Products

cascette-rs supports the following Blizzard TACT product codes. All products
use the same NGDP/CASC infrastructure for content distribution.

## Product Families

Blizzard groups product codes into **product families**. Products within a
family share a launcher binary, CDN path, and install root directory. Each
family is identified by a **product tag** — a string used by the `bts`
bootstrapper to select the correct launcher build.

The product family concept appears in Blizzard's own telemetry protobuf
(`GameDetails.product_family`) and in the `bts/versions` BPSV where the
"Region" column contains product tags rather than geographic regions.

### World of Warcraft Family

Product tag: `wow`\
CDN path: `tpr/wow`\
Launcher binary: `World of Warcraft Launcher.exe`

All WoW products share the same CDN path and launcher binary. Multiple
products can coexist under the same install root, each in its own game
subfolder (e.g., `_classic_/`, `_classic_era_/`). The launcher detects
which subfolder the user selects and launches the corresponding executable.

| Product Code | Description | Subfolder | Notes |
|-------------|-------------|-----------|-------|
| `wow` | World of Warcraft (Retail) | `_retail_` | Live retail build |
| `wow_classic` | Classic (Progressing) | `_classic_` | Re-releases WoW expansions in sequence |
| `wow_classic_era` | Classic Era | `_classic_era_` | The original World of Warcraft from 2004 |
| `wow_classic_titan` | Classic Titan | - | CN region only, WotLK 3.80.x with upgraded Classic/TBC content for level 80 |
| `wow_anniversary` | 20th Anniversary Edition | - | Progresses through all Classic releases at a faster pace |
| `wowz` | Internal/Development | - | Not publicly accessible |

### Battle.net Platform Products

These products manage the Battle.net launcher, background services, and
game catalog metadata. They are not games.

| Product Code | Description | CDN Path | Config Path | Notes |
|-------------|-------------|----------|-------------|-------|
| `agent` | Background service for game management | `tpr/bnt001` | `tpr/configs/data` | Manages downloads, installations, and updates |
| `bna` | Battle.net desktop app | `tpr/bnt002` | `tpr/configs/data` | The launcher UI application |
| `bts` | Bootstrapper (shared launcher setup) | `tpr/bnt004` | `tpr/configs/data` | Distributes launcher binaries for all product families |
| `catalogs` | Game catalog | `tpr/catalogs` | `tpr/configs/data` | Metadata-only; the catalog of games published on Battle.net |

## Bootstrapper (bts) and Launcher Resolution

The `bts` product is central to how Blizzard distributes launcher binaries.
Understanding it is necessary for implementing game installations.

### How it works

When a game is installed, the Blizzard Agent creates multiple concurrent TACT
operations for game content and launcher binaries. All operations share the
same CASC container (`install_path/Data/`) for encoded storage but extract
loose files to different directories. Each operation has its own build config,
CDN config, and install manifest.

See [Launcher Install](operations/launcher-install.md) for the full
three-operation architecture.

### Resolution flow

```text
Game product (e.g., wow_classic_era)
  │
  ├─ Fetch game product config JSON from CDN
  │   └─ Extract launcher_install_info.product_tag → "wow"
  │
  ├─ Query bts/versions, filter Region == "wow"
  │   └─ Get bootstrapper BuildConfig + CDNConfig
  │
  ├─ Query bts/versions, filter Region == "launcher"
  │   └─ Get launcher binary BuildConfig + CDNConfig
  │
  ├─ Query bts/cdns, filter Name == geographic region (e.g., "us")
  │   └─ Get CDN endpoints with path "tpr/bnt004"
  │
  └─ Run three concurrent TACT installs:
      ├─ Game content → install_path/_classic_era_/
      ├─ Bootstrapper → install_path/
      └─ Launcher binary → install_path/
```

### Product config JSON

The game product's configuration (fetched from
`{ConfigPath}/{hash[0:2]}/{hash[2:4]}/{hash}`) contains the launcher
resolution fields:

```json
{
  "all": {
    "config": {
      "launcher_install_info": {
        "bootstrapper_product": "bts",
        "bootstrapper_branch": "launcher",
        "product_tag": "wow"
      },
      "shared_container_default_subfolder": "_classic_era_"
    }
  }
}
```

- `product_tag` determines which row to select from `bts/versions`
- `shared_container_default_subfolder` sets the game content subfolder
- Builds before approximately version 1.14.0 lack a `ProductConfig` column in
  the versions BPSV; cascette-rs uses `fallback_product_tag()` for these

### bts/versions BPSV semantics

The `bts/versions` response uses its "Region" column for **product tags**,
not geographic regions. Each row maps a product tag to a launcher build:

```text
Region|BuildConfig|CDNConfig|VersionsName|BuildId
wow|1b78...|4136...|1.18.10.3141|3141
d3|1b78...|4136...|1.18.10.3141|3141
launcher|a2c1...|9f3e...|1.18.10.3140|3140
```

The `bts/cdns` response uses standard geographic regions (`us`, `eu`, `kr`,
etc.) with CDN path `tpr/bnt004`.

### Product tag mappings

All product codes within a family resolve to the same product tag, meaning
they share the same launcher binary.

| Product Tag | Product Codes | Family |
|-------------|--------------|--------|
| `wow` | `wow`, `wow_classic`, `wow_classic_era`, `wow_classic_titan`, `wow_anniversary` | World of Warcraft |
| `d3` | `d3` | Diablo III |
| `ow` | `ow` | Overwatch |
| `hero` | `hero` | Heroes of the Storm |
| `s1` | `s1` | StarCraft Remastered |
| `s2` | `s2` | StarCraft II |
| `w3` | `w3` | Warcraft III |
| `bna` | `bna` | Battle.net App |
| `launcher` | `launcher` | Battle.net Desktop App |

The `catalogs` product is metadata-only and has no product tag mapping.

## Process Detection

The agent detects running game processes to prevent operations (like uninstall)
while a game is active. Each WoW product maps to known executable names across
platforms.

| Product Code | Windows | macOS | Linux |
|-------------|---------|-------|-------|
| `wow` | `Wow.exe`, `WowB.exe`, `WowT.exe`, `Wow-64.exe` | `WoW` | `WoW-x86_64` |
| `wow_classic` | `WowClassic.exe`, `WowClassicB.exe`, `WowClassicT.exe` | `WoWClassic` | `WoWClassic-x86_64` |
| `wow_classic_era` | `WowClassic.exe`, `WowClassicB.exe` | `WoWClassic` | `WoWClassic-x86_64` |
| `wow_classic_titan` | `WowClassic.exe` | `WoWClassic` | `WoWClassic-x86_64` |
| `wow_anniversary` | `WowClassic.exe` | `WoWClassic` | `WoWClassic-x86_64` |

Battle.net platform products (`agent`, `bna`, `bts`, `catalogs`) do not have
process detection entries.

## Shared Install Directories

Multiple products from the same family can coexist under one install root.
Each product occupies its own game subfolder. A `.flavor.info` file in each
subfolder identifies the product variant.

```text
C:\Games\World of Warcraft\
├── World of Warcraft Launcher.exe    ← from bts (shared)
├── Data/                             ← shared CASC container
│   ├── data/
│   └── indices/
├── _retail_/                         ← wow
│   ├── .flavor.info
│   └── Wow.exe
├── _classic_/                        ← wow_classic
│   ├── .flavor.info
│   └── WowClassic.exe
└── _classic_era_/                    ← wow_classic_era
    ├── .flavor.info
    └── WowClassic.exe
```

The subfolder name comes from (in priority order):
1. The `/register` API request (from the Battle.net app)
2. The `.product.db` protobuf file
3. The `shared_container_default_subfolder` field in product config JSON

## CDN Path Notes

- All WoW products share `tpr/wow` despite having different product codes.
  The CDN path comes from the `Path` field in CDN BPSV responses.
- The `agent` product uses CDN path `tpr/bnt001`.
- The `bna` product uses CDN path `tpr/bnt002`.
- The `bts` product uses CDN path `tpr/bnt004`.
- Products with a `ConfigPath` (`bts`, `catalogs`) fetch product configuration
  JSON from `{ConfigPath}/{hash[0:2]}/{hash[2:4]}/{hash}`.
- No public CDN archives exist for `agent` or `bts`. Only current versions are
  available from Blizzard's CDN.

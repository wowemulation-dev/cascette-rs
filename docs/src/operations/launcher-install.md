# Bootstrapper and Launcher Binary Installation

## Background

Blizzard Agent creates **three** concurrent TACT update operations per
game install:

1. **Game content** — downloads game data to `install_path/subfolder/`
   (e.g., `_classic_era_/`)
2. **Bootstrapper** — downloads the setup/provisioner binary from the
   `bts` product (Region == `product_tag`, e.g., `"wow"`) to
   `install_path/` (no subfolder)
3. **Launcher binary** — downloads the launcher binary from the `bts`
   product (Region == `bootstrapper_branch`, e.g., `"launcher"`) to
   `install_path/` (no subfolder)

The `bts` product provides both the setup binary and the launcher binary
as separate builds, addressed by different Region values in `bts/versions`.

The Blizzard Agent zeroes the subfolder field for the launcher binary
operation, causing the launcher to be extracted to the install root.
Our implementation achieves the same result by setting
`game_subfolder: None` on the `InstallConfig`.

## Product Config

The game product's versions BPSV includes a `ProductConfig` column
containing a hex hash. This hash points to a JSON file on CDN at:

```
{ConfigPath}/{hash[0:2]}/{hash[2:4]}/{hash}
```

Where `ConfigPath` comes from the CDNs BPSV (typically `tpr/configs/data`).

The JSON contains `launcher_install_info`:

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

## bts Product Layout

The `bts` product has non-standard BPSV semantics:

- **`bts/versions`**: The "Region" column contains product codes (not
  geographic regions). Two Region values matter:
  - `"wow"` — setup/provisioner binary (build 3141_wow)
  - `"launcher"` — shared launcher binary (build 3140_launcher_merged)
- **`bts/cdns`**: Uses standard geographic regions (`us`, `eu`, etc.)
  with CDN path `tpr/bnt004`.

The bts install manifest for the `wow` product tag contains only:
- `World-of-Warcraft-Setup.exe` (Windows)
- macOS setup app bundle (OSX)

Only 2 tags exist: `OSX` and `Windows`. No `"launcher"` tag.

The `"launcher"` Region row is shared across all product families. All
WoW variants, Diablo, Overwatch, etc. use the same launcher binary build.

To resolve both bts operations for `wow_classic_era`:

1. Read `product_tag` and `bootstrapper_branch` from the product config
   (`"wow"` and `"launcher"` respectively)
2. **Bootstrapper**: Query `bts/versions`, filter by Region == `"wow"`
3. **Launcher binary**: Query `bts/versions`, filter by Region == `"launcher"`
4. Query `bts/cdns`, filter by "Name" == user's geographic region
5. Run both as separate concurrent installs to the install root

## Implementation

### Product Config Infrastructure

Module `cascette-protocol/src/product_config.rs`:

- `ProductConfig` struct with serde deserialization
- `LauncherInstallInfo` sub-struct
- `fetch_product_config()` uses `HttpClient` directly (product config URLs use
  `{ConfigPath}/{hash[0:2]}/{hash[2:4]}/{hash}` — no content type segment)
- `config_path_from_bpsv_row()` extracts `ConfigPath` from CDNs BPSV rows
- `fallback_product_tag()` provides hardcoded mappings for older builds
- `product_config_hash` and `config_path` fields added to `ProductMetadata`

### Bootstrapper Resolution

Function `resolve_bootstrapper_metadata` in `executor/helpers.rs`:

- Separate from `resolve_product_metadata` because `bts` has different
  semantics (product tag lookup in versions, geographic region in CDNs)
- Takes `product_tag` and user `region` as separate parameters
- Returns `ProductMetadata` for the `bts` build

### Concurrent bts Installs

Both `executor/install.rs` and `executor/backfill.rs`:

- After resolving game metadata, call `resolve_bts_configs()` which
  resolves both bootstrapper and launcher binary metadata
- `resolve_bts_configs()` returns `(Option<InstallConfig>, Option<InstallConfig>)`
  for the bootstrapper and launcher binary respectively
- Both bts operations use `build_launcher_install_config()`:
  - `game_subfolder: None` (extract to install root)
  - Tag query: platform + architecture + `"launcher"` tag (via `extra_tags`)
  - Same base install path
- Both are spawned as concurrent tasks (`tokio::spawn`)
- Both failures are non-fatal (logged as warnings)
- Main executor waits for all three tasks before reporting results
- Cancellation aborts all three tasks

The `"launcher"` extra tag is silently ignored by the bts install
manifest (which has no such tag). The effective filter is platform-only.
Unknown tags are ignored during tag filtering.

### Fallback for Older Builds

Builds without a `ProductConfig` column (pre-1.14):

- `fallback_product_tag()` maps game product codes to bts product tags
- Known mappings: `wow_classic_era` -> `"wow"`,
  `wow_classic` -> `"wow"`, `wow` -> `"wow"` (see table below)
- `DEFAULT_BOOTSTRAPPER_BRANCH` (`"launcher"`) is used for the launcher
  binary operation when no product config is available
- Queries `bts/versions` with the fallback product tag and default branch
- Logs that fallback is being used

### Agent State

- Schema v7: `product_config_hash` column added to products table
- `Product` model stores the hash for use in subsequent operations
- Migration `v6_to_v7` handles existing databases

### Files Modified

| File | Change |
|------|--------|
| `cascette-protocol/src/product_config.rs` | ProductConfig struct, JSON parser, fetch, fallback table, `DEFAULT_BOOTSTRAPPER_BRANCH` constant |
| `cascette-protocol/src/lib.rs` | Re-export `product_config` module |
| `cascette-protocol/src/cdn/mod.rs` | `CdnClient::http_client()` accessor |
| `cascette-agent/src/executor/helpers.rs` | `resolve_bootstrapper_metadata`, `build_launcher_install_config`, `ProductMetadata` fields |
| `cascette-agent/src/executor/install.rs` | `resolve_bts_configs` (replaces `resolve_launcher_config`), concurrent bootstrapper + launcher binary tasks |
| `cascette-agent/src/executor/backfill.rs` | `resolve_bts_configs` (replaces `resolve_launcher_config`), concurrent bootstrapper + launcher binary tasks |
| `cascette-installation/src/config.rs` | `TagQuery::extra_tags` field, included in `tag_names()` |
| `cascette-agent/src/state/db.rs` | Schema v7 migration |
| `cascette-agent/src/state/registry.rs` | Read/write `product_config_hash` |
| `cascette-agent/src/models/product.rs` | `product_config_hash` field |

### Testing

Implemented:

- Unit tests for `ProductConfig` JSON parsing (URL building, deserialization,
  missing fields) in `cascette-protocol/src/product_config.rs`
- Unit tests for `fallback_product_tag()` mappings

Not yet implemented:

- Wiremock tests for `bts/versions` and `bts/cdns` product tag filtering
- End-to-end install with bootstrapper + launcher binary (requires `tpr/bnt004` CDN data)

### CDN Mirror Requirements

The `bts` product uses CDN path `tpr/bnt004`. The local USB mirror
needs this path mirrored alongside `tpr/wow`. Only the current version
is available (no public archives for `bts`).

### Known Product Tag Mappings

For the full product list, see [Supported Products](../products.md).

| Game Product | Product Tag | Notes |
|-------------|-------------|-------|
| `wow` | `wow` | WoW Retail |
| `wowt` | `wowt` | WoW PTR |
| `wowb` | `wowb` | WoW Beta |
| `wow_classic` | `wow` | Classic Progressing (shares wow tag) |
| `wow_classic_era` | `wow` | Classic Era (shares wow tag) |
| `wow_classic_titan` | `wow` | Classic Titan CN (shares wow tag) |
| `wow_anniversary` | `wow` | 20th Anniversary Edition (shares wow tag) |
| `d3` | `d3` | Diablo III |
| `ow` | `ow` | Overwatch |
| `hero` | `hero` | Heroes of the Storm |
| `s1` | `s1` | StarCraft Remastered |
| `s2` | `s2` | StarCraft II |
| `w3` | `w3` | Warcraft III |
| `bna` | `bna` | Battle.net App |
| `launcher` | `launcher` | Battle.net Desktop App itself |

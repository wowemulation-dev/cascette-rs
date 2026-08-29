# cascette-import Examples

## blizztrack_verification

Fetches per-region build information from the live BlizzTrack API for `agent` and `wow_classic_era` products. Verifies that builds are returned with valid version strings and build numbers.

```sh
cargo run -p cascette-import --example blizztrack_verification --features blizztrack
```

Requires network access to `blizztrack.com`.

## import_community_data

Demonstrates fetching and importing data from all three community sources: wago.tools (build history), WoWDev listfile (FileDataID-to-path mappings), and WoWDev TACT keys (encryption keys). Exercises build searching with criteria, FileDataID resolution, and cache statistics.

```sh
cargo run -p cascette-import --example import_community_data
```

Requires network access to `wago.tools` and `github.com`. Downloads the community listfile (~200 MB).

## listfile_name_lookup

Demonstrates Jenkins96 name hashing for CASC root file lookups (equivalent to BuildBackup `calchashlistfile` and CascLib `CascFindFile`). Shows path normalization, FDID-to-hash relationships, and `ListfileProvider` usage. Optionally accepts file paths as CLI arguments to hash.

```sh
cargo run -p cascette-import --example listfile_name_lookup --features listfile
```

```sh
cargo run -p cascette-import --example listfile_name_lookup --features listfile -- \
  "Interface\\Icons\\INV_Misc_QuestionMark.blp"
```

No network access required for hash demonstration. `ListfileProvider` initialization shown but not exercised.

## wago_verification

Fetches WoW Classic build data from the live wago.tools API and searches for specific version patterns (1.13.*). Verifies that known builds like 1.13.2.31650 are present in the results.

```sh
cargo run -p cascette-import --example wago_verification
```

Requires network access to `wago.tools`.

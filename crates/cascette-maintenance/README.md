# cascette-maintenance

Maintenance operations for local CASC installations.

Provides four coordinated operations matching agent.exe's maintenance subsystem:

- **Preservation set** - Collects encoding keys referenced by the current build
- **Garbage collection** - Removes content not in the preservation set
- **Compaction** - Defragments segments and consolidates partially-full archives
- **Build repair** - Validates content integrity and rebuilds damaged indices

All operations support dry-run mode via `ExecutionMode::DryRun`.

## Example

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run --example maintenance_analysis \
    -p cascette-maintenance --features local-install
```

Runs all four maintenance operations in dry-run mode against a local WoW
installation and prints a report. No files are modified.

The `CASCETTE_WOW_PATH` environment variable should point to the WoW
installation root (the directory containing `Data/`).

### Sample output

```text
=== CASC Maintenance Analysis ===
Installation: /path/to/wow/Data

Index entries: 130349
Archive files: 4
Archive size:  4013625379 bytes

--- Preservation Set ---
  Keys preserved: 130349
  Source entries:  130349
  Duration:       144.450ms

--- Garbage Collection (dry-run) ---
  Entries scanned:   130349
  Would remove:      0
  Orphaned segments: 0
  Bytes freeable:    0

--- Compaction (dry-run) ---
  Segments analyzed: 4
  Would compact:     0
  Moves planned:     0
  Bytes reclaimable: 0

--- Build Repair (dry-run) ---
  Entries verified: 130349
  Valid:            130326
  Corrupted:        23
  Duration:         146.741s

Total duration: 147.093s
```

## Features

- `local-install` - Enables examples that require a local WoW installation

## Testing strategy

1. Run dry-run mode against real WoW Classic clients to verify analysis
2. Once dry-run output is correct, run execute mode against a **copy** of a client
3. Upgraded clients (not fresh installs) accumulate orphaned content and are ideal test subjects

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

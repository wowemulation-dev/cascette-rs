# cascette-maintenance Examples

Both examples require a local WoW installation and the `local-install` feature flag.
Set `CASCETTE_WOW_PATH` to the installation root directory.

## maintenance_analysis

Runs all four maintenance operations (preservation, garbage collection, compaction, repair)
in dry-run mode and prints a report of what each would do.

```sh
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-maintenance \
  --example maintenance_analysis --features local-install
```

**Prerequisites:** Requires a local WoW installation with a `Data/` directory.

## maintenance_verification

Runs a dry-run maintenance pass and prints a summary of preservation set size,
orphaned segments, and corruption status.

```sh
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-maintenance \
  --example maintenance_verification --features local-install
```

**Prerequisites:** Requires a local WoW installation with a `Data/` directory.

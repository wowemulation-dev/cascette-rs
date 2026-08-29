# cascette-metadata Examples

## metadata_orchestrator

Initializes the community listfile and TACT key providers, builds a `MetadataOrchestrator`,
and demonstrates FileDataID resolution, reverse path lookup, and health reporting.

```sh
cargo run -p cascette-metadata --example metadata_orchestrator
```

**Prerequisites:** Requires network access. Downloads the community listfile (~200 MB CSV)
and TACT keys from GitHub into a temporary directory.

# cascette-metadata

File metadata, content categorization, and TACT key orchestration for NGDP/CASC.

Provides:

- **FileDataID service** - Bidirectional ID-to-path resolution with case-insensitive lookup
- **Content categorization** - Classifies files into categories (Executable,
  Audio, Graphics, Interface, Data, WorldData, Unknown)
- **Metadata orchestrator** - Coordinates FDID resolution and TACT key access with health reporting

## Example

```bash
cargo run --example metadata_orchestrator -p cascette-metadata
```

Initializes import providers, builds the orchestrator, resolves FileDataIDs,
and displays statistics. Requires network access.

## Features

- `import` (default) - Integration with `cascette-import` providers

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

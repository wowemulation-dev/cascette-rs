# cascette-import

Community data source integration for NGDP/CASC.

Provides integration with three community-maintained data sources:

- **wago.tools** - Historical build information via the `/api/builds` endpoint
- **WoWDev Listfile** - File ID to path mappings from `github.com/wowdev/wow-listfile`
- **WoWDev TACT Keys** - Encryption keys from `github.com/wowdev/TACTKeys`

Each source is implemented as a provider behind the `ImportProvider` trait. The
`ImportManager` coordinates multiple providers, aggregating results and managing
per-provider health and caching.

## Example

```bash
cargo run --example import_community_data -p cascette-import
```

Fetches builds from wago.tools, resolves FileDataIDs via the listfile, and
loads TACT keys. Requires network access.

## Features

- `wago` (default) - wago.tools build information provider
- `listfile` (default) - WoWDev listfile provider
- `tactkeys` (default) - WoWDev TACT keys provider

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

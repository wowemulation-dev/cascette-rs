# cascette-protocol Examples

## build_file_tree

Walks a build's CDN content graph and reports file presence, grouped by CDN namespace
(config, data, patch). Reads build config, CDN config, encoding file, and all archive
indices. Works with both local CDN mirrors and remote CDN URLs.

Supports two modes:

**Live mode** queries Ribbit for the current build, resolves all hashes automatically, and
uses official CDN hosts:

```sh
cargo run -p cascette-protocol --example build_file_tree -- <product> [region] [--paths]

# Query the current wow_classic_era build
cargo run -p cascette-protocol --example build_file_tree -- wow_classic_era

# Specific region
cargo run -p cascette-protocol --example build_file_tree -- wow_classic_era eu
```

**Manual mode** uses explicit hashes and a CDN source (local mirror or URL):

```sh
cargo run -p cascette-protocol --example build_file_tree -- \
  <product> <build_config_hash> <cdn_config_hash> <source> [cdn_path] [options]

# Local mirror
cargo run -p cascette-protocol --example build_file_tree -- \
  wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb \
  /path/to/cdn/mirror tpr/wow
```

Manual mode options:
- `--paths` -- print one path/URL per tracked file (pipe-friendly, suppresses other output)
- `--product-config <hash>` -- include the product config file (from versions BPSV)
- `--config-path <path>` -- ConfigPath for product config (default: `tpr/configs/data`)

**Prerequisites:** Live mode requires network access to Ribbit and official CDN. Manual mode
requires a local CDN mirror directory or network access to a CDN.

## cdn_archive_sizes

Downloads archive index files for a build and reports the total data size across all
archives. Only downloads the small index files, not the archives themselves. Equivalent
to BuildBackup's `dumpsizes` command.

```sh
cargo run -p cascette-protocol --example cdn_archive_sizes
cargo run -p cascette-protocol --example cdn_archive_sizes -- <product> <region> [max_archives]
```

Defaults to `wow_classic_era`, region `us`, and 10 archives.

**Prerequisites:** Requires network access to Ribbit and community CDN mirrors.

## cdn_chain_verification

Tests the full download-parse-decode chain: downloads build config, encoding table,
install manifest, root manifest, and an archive index using pinned hashes from
WoW Classic 1.13.2.31650. Verifies BLTE decoding and encoding table lookups.

```sh
cargo run -p cascette-protocol --example cdn_chain_verification
```

Optional environment variables:
- `CASCETTE_CDN_HOSTS` -- comma-separated CDN hostnames (default: `casc.wago.tools,cdn.arctium.tools,archive.wow.tools`)
- `CASCETTE_CDN_PATH` -- CDN product path (default: `tpr/wow`)

**Prerequisites:** Requires network access to community CDN mirrors.

## cdn_protocol_verification

Exercises TACT/Ribbit queries and CDN downloads using pinned hashes from
WoW Classic 1.13.2.31650. Tests version queries, CDN config queries, config/data
downloads, and error handling for nonexistent hashes.

```sh
cargo run -p cascette-protocol --example cdn_protocol_verification
```

Optional environment variables:
- `CASCETTE_CDN_HOSTS` -- comma-separated CDN hostnames (default: `casc.wago.tools,cdn.arctium.tools,archive.wow.tools`)
- `CASCETTE_CDN_PATH` -- CDN product path (default: `tpr/wow`)

**Prerequisites:** Requires network access to community CDN mirrors.

## dump_product_info

Queries Ribbit for the live version and CDN data, then downloads and parses the build
config and CDN config to show the full picture of a build. Equivalent to BuildBackup's
`dumpinfo` command.

```sh
cargo run -p cascette-protocol --example dump_product_info
cargo run -p cascette-protocol --example dump_product_info -- <product> <region>
```

Defaults to `wow_classic_era` and region `us`.

**Prerequisites:** Requires network access to Ribbit and a CDN endpoint.

## wow_classic_native

Queries WoW Classic product information and downloads core configuration files using
full native features (Ribbit TCP fallback, connection pooling, HTTP/2, streaming
downloads, disk caching).

```sh
cargo run -p cascette-protocol --example wow_classic_native
```

**Prerequisites:** Requires network access to Ribbit and a CDN endpoint.

## wow_classic_wasm

Same workflow as `wow_classic_native` but written to be WASM-compatible. Uses only
browser-safe APIs (TACT HTTPS, localStorage caching). Can be tested natively or
compiled for `wasm32-unknown-unknown` with `wasm-pack`.

```sh
# Native test
cargo run -p cascette-protocol --example wow_classic_wasm

# WASM build (requires wasm-pack)
wasm-pack build --target web
```

**Prerequisites:** Requires network access to Ribbit and a CDN endpoint.

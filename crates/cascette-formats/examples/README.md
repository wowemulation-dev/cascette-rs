# cascette-formats Examples

## archive_index

Builds a CDN archive index with `ArchiveIndexBuilder`, writes it to a buffer, parses it back with `ArchiveIndex::parse`, performs entry lookups, and demonstrates rebuilding an index from an existing one.

```sh
cargo run -p cascette-formats --example archive_index
```

No prerequisites. Uses generated test data.

## binary_patches

Demonstrates ZBSDIFF1 binary differential patching: creating patches with `ZbsdiffBuilder`, applying them with `apply_patch_memory`, analyzing patch characteristics, streaming patcher usage, and comparing simple vs optimized patch algorithms.

```sh
cargo run -p cascette-formats --example binary_patches
```

No prerequisites. Uses generated test data.

## blte_encoding

Demonstrates BLTE container format operations: single-chunk and multi-chunk files, None/ZLib/LZ4 compression modes, round-trip serialization via `CascFormat` trait, and `BlteBuilder` API usage.

```sh
cargo run -p cascette-formats --example blte_encoding
```

No prerequisites. Uses generated test data.

## encoding_table

Builds an encoding file with CKey-to-EKey and EKey-to-ESpec mappings using `EncodingBuilder`, serializes and parses it back, performs content key lookups, batch lookups, and demonstrates modifying an existing encoding file.

```sh
cargo run -p cascette-formats --example encoding_table
```

No prerequisites. Uses generated test data.

## manifests

Builds and parses Install manifests (with platform/architecture/locale tags and file associations) and Download manifests (V1 and V3, with priority categories, checksums, and 40-bit file sizes).

```sh
cargo run -p cascette-formats --example manifests
```

No prerequisites. Uses generated test data.

## parse_build_config

Parses BuildConfig, CdnConfig, and KeyringConfig from realistic config strings. Demonstrates field extraction, validation, round-trip serialization, and keyring key lookup by hex ID and numeric ID.

```sh
cargo run -p cascette-formats --example parse_build_config
```

No prerequisites. Uses hardcoded config strings.

## root_file

Builds root files with FileDataID mappings using `RootBuilder`, parses them back, resolves files by FileDataID and by path (case-insensitive, slash-normalized), and demonstrates all root file versions (V1-V4) and locale/content flags.

```sh
cargo run -p cascette-formats --example root_file
```

No prerequisites. Uses generated test data.

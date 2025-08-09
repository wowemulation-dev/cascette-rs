# ngdp-bpsv Examples

This directory contains examples demonstrating how to use the `ngdp-bpsv` crate for parsing and building BPSV documents.

## Available Examples

### `parse_basic.rs`

Demonstrates basic BPSV parsing with real Ribbit version data:

- Parse product versions from BPSV format
- Access schema information
- Iterate through data rows
- Handle sequence numbers

```bash
cargo run --example parse_basic
```

### `build_bpsv.rs`

Shows how to build BPSV documents programmatically:

- Create schema with field definitions
- Add data rows with type validation
- Generate BPSV string output
- Round-trip compatibility testing

```bash
cargo run --example build_bpsv
```

### `typed_access.rs`

Demonstrates type-safe value access patterns:

- Access values by column name
- Type conversion and validation
- Error handling for missing fields
- Working with different field types (STRING, HEX, DEC)

```bash
cargo run --example typed_access
```

## Running Examples

To run all examples:

```bash
cargo run --example parse_basic -p ngdp-bpsv
cargo run --example build_bpsv -p ngdp-bpsv
cargo run --example typed_access -p ngdp-bpsv
```

## Example Data

The examples use real BPSV data formats from:

- Ribbit version responses
- CDN configuration data
- Product information

This demonstrates real-world usage patterns and ensures compatibility with actual Blizzard data formats.

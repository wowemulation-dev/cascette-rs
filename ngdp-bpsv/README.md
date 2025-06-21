# ngdp-bpsv

A Rust parser and writer for BPSV (Blizzard Pipe-Separated Values) format, used
throughout Blizzard's NGDP (Next Generation Data Pipeline) system.

## Overview

BPSV is a structured data format used by Blizzard Entertainment across their
content delivery network. It features:

- 📊 Typed columns (STRING, HEX, DEC)
- 🔢 Sequence numbers for version tracking
- 📋 Pipe-separated values with header definitions
- ✅ Built-in validation for data types and constraints

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-bpsv = "0.1"
```

## Quick Start

### Parsing BPSV Data

```rust
use ngdp_bpsv::BpsvDocument;

let data = r#"Region!STRING:0|BuildId!DEC:4|Hash!HEX:32
## seqn = 12345
us|1234|deadbeefcafebabedeadbeefcafebabe
eu|5678|1234567890abcdef1234567890abcdef"#;

let doc = BpsvDocument::parse(data)?;
println!("Sequence: {:?}", doc.sequence_number());
println!("Rows: {}", doc.rows().len());
```

### Building BPSV Data

```rust
use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};

let mut builder = BpsvBuilder::new();
builder.add_field("Region", BpsvFieldType::String(0))?;
builder.add_field("BuildId", BpsvFieldType::Decimal(4))?;
builder.set_sequence_number(12345);

builder.add_row(vec![
    BpsvValue::String("us".to_string()),
    BpsvValue::Decimal(1234),
])?;

let output = builder.build()?;
```

## Format Specification

### Field Types

- **STRING:length** - String field (length 0 = unlimited)
- **HEX:length** - Hexadecimal field (length in chars)
- **DEC:length** - Decimal integer field

### Structure

```text
FieldName!TYPE:length|AnotherField!TYPE:length
## seqn = 12345
value1|value2
value3|value4
```

## Examples

### Parse Ribbit Version Data

```rust
use ngdp_bpsv::BpsvDocument;

let versions_data = std::fs::read_to_string("versions.bpsv")?;
let doc = BpsvDocument::parse(&versions_data)?;

// Find all US region entries
let us_rows = doc.find_rows_by_field("Region", "us")?;
for row_idx in us_rows {
    let row = &doc.rows()[row_idx];
    if let Some(build_id) = row.get_raw_by_name("BuildId", doc.schema()) {
        println!("US Build: {}", build_id);
    }
}
```

### Type-Safe Value Access

```rust
// Access typed values from a row
let row = &doc.rows()[0];
let schema = doc.schema();

// Get raw string value
let region = row.get_raw_by_name("Region", schema).unwrap();

// Get typed value (requires mutable row)
let mut row = doc.rows()[0].clone();
let typed_values = row.get_typed_values(schema)?;
if let BpsvValue::Decimal(build_id) = &typed_values[1] {
    println!("Build ID: {}", build_id);
}
```

### Build CDN Configuration

```rust
use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};

let mut builder = BpsvBuilder::new();
builder.add_field("Name", BpsvFieldType::String(0))?;
builder.add_field("Path", BpsvFieldType::String(0))?;
builder.add_field("Hosts", BpsvFieldType::String(0))?;
builder.set_sequence_number(2241282);

builder.add_row(vec![
    BpsvValue::String("us".to_string()),
    BpsvValue::String("tpr/wow".to_string()),
    BpsvValue::String("us.cdn.blizzard.com level3.blizzard.com".to_string()),
])?;

println!("{}", builder.build()?);
```

## Features

- 🚀 Fast parsing with minimal allocations
- 🔍 Type validation and error reporting
- 🏗️ Builder pattern for document creation
- 📝 Round-trip compatibility (parse → build → parse)
- 🔧 Case-insensitive field type parsing
- 📭 Empty value support for all field types

## Error Handling

The library provides detailed error types for common issues:

```rust
use ngdp_bpsv::{BpsvDocument, Error};

match BpsvDocument::parse(data) {
    Ok(doc) => println!("Parsed {} rows", doc.rows().len()),
    Err(Error::InvalidHeader { line }) => {
        println!("Invalid header: {}", line);
    }
    Err(Error::RowValidation { row_index, reason }) => {
        println!("Row {} invalid: {}", row_index, reason);
    }
    Err(e) => println!("Parse error: {}", e),
}
```

## Performance

The parser is optimized for the typical BPSV use cases:

- Small to medium documents (< 10,000 rows)
- Fast field lookups via schema indexing
- Lazy type parsing (on-demand conversion)

See the benchmarks for detailed performance metrics.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../LICENSE-MIT))

at your option.

## Acknowledgments

This crate is part of the cascette-rs project, providing tools for World of Warcraft
emulation development.

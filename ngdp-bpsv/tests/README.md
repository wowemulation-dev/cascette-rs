# ngdp-bpsv Tests

This directory contains comprehensive tests for the `ngdp-bpsv` crate.

## Test Files

- `integration_test.rs` - Integration tests with real-world BPSV data

## Test Coverage

The tests cover all aspects of BPSV parsing and building:

### Parser Tests (`src/parser.rs`)
- Complete document parsing
- Schema-only parsing  
- Raw row parsing without validation
- Empty document handling
- Invalid document error handling
- Case-insensitive field types
- Sequence number parsing variations
- Statistics extraction

### Builder Tests (`src/builder.rs`)
- Basic document building
- Field addition and validation
- Schema mismatch error handling
- Values row addition
- Raw row addition
- Round-trip parsing (parse → build → parse)
- Creating from existing BPSV data

### Schema Tests (`src/schema.rs`)
- Header parsing with field definitions
- Case-insensitive field type parsing
- Field access methods
- Duplicate field error detection
- Row validation against schema

### Field Type Tests (`src/field_type.rs`)
- All field type parsing (STRING, HEX, DEC)
- Value validation for each type
- Display formatting
- Value normalization

### Value Tests (`src/value.rs`)
- Value parsing and conversion
- Type compatibility checking
- Accessor methods
- BPSV string generation
- Invalid value handling

### Document Tests (`src/document.rs`)
- Document creation and manipulation
- Column access by name and index
- Row operations
- Schema validation
- Find operations

## Integration Tests

The integration tests use real BPSV data from:
- WoW product versions
- CDN configuration responses
- Ribbit summary data
- Complex field combinations

## Running Tests

```bash
# Run all ngdp-bpsv tests
cargo test -p ngdp-bpsv

# Run with output
cargo test -p ngdp-bpsv -- --nocapture

# Run specific test categories
cargo test -p ngdp-bpsv parser
cargo test -p ngdp-bpsv builder  
cargo test -p ngdp-bpsv schema
cargo test -p ngdp-bpsv integration
```

## Benchmarks

The crate includes performance benchmarks in `benches/` for:
- Parsing documents of various sizes
- Building operations
- Column access patterns
- Round-trip operations
- Validation performance
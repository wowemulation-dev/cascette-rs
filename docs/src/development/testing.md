# Testing Guidelines

This page covers testing conventions and practices for cascette-rs.

## Test Organization

### Module Structure

Tests live in the same file as the code they test, using a `#[cfg(test)]` module:

```rust
pub fn parse_header(data: &[u8]) -> Result<Header, ParseError> {
    // Implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header_with_valid_data_returns_header() {
        // Test implementation
    }
}
```

### Nested Modules for Large Files

For files with many tests, use nested modules to group related tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    mod parsing {
        use super::*;

        #[test]
        fn test_parse_entry_from_valid_bytes() { ... }

        #[test]
        fn test_parse_entry_from_truncated_bytes_returns_error() { ... }
    }

    mod building {
        use super::*;

        #[test]
        fn test_builder_with_entries_produces_sorted_output() { ... }
    }

    mod edge_cases {
        use super::*;

        #[test]
        fn test_edge_empty_input_returns_empty_result() { ... }
    }
}
```

## Test Naming Convention

### Pattern

Use this naming pattern for test functions:

```text
test_<subject>_<condition>_<expected_outcome>
```

Components:

| Part | Description | Example |
|------|-------------|---------|
| `subject` | What is being tested | `parser`, `builder`, `entry` |
| `condition` | The scenario or input | `with_valid_data`, `from_empty_input` |
| `expected_outcome` | What should happen | `returns_struct`, `returns_error` |

### Examples

**Parsing tests:**

```rust
// Good - specific and descriptive
fn test_parse_header_with_valid_magic_returns_header() { ... }
fn test_parse_header_with_invalid_magic_returns_error() { ... }
fn test_parse_entry_from_truncated_data_returns_incomplete_error() { ... }

// Bad - too vague
fn test_parse() { ... }
fn test_header() { ... }
fn test_error() { ... }
```

**Building tests:**

```rust
// Good
fn test_builder_with_single_entry_creates_valid_output() { ... }
fn test_builder_with_unsorted_entries_sorts_before_writing() { ... }

// Bad
fn test_builder() { ... }
fn test_build() { ... }
```

**Round-trip tests:**

```rust
// Good - suffix with _round_trip
fn test_index_entry_round_trip_preserves_all_fields() { ... }
fn test_blte_compression_round_trip_matches_original() { ... }

// Bad
fn test_round_trip() { ... }  // Round trip of what?
```

### Category Prefixes

Use consistent prefixes for special test categories:

| Prefix | Use Case | Example |
|--------|----------|---------|
| `test_edge_*` | Edge cases and boundary conditions | `test_edge_empty_input_handled` |
| `test_error_*` | Error path validation | `test_error_invalid_checksum_detected` |
| `*_round_trip` | Serialization/deserialization | `test_config_round_trip` |

Edge case examples:

```rust
fn test_edge_empty_index_builds_successfully() { ... }
fn test_edge_single_entry_is_searchable() { ... }
fn test_edge_max_u32_offset_handled() { ... }
fn test_edge_zero_length_data_returns_empty() { ... }
```

Error handling examples:

```rust
fn test_error_truncated_footer_returns_parse_error() { ... }
fn test_error_invalid_checksum_returns_mismatch() { ... }
fn test_error_unsorted_entries_rejected() { ... }
```

## Test Types

### Unit Tests

Test individual functions in isolation:

```rust
#[test]
fn test_jenkins96_hash_with_known_input_produces_expected_output() {
    let result = Jenkins96::hash(b"test");
    assert_eq!(result.hash32, 0x12345678);  // Known value
}
```

### Integration Tests

Place in `tests/` directory for testing public APIs:

```text
crates/cascette-formats/
├── src/
│   └── lib.rs
└── tests/
    └── archive_integration.rs
```

### Property-Based Tests

Use `proptest` for testing invariants across many inputs:

```rust
#[cfg(test)]
mod proptest_tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn round_trip_preserves_entries(entries in prop::collection::vec(any::<Entry>(), 0..100)) {
            let built = build(&entries);
            let parsed = parse(&built)?;
            prop_assert_eq!(entries, parsed);
        }
    }
}
```

Property test naming (inside `proptest!` macro):

- No `test_` prefix needed (macro adds it)
- Describe the property being verified
- Examples: `round_trip_preserves_entries`, `checksum_detects_corruption`

## Assertions

### Use `pretty_assertions`

Import `pretty_assertions` for better diff output on failures:

```rust
#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    #[test]
    fn test_something() {
        assert_eq!(expected, actual);  // Shows colored diff on failure
    }
}
```

### Common Assertions

| Assertion | Use Case |
|-----------|----------|
| `assert_eq!(expected, actual)` | Value equality |
| `assert_ne!(a, b)` | Values differ |
| `assert!(condition)` | Boolean conditions |
| `assert!(result.is_ok())` | Success check |
| `assert!(result.is_err())` | Error check |
| `matches!(value, pattern)` | Pattern matching |

### Error Assertions

Test specific error types:

```rust
#[test]
fn test_parse_with_invalid_data_returns_checksum_error() {
    let result = parse(invalid_data);

    assert!(matches!(
        result,
        Err(ParseError::ChecksumMismatch { .. })
    ));
}
```

## Running Tests

### Basic Commands

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p cascette-formats

# Run tests matching a pattern
cargo test edge_          # All edge case tests
cargo test error_         # All error tests
cargo test round_trip     # All round-trip tests

# Run a specific test
cargo test test_parse_header_with_valid_data
```

### Feature Combinations

Test with different feature combinations:

```bash
# Default features
cargo test --workspace

# No default features (minimal build)
cargo test --workspace --no-default-features

# All features
cargo test --workspace --all-features
```

### Code Coverage

Generate coverage reports:

```bash
# Generate LCOV report
cargo llvm-cov --workspace --lcov --output-path lcov.info

# Generate HTML report
cargo llvm-cov --workspace --html

# Open HTML report
open target/llvm-cov/html/index.html
```

## Test Data

### Embedded Test Data

For small test cases, embed data directly in tests:

```rust
#[test]
fn test_parse_minimal_header() {
    let data = [
        0x42, 0x4C, 0x54, 0x45,  // Magic: "BLTE"
        0x00, 0x00, 0x00, 0x10,  // Header size: 16
    ];

    let header = parse_header(&data).expect("should parse");
    assert_eq!(header.magic, b"BLTE");
}
```

### Test Fixtures

For larger test files, use the `include_bytes!` macro or test fixtures:

```rust
const TEST_INDEX: &[u8] = include_bytes!("fixtures/sample.index");

#[test]
fn test_parse_real_index_file() {
    let index = ArchiveIndex::parse(TEST_INDEX).expect("should parse");
    assert!(!index.entries.is_empty());
}
```

### Property Test Strategies

Define reusable strategies for property tests:

```rust
fn valid_entry_strategy() -> impl Strategy<Value = IndexEntry> {
    (
        prop::array::uniform16(any::<u8>()),  // 16-byte key
        0u32..u32::MAX,                        // offset
        1u32..1_000_000,                       // size
    ).prop_map(|(key, offset, size)| {
        IndexEntry { key: key.to_vec(), offset, size, archive_index: None }
    })
}
```

## CI Integration

Tests run automatically on every pull request. The CI workflow:

1. Runs `cargo test --workspace` with default features
2. Runs `cargo test --workspace --no-default-features`
3. Tests each changed crate individually on stable Rust
4. Collects code coverage and uploads to Codecov

See `.github/workflows/ci.yml` for the full configuration.

# Coding Standards

This page covers coding conventions and style guidelines for cascette-rs.

## Formatting

All code must be formatted with `rustfmt`. Run before committing:

```bash
cargo fmt --all
```

The workspace uses default rustfmt settings. No custom configuration is needed.

## Linting

The workspace enables strict clippy lints. All warnings must be resolved:

```bash
cargo clippy --workspace --all-targets
```

### Lint Configuration

From `Cargo.toml`:

```toml
[workspace.lints.clippy]
# Lint groups at low priority
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }

# Safety lints at higher priority
unwrap_used = { level = "warn", priority = 2 }
panic = { level = "warn", priority = 2 }
expect_used = { level = "warn", priority = 2 }
```

## Error Handling

### Library Code

Library crates must use proper error handling:

```rust
// Good - returns Result
pub fn parse(data: &[u8]) -> Result<Header, ParseError> {
    if data.len() < HEADER_SIZE {
        return Err(ParseError::InsufficientData {
            expected: HEADER_SIZE,
            actual: data.len(),
        });
    }
    // ...
}

// Bad - panics
pub fn parse(data: &[u8]) -> Header {
    assert!(data.len() >= HEADER_SIZE);  // Don't do this
    // ...
}
```

### Error Types

Use `thiserror` for error definitions:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("insufficient data: expected {expected} bytes, got {actual}")]
    InsufficientData { expected: usize, actual: usize },

    #[error("invalid magic: expected {expected:?}, got {actual:?}")]
    InvalidMagic { expected: [u8; 4], actual: [u8; 4] },

    #[error("checksum mismatch")]
    ChecksumMismatch { expected: [u8; 8], actual: [u8; 8] },
}
```

### Avoiding `unwrap()` and `expect()`

Library code should avoid `unwrap()` and `expect()`. Use these alternatives:

```rust
// Instead of unwrap(), propagate errors
let value = map.get(&key).ok_or(Error::KeyNotFound)?;

// Instead of expect(), use ok_or_else() with context
let value = map.get(&key)
    .ok_or_else(|| Error::KeyNotFound { key: key.clone() })?;

// For truly impossible cases, use unreachable!() with comment
match validated_enum {
    Known::Variant => { /* ... */ }
    // Validation already checked all variants
}
```

When `expect()` is unavoidable (e.g., in `binrw` map functions), add a
file-level allow with documentation:

```rust
//! Module description
//!
//! Uses expect in binrw map functions where Result types cannot be used.
#![allow(clippy::expect_used)]
```

### Test Code

Test code may use `unwrap()` and `expect()` with the allow attribute:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    // Tests can use unwrap/expect/panic freely
}
```

## Binary Format Parsing

### Use binrw

All binary formats use the `binrw` crate for parsing and building:

```rust
use binrw::{BinRead, BinWrite};

#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]  // NGDP uses big-endian
pub struct Header {
    #[brw(magic = b"BLTE")]
    pub magic: (),

    pub header_size: u32,
    pub flags: u8,
}
```

### Big-Endian Default

NGDP/CASC formats use big-endian byte order. Always specify:

```rust
#[derive(BinRead, BinWrite)]
#[brw(big)]  // Required for NGDP formats
pub struct Entry {
    pub offset: u32,
    pub size: u32,
}
```

If a field uses little-endian (rare), annotate explicitly:

```rust
#[derive(BinRead, BinWrite)]
#[brw(big)]
pub struct MixedEntry {
    pub big_endian_field: u32,

    #[brw(little)]
    pub little_endian_field: u32,  // Exception - document why
}
```

### Round-Trip Testing

Every format must have round-trip tests:

```rust
#[test]
fn test_header_round_trip_preserves_all_fields() {
    let original = Header {
        header_size: 16,
        flags: 0x01,
    };

    let mut buffer = Vec::new();
    original.write(&mut Cursor::new(&mut buffer)).unwrap();

    let parsed = Header::read(&mut Cursor::new(&buffer)).unwrap();

    assert_eq!(original, parsed);
}
```

## Documentation

### Public API Documentation

All public items require documentation:

```rust
/// Parses a BLTE header from the given data.
///
/// # Arguments
///
/// * `data` - Raw bytes containing the BLTE header
///
/// # Returns
///
/// The parsed header on success, or an error if parsing fails.
///
/// # Errors
///
/// Returns `ParseError::InsufficientData` if the data is too short.
/// Returns `ParseError::InvalidMagic` if the magic bytes don't match.
///
/// # Examples
///
/// ```
/// use cascette_formats::blte::parse_header;
///
/// let data = include_bytes!("../fixtures/sample.blte");
/// let header = parse_header(data)?;
/// println!("Header size: {}", header.header_size);
/// # Ok::<(), cascette_formats::blte::ParseError>(())
/// ```
pub fn parse_header(data: &[u8]) -> Result<Header, ParseError> {
    // ...
}
```

### Binary Format Documentation

Document binary formats with exact byte layouts:

```rust
/// Archive index entry.
///
/// ## Binary Layout
///
/// | Offset | Size | Field | Description |
/// |--------|------|-------|-------------|
/// | 0x00 | 16 | key | Encoding key (MD5 hash) |
/// | 0x10 | 4 | size | Compressed size in bytes |
/// | 0x14 | 4 | offset | Offset into archive file |
///
/// Total size: 24 bytes (0x18)
///
/// All multi-byte fields are big-endian.
#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]
pub struct IndexEntry {
    pub key: [u8; 16],
    pub size: u32,
    pub offset: u32,
}
```

## Naming Conventions

### Types and Traits

| Item | Convention | Example |
|------|------------|---------|
| Structs | PascalCase | `ArchiveIndex`, `BlteHeader` |
| Enums | PascalCase | `CompressionType`, `ParseError` |
| Traits | PascalCase | `CascFormat`, `KeyStore` |
| Type aliases | PascalCase | `ContentKey`, `EncodingKey` |

### Functions and Methods

| Item | Convention | Example |
|------|------------|---------|
| Functions | snake_case | `parse_header`, `build_index` |
| Methods | snake_case | `self.get_entry()`, `self.is_valid()` |
| Constructors | `new` or `from_*` | `Header::new()`, `Key::from_hex()` |
| Conversions | `to_*` or `into_*` | `to_bytes()`, `into_vec()` |
| Getters | no prefix | `fn size(&self)` not `fn get_size(&self)` |
| Boolean getters | `is_*` or `has_*` | `is_empty()`, `has_entries()` |

### Constants and Statics

```rust
// Constants: SCREAMING_SNAKE_CASE
pub const HEADER_SIZE: usize = 16;
pub const MAGIC_BYTES: [u8; 4] = *b"BLTE";

// Statics (rare): SCREAMING_SNAKE_CASE
static GLOBAL_CONFIG: Lazy<Config> = Lazy::new(Config::default);
```

### Modules

Module names use snake_case:

```rust
mod archive;
mod blte;
mod encoding;
mod root;
```

File structure mirrors module structure:

```text
src/
├── archive/
│   ├── mod.rs
│   ├── index.rs
│   └── builder.rs
├── blte/
│   ├── mod.rs
│   ├── header.rs
│   └── compression.rs
└── lib.rs
```

## Memory and Performance

### Zero-Copy When Possible

Prefer borrowing over copying:

```rust
// Good - borrows data
pub fn parse<'a>(data: &'a [u8]) -> Result<Entry<'a>, Error> {
    Ok(Entry {
        key: &data[0..16],
        // ...
    })
}

// Less efficient - copies data
pub fn parse(data: &[u8]) -> Result<Entry, Error> {
    Ok(Entry {
        key: data[0..16].to_vec(),
        // ...
    })
}
```

### Avoid Loading Large Files Into Memory

Stream large files instead of loading entirely:

```rust
// Good - streams data
pub fn process_archive<R: Read + Seek>(reader: &mut R) -> Result<(), Error> {
    loop {
        let entry = read_entry(reader)?;
        process_entry(&entry)?;
    }
}

// Bad - loads everything
pub fn process_archive(data: &[u8]) -> Result<(), Error> {
    let archive = parse_entire_archive(data)?;  // Out of memory for large files
    // ...
}
```

### Use Appropriate Collection Types

| Use Case | Type |
|----------|------|
| Ordered, indexed access | `Vec<T>` |
| Key-value lookup | `HashMap<K, V>` or `BTreeMap<K, V>` |
| Unique values | `HashSet<T>` or `BTreeSet<T>` |
| Small fixed-size | `[T; N]` or `ArrayVec<T, N>` |
| Bytes | `Bytes` (from bytes crate) for shared ownership |

## Unsafe Code

Unsafe code requires explicit documentation:

```rust
/// # Safety
///
/// Caller must ensure:
/// - `ptr` is valid for reads of `len` bytes
/// - `ptr` is properly aligned for `T`
/// - The memory is not mutated during this call
pub unsafe fn read_from_ptr<T>(ptr: *const u8, len: usize) -> T {
    // ...
}
```

Prefer safe abstractions when possible. Use unsafe only when necessary for
performance or FFI.

## WASM Compatibility

Core libraries must compile to WASM. Avoid:

- C dependencies (use pure Rust implementations)
- File system access in library code
- Platform-specific code without `#[cfg]` guards

Test WASM compilation:

```bash
cargo check --target wasm32-unknown-unknown -p cascette-crypto
cargo check --target wasm32-unknown-unknown -p cascette-formats
```

# ribbit-client

[![Crates.io](https://img.shields.io/crates/v/ribbit-client.svg)](https://crates.io/crates/ribbit-client)
[![Documentation](https://docs.rs/ribbit-client/badge.svg)](https://docs.rs/ribbit-client)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../LICENSE)

Async TCP client for Blizzard's Ribbit protocol, used to retrieve version information,
CDN configurations, and other metadata for Blizzard games.

## Features

- 🚀 **Async/await support** - Built on Tokio for efficient async I/O
- 🌍 **Multi-region support** - US, EU, CN, KR, TW, SG regions
- 📝 **Protocol versions** - Both V1 (MIME) and V2 (raw PSV) protocols
- ✅ **Checksum validation** - SHA-256 integrity verification for V1 responses
- 🔐 **Signature parsing** - ASN.1/PKCS#7 signature extraction and validation
- 🛡️ **Type-safe** - Strongly typed endpoints and responses
- 📊 **Comprehensive testing** - Unit, integration, and benchmark tests
- 🎯 **Production ready** - Following Rust best practices with pedantic lints

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ribbit-client = "0.1"
tokio = { version = "1", features = ["full"] }
```

Basic usage:

```rust
use ribbit_client::{RibbitClient, Region, Endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client for the US region
    let client = RibbitClient::new(Region::US);

    // Request WoW version information
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    let response = client.request(&endpoint).await?;

    // Access the parsed data
    if let Some(data) = response.data {
        println!("WoW Versions:\n{}", data);
    }

    Ok(())
}
```

## Supported Endpoints

| Endpoint | Description | Example |
|----------|-------------|---------|
| `Summary` | List all available products | `client.request(&Endpoint::Summary)` |
| `ProductVersions` | Get version info for a product | `Endpoint::ProductVersions("wow".to_string())` |
| `ProductCdns` | Get CDN server information | `Endpoint::ProductCdns("wow".to_string())` |
| `ProductBgdl` | Background download config | `Endpoint::ProductBgdl("wow".to_string())` |
| `Cert` | Certificate by SHA-1 hash | `Endpoint::Cert(hash.to_string())` |
| `Ocsp` | OCSP response by hash | `Endpoint::Ocsp(hash.to_string())` |
| `Custom` | Any custom endpoint path | `Endpoint::Custom("custom/path".to_string())` |

## Protocol Versions

### V1 Protocol (MIME)

- Full MIME message parsing with multipart support
- SHA-256 checksum validation from epilogue
- ASN.1 signature parsing for attached signatures
- Automatic content type detection

### V2 Protocol (Raw PSV)

- Direct PSV (Pipe-Separated Values) format
- Lower overhead, faster parsing
- No MIME wrapper or checksums

```rust
// Use V2 protocol for better performance
let client = RibbitClient::new(Region::EU)
    .with_protocol_version(ProtocolVersion::V2);
```

## Examples

The crate includes several examples demonstrating different use cases:

```bash
# Basic client usage
cargo run --example basic_usage

# Parse version data into structured format
cargo run --example parse_versions

# Query multiple WoW products
cargo run --example wow_products

# Debug MIME structure (V1 protocol)
cargo run --example mime_parsing
```

## Response Structure

Responses contain both raw data and parsed components:

```rust
pub struct Response {
    /// Raw response bytes
    pub raw: Vec<u8>,

    /// Parsed data (PSV format)
    pub data: Option<String>,

    /// MIME parts (V1 only)
    pub mime_parts: Option<MimeParts>,
}

pub struct MimeParts {
    /// Main data content
    pub data: String,

    /// Signature bytes (if present)
    pub signature: Option<Vec<u8>>,

    /// Parsed signature information
    pub signature_info: Option<SignatureInfo>,

    /// SHA-256 checksum from epilogue
    pub checksum: Option<String>,
}
```

## PSV Data Format

Ribbit returns data in PSV (Pipe-Separated Values) format with typed columns:

```rust
// Example: Parse WoW version data
let endpoint = Endpoint::ProductVersions("wow".to_string());
let response = client.request(&endpoint).await?;

if let Some(data) = response.data {
    // First line contains column definitions
    // Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|...

    for line in data.lines().skip(1) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split('|').collect();
        let region = fields[0];
        let build_config = fields[1];
        let cdn_config = fields[2];
        // ... process remaining fields
    }
}
```

## Error Handling

The client provides detailed error types for different failure scenarios:

```rust
use ribbit_client::{Error, Result};

match client.request(&endpoint).await {
    Ok(response) => println!("Success!"),
    Err(Error::ConnectionFailed { host, port }) => {
        eprintln!("Failed to connect to {}:{}", host, port);
    }
    Err(Error::ChecksumMismatch) => {
        eprintln!("Response failed integrity check");
    }
    Err(Error::MimeParseError(msg)) => {
        eprintln!("Failed to parse MIME: {}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Advanced Usage

### Custom Region Configuration

```rust
let mut client = RibbitClient::new(Region::US);
client.set_region(Region::EU);
```

### Raw Response Access

```rust
// Get raw bytes for custom parsing
let raw_data = client.request_raw(&endpoint).await?;
println!("Received {} bytes", raw_data.len());
```

### V1 Protocol with Signature Verification

```rust
let client = RibbitClient::new(Region::US);  // V1 is default

let response = client.request(&endpoint).await?;
if let Some(mime_parts) = response.mime_parts {
    // Checksum is automatically validated
    println!("Checksum verified: {:?}", mime_parts.checksum);

    // Access signature information
    if let Some(sig_info) = mime_parts.signature_info {
        println!("Signature format: {}", sig_info.format);
        println!("Algorithm: {}", sig_info.algorithm);
        println!("Signers: {}", sig_info.signer_count);
        println!("Certificates: {}", sig_info.certificate_count);
    }
}
```

### Debugging

Enable trace logging to see detailed protocol information:

```rust
use tracing_subscriber::EnvFilter;

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env()
        .add_directive("ribbit_client=trace".parse()?))
    .init();
```

## Performance

The client is optimized for performance with:

- Reusable TCP connections per request
- Efficient MIME parsing with streaming support
- Zero-copy parsing where possible
- Async I/O for concurrent requests

Run benchmarks:

```bash
cargo bench
```

## Testing

The crate includes comprehensive tests:

```bash
# Run all tests
cargo test

# Run with trace logging
RUST_LOG=ribbit_client=trace cargo test

# Run specific test
cargo test test_ribbit_summary_v1
```

## Code Quality

This crate follows Rust best practices:

- Clippy pedantic lints enabled
- All public APIs documented with examples
- `#[must_use]` attributes where appropriate
- Comprehensive error documentation
- No unsafe code

## Contributing

Contributions are welcome! Please ensure:

- All tests pass
- Code follows Rust best practices
- Documentation is updated
- Examples demonstrate new features

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../LICENSE-MIT))

at your option.

## Acknowledgments

This crate is part of the cascette-rs project, providing tools for World of Warcraft
emulation development.

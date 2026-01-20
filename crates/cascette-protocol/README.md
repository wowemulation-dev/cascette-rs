# cascette-protocol

NGDP/CASC protocol implementation with Ribbit/TACT service discovery and CDN content delivery.

## Status

Working implementation with protocol clients, CDN downloading, and caching.

## Features

- Unified protocol client with automatic fallback (TACT HTTPS -> HTTP -> Ribbit TCP)
- TACT client for HTTPS/HTTP queries to `us.version.battle.net`
- Ribbit TCP client for direct protocol connections on port 1119
- CDN client for content downloads with range requests and progress tracking
- CDN streaming with BLTE decompression and concurrent chunk downloads
- Protocol response caching with configurable TTLs
- V1 MIME format support with PKCS#7 signature verification
- Connection pooling and HTTP/2 support via reqwest
- Retry policies with exponential backoff and jitter
- Thread-local buffers and string interning for performance

## Modules

- `client` - Unified `RibbitTactClient` with fallback between protocols
  - `ribbit` - Ribbit TCP protocol client
  - `tact` - TACT HTTPS/HTTP client
- `cdn` - CDN content delivery client
  - `range` - Range request support for partial downloads
- `cdn_streaming` - Streaming CDN downloads with BLTE decompression
  - `archive` - Archive streaming with index parsing
  - `blte` - BLTE block decompression
  - `bootstrap` - CDN bootstrap and initialization
  - `config` - Streaming configuration
  - `http` - HTTP streaming primitives
  - `metrics` - Prometheus metrics export
  - `optimizer` - Download optimization strategies
  - `path` - CDN path construction
  - `pool` - Connection pooling
  - `range` - Range request handling
  - `recovery` - Error recovery and retry
- `cache` - Protocol response caching
- `config` - Client and cache configuration
- `error` - Error types with retry classification
- `mime_parser` - BPSV response parsing
- `optimized` - Performance utilities (buffers, interning)
- `retry` - Retry policies with backoff
- `transport` - HTTP client configuration
- `v1_mime` - V1 MIME format with signature verification
  - `certificate` - X.509 certificate parsing
  - `signature` - PKCS#7/CMS signature verification
  - `types` - V1 MIME data types

## Usage

```rust
use cascette_protocol::{RibbitTactClient, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitTactClient::new(ClientConfig::default())?;

    // Query with automatic protocol fallback
    let versions = client.query("v1/products/wow/versions").await?;

    for row in versions.rows() {
        if let Some(version) = row.get_by_name("VersionsName", versions.schema()) {
            println!("Version: {}", version.as_string().unwrap_or("unknown"));
        }
    }

    Ok(())
}
```

## Platform Support

### Native (Full Support)
- Linux, macOS, Windows
- All features available including TCP Ribbit protocol and full caching

### WASM (Partial Support - In Progress)
- TCP Ribbit protocol disabled (browsers cannot use raw TCP sockets)
- Caching disabled (no persistent storage in browser environment)
- TACT HTTPS/HTTP protocol supported via Fetch API
- Requires additional work for full production use

## Dependencies

- `tokio` - Async runtime
- `reqwest` - HTTP client with rustls
- `async-trait` - Async trait support
- `bytes` - Zero-copy byte buffers
- `futures` - Async utilities
- `serde` - Serialization
- `tracing` - Logging and diagnostics
- `cascette-formats` - BPSV and other format parsers
- `cascette-cache` - Caching infrastructure
- `cascette-crypto` - Hash functions

### V1 MIME Support

- `mail-parser` - MIME message parsing
- `cms` - PKCS#7/CMS signature parsing
- `x509-cert` - X.509 certificate parsing
- `rsa` - RSA signature verification
- `sha2` - SHA-2 hash functions

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

---

**Note**: This project is not affiliated with Blizzard Entertainment. It is
an independent implementation based on reverse engineering by the World of
Warcraft emulation community.

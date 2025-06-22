# ngdp-cache

Generic caching functionality for NGDP (Next Generation Distribution Pipeline) components.

## Features

This crate provides specialized cache implementations for different NGDP components:

- **Generic Cache**: For arbitrary key-value storage
- **TACT Cache**: For TACT protocol data (configs, indices, data files)
- **CDN Cache**: For CDN content (archives, loose files)
- **Ribbit Cache**: For Ribbit protocol responses with TTL support
- **Cached Ribbit Client**: Wrapper around RibbitClient for transparent caching

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-cache = "0.1.0"
```

### Example

```rust
use ngdp_cache::{
    generic::GenericCache,
    tact::TactCache,
    cached_ribbit_client::CachedRibbitClient
};
use ribbit_client::{Endpoint, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generic cache for arbitrary data
    let cache = GenericCache::new().await?;
    cache.write("my_key", b"Hello, World!").await?;
    let data = cache.read("my_key").await?;

    // TACT cache follows CDN directory structure
    let tact = TactCache::new().await?;
    let hash = "abcdef1234567890abcdef1234567890";
    tact.write_config(hash, b"config data").await?;

    // Cached Ribbit client - a complete drop-in replacement for RibbitClient
    let client = CachedRibbitClient::new(Region::US).await?;

    // All RibbitClient methods work with transparent caching:
    let summary = client.get_summary().await?;  // Cached for 5 minutes
    let versions = client.get_product_versions("wow").await?;  // Also cached
    let cert = client.request_raw(&Endpoint::Cert("abc123".to_string())).await?;  // Cached for 30 days

    // Full Response objects are cached too
    let response = client.request(&Endpoint::ProductCdns("d4".to_string())).await?;

    // Typed responses work seamlessly
    let typed_versions = client.request_typed::<ribbit_client::ProductVersionsResponse>(
        &Endpoint::ProductVersions("wow".to_string())
    ).await?;

    Ok(())
}
```

## Cache Location

The cache is stored in the platform-specific cache directory:

- **Linux**: `~/.cache/ngdp/`
- **macOS**: `~/Library/Caches/ngdp/`
- **Windows**: `C:\Users\{user}\AppData\Local\ngdp\cache\`

Each cache type has its own subdirectory:

- `generic/` - Generic cache data
- `tact/` - TACT protocol metadata responses (versions, CDN configs, BGDL)
- `cdn/` - CDN content (config/, data/, patch/, indices)
- `ribbit/` - Ribbit responses organized by region/product/endpoint
- `ribbit/cached/` - Cached Ribbit client responses using Blizzard MIME naming

## 📄 License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

## 🫶 Acknowledgments

This crate is part of the `cascette-rs` project, providing tools for World of Warcraft
emulation development.

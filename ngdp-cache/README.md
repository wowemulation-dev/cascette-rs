# ngdp-cache

Generic caching functionality for NGDP (Next Generation Distribution Pipeline) components.

## Features

This crate provides specialized cache implementations for different NGDP components:

- **Generic Cache**: For arbitrary key-value storage
- **TACT Cache**: For TACT protocol data (configs, indices, data files)
- **CDN Cache**: For CDN content (archives, loose files)
- **Ribbit Cache**: For Ribbit protocol responses with TTL support
- **Cached Ribbit Client**: Wrapper around RibbitClient for transparent caching
- **Cached TACT Client**: Wrapper around TactClient for transparent caching of metadata
- **Cached CDN Client**: Wrapper around CdnClient for transparent caching of CDN content

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-cache = "0.4.3"
```

### Example

```rust
use ngdp_cache::{
    generic::GenericCache,
    cdn::CdnCache,
    cached_ribbit_client::CachedRibbitClient,
    cached_cdn_client::CachedCdnClient,
};
use ribbit_client::{Endpoint, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generic cache for arbitrary data
    let cache = GenericCache::new().await?;
    cache.write("my_key", b"Hello, World!").await?;
    let data = cache.read("my_key").await?;

    // CDN cache follows CDN directory structure
    let cdn_cache = CdnCache::new().await?;
    let hash = "abcdef1234567890abcdef1234567890";
    cdn_cache.write_config(hash, b"config data").await?;

    // Cached Ribbit client - a complete drop-in replacement for RibbitClient
    let ribbit_client = CachedRibbitClient::new(Region::US).await?;

    // All RibbitClient methods work with transparent caching:
    let summary = ribbit_client.get_summary().await?;  // Cached for 5 minutes
    let versions = ribbit_client.get_product_versions("wow").await?;  // Also cached
    let cert = ribbit_client.request_raw(&Endpoint::Cert("abc123".to_string())).await?;  // Cached for 30 days

    // Cached CDN client - transparent caching for CDN content
    let cdn_client = CachedCdnClient::new().await?;

    // Download content with automatic caching
    let response = cdn_client.download(
        "blzddist1-a.akamaihd.net",
        "tpr/wow",
        "2e9c1e3b5f5a0c9d9e8f1234567890ab"
    ).await?;

    // Check if response came from cache
    println!("From cache: {}", response.is_from_cache());
    let data = response.bytes().await?;

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

## License

This crate is dual-licensed under either:

- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## 🫶 Acknowledgments

This crate is part of the `cascette-rs` project, providing tools for World of Warcraft
emulation development.

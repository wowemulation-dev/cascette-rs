# ngdp-cache

Generic caching functionality for NGDP (Next Generation Data Pipeline) components.

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

    // Cached Ribbit client for transparent request caching
    let client = CachedRibbitClient::new(Region::US).await?;
    let endpoint = Endpoint::Cert("abc123".to_string());
    let raw_response = client.request_raw(&endpoint).await?;

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
- `tact/` - TACT protocol data (config/, data/, patch/)
- `cdn/` - CDN content (archives/, loose/)
- `ribbit/` - Ribbit responses organized by region/product/endpoint
- `ribbit/cached/` - Cached Ribbit client responses using Blizzard MIME naming

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.

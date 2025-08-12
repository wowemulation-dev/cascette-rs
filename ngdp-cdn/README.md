# ngdp-cdn

CDN client for downloading NGDP (Next Generation Distribution Pipeline) content from
Blizzard's CDN servers.

## Features

- **Async/await** - Built on Tokio for async I/O
- **Retry logic** - Configurable exponential backoff with jitter
- **Connection pooling** - Reuse of connections for multiple downloads
- **Compression support** - Automatic gzip/deflate decompression
- **Concurrent downloads** - Download multiple files in parallel
- **Error handling** - Error types for CDN operations
- **Configurable timeouts** - Separate connection and request timeouts
- **Fallback support** - Built-in backup CDN support with configurable hosts

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-cdn = "0.4.3"
```

### Basic Example

```rust
use ngdp_cdn::CdnClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a CDN client
    let client = CdnClient::new()?;

    // Download content by hash
    let response = client.download(
        "blzddist1-a.akamaihd.net",
        "tpr/wow",
        "2e9c1e3b5f5a0c9d9e8f1234567890ab",
    ).await?;

    let content = response.bytes().await?;
    println!("Downloaded {} bytes", content.len());

    Ok(())
}
```

### Advanced Configuration

```rust
use ngdp_cdn::CdnClient;

let client = CdnClient::builder()
    .max_retries(5)
    .initial_backoff_ms(200)
    .max_backoff_ms(30_000)
    .connect_timeout(60)
    .request_timeout(300)
    .pool_max_idle_per_host(50)
    .build()?;
```

### CDN Fallback Support

The crate provides `CdnClientWithFallback` which tries multiple CDN hosts
when downloads fail. It prioritizes Blizzard's official CDN servers first, then falls
back to community mirrors if all primary servers fail.

```rust
use ngdp_cdn::CdnClientWithFallback;

// Create with default backup CDNs (arctium.tools and reliquaryhq.com)
let client = CdnClientWithFallback::new()?;

// Add Blizzard CDNs from Ribbit response (these are tried first)
client.add_primary_cdns(vec![
    "blzddist1-a.akamaihd.net",
    "level3.blizzard.com",
    "blzddist2-a.akamaihd.net",
]);

// Download process:
// 1. Try blzddist1-a.akamaihd.net
// 2. Try level3.blizzard.com
// 3. Try blzddist2-a.akamaihd.net
// 4. Try cdn.arctium.tools (community backup)
// 5. Try tact.mirror.reliquaryhq.com (community backup)
let response = client.download("tpr/wow", "content_hash").await?;
```

#### Default Backup CDNs

By default, the fallback client includes two backup CDN servers that are only
used after all Blizzard CDNs have been exhausted:

- `http://cdn.arctium.tools/`
- `https://tact.mirror.reliquaryhq.com/`

These are community-maintained mirrors that provide access to game content when
official servers are unavailable.

#### Contributing Community CDN Changes

If you'd like to suggest changes to the default community backup CDNs (add new ones,
update existing ones, or remove inactive ones), please file a ticket on our
[GitHub Issues](https://github.com/wowemulation-dev/cascette-rs/issues) page.

When suggesting a community CDN, please include:

- The CDN URL and confirmation it supports NGDP/TACT protocols
- Information about who maintains it and its reliability
- Any regional restrictions or limitations

#### Custom Configuration

```rust
let client = CdnClientWithFallback::builder()
    .add_primary_cdn("primary.example.com")
    .add_primary_cdn("secondary.example.com")
    .use_default_backups(false)  // Disable default backup CDNs
    .configure_base_client(|builder| {
        builder
            .max_retries(5)
            .initial_backoff_ms(200)
    })
    .build()?;
```

### CDN URL Structure

NGDP CDN URLs follow a specific pattern for content addressing:

```text
http://{cdn_host}/{path}/{hash[0:2]}/{hash[2:4]}/{hash}
```

For example:

- Hash: `2e9c1e3b5f5a0c9d9e8f1234567890ab`
- CDN Host: `blzddist1-a.akamaihd.net`
- Path: `tpr/wow`
- Results in: `http://blzddist1-a.akamaihd.net/tpr/wow/2e/9c/2e9c1e3b5f5a0c9d9e8f1234567890ab`

### Error Handling

The crate provides specific error types for CDN operations:

```rust
use ngdp_cdn::Error;

match client.download(host, path, hash).await {
    Ok(response) => {
        // Process response
    }
    Err(Error::ContentNotFound { hash }) => {
        println!("Content {} not found on CDN", hash);
    }
    Err(Error::RateLimited { retry_after_secs }) => {
        println!("Rate limited, retry after {} seconds", retry_after_secs);
    }
    Err(e) => {
        println!("Other error: {}", e);
    }
}
```

## Integration with NGDP

This crate is designed to work with other NGDP components:

1. Use `ribbit-client` to get CDN configuration
2. Use `tact-client` to get content manifests
3. Use `ngdp-cdn` to download the actual content

```rust
// Example workflow with automatic fallback
let ribbit = ribbit_client::RibbitClient::new(Region::US);
let cdns = ribbit.get_product_cdns("wow").await?;

let tact = tact_client::HttpClient::new(Region::US)?;
let versions = tact.get_versions_parsed("wow").await?;

// Use fallback client for automatic CDN failover
let cdn_client = ngdp_cdn::CdnClientWithFallback::new()?;

// Add all CDN hosts from Ribbit response as primary
for cdn_entry in &cdns {
    cdn_client.add_primary_cdns(&cdn_entry.hosts);
}

// Download will automatically try all CDNs (primary + backup)
let content = cdn_client.download(&cdns[0].path, &content_hash).await?;
```

## Performance Considerations

- **Connection pooling**: The client maintains a pool of connections to each host
- **Concurrent downloads**: Use multiple client instances or clone the client for
  parallel downloads
- **Retry strategy**: Configure retry parameters based on network conditions
- **Timeouts**: Adjust timeouts based on file sizes and network speed

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

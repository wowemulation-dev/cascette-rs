# ngdp-cdn

CDN client for downloading NGDP (Next Generation Distribution Pipeline) content from
Blizzard's CDN servers.

## Features

- 🚀 **Async/await** - Built on Tokio for high-performance async I/O
- 🔄 **Automatic retry** - Configurable exponential backoff with jitter
- 🏊 **Connection pooling** - Efficient reuse of connections for multiple downloads
- 🗜️ **Compression support** - Automatic gzip/deflate decompression
- ⚡ **Concurrent downloads** - Download multiple files in parallel
- 🛡️ **Error handling** - Comprehensive error types for CDN operations
- ⏱️ **Configurable timeouts** - Separate connection and request timeouts

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
ngdp-cdn = "0.1"
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
// Example workflow (simplified)
let ribbit = ribbit_client::RibbitClient::new(Region::US);
let cdns = ribbit.get_product_cdns("wow").await?;

let tact = tact_client::HttpClient::new(Region::US)?;
let versions = tact.get_versions_parsed("wow").await?;

let cdn_client = ngdp_cdn::CdnClient::new()?;
for host in cdns.hosts {
    if let Ok(content) = cdn_client.download(&host, &cdns.path, &content_hash).await {
        // Successfully downloaded from this CDN
        break;
    }
}
```

## Performance Considerations

- **Connection pooling**: The client maintains a pool of connections to each host
- **Concurrent downloads**: Use multiple client instances or clone the client for
  parallel downloads
- **Retry strategy**: Configure retry parameters based on your network conditions
- **Timeouts**: Adjust timeouts based on file sizes and network speed

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

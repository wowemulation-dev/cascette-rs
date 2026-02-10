# cascette-ribbit

Ribbit protocol server for NGDP/CASC installations.

## Protocols

### HTTP/HTTPS (TACT v2)

- `GET /{product}/versions` - Version information
- `GET /{product}/cdns` - CDN configuration
- `GET /{product}/bgdl` - Background download information

### TCP (Ribbit v1)

MIME-wrapped responses with SHA-256 checksums:

- `v1/products/{product}/versions`
- `v1/products/{product}/cdns`
- `v1/products/{product}/bgdl`
- `v1/summary` - List all products

### TCP (Ribbit v2)

Raw BPSV responses:

- `v2/products/{product}/versions`
- `v2/products/{product}/cdns`
- `v2/products/{product}/bgdl`

## Usage

```rust
use cascette_ribbit::{Server, ServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = ServerConfig::from_args();
    config.validate()?;

    let server = Server::new(config)?;
    server.run().await?;

    Ok(())
}
```

### Configuration

Configuration via CLI arguments or environment variables:

- `--http-bind` / `CASCETTE_RIBBIT_HTTP_BIND` (default: `0.0.0.0:8080`)
- `--tcp-bind` / `CASCETTE_RIBBIT_TCP_BIND` (default: `0.0.0.0:1119`)
- `--builds` / `CASCETTE_RIBBIT_BUILDS` (default: `./builds.json`)
- `--cdn-hosts` / `CASCETTE_RIBBIT_CDN_HOSTS` (default: `cdn.arctium.tools`)
- `--cdn-path` / `CASCETTE_RIBBIT_CDN_PATH` (default: `tpr/wow`)
- `--tls-cert` / `CASCETTE_RIBBIT_TLS_CERT` (optional, enables HTTPS)
- `--tls-key` / `CASCETTE_RIBBIT_TLS_KEY` (required if TLS enabled)

### Build Database

JSON format with build records:

```json
[{
  "id": 1,
  "product": "wow",
  "version": "1.14.2.42597",
  "build": "42597",
  "build_config": "0123456789abcdef0123456789abcdef",
  "cdn_config": "fedcba9876543210fedcba9876543210",
  "product_config": null,
  "build_time": "2024-01-01T00:00:00+00:00",
  "encoding_ekey": "aaaabbbbccccddddeeeeffffaaaaffff",
  "root_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
  "install_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
  "download_ekey": "ddddeeeeffffaaaabbbbccccddddeeee"
}]
```

## Testing

```bash
cargo test --package cascette-ribbit
cargo bench --package cascette-ribbit
```

## License

See project root for license information.

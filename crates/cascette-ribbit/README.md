# cascette-ribbit

Ribbit protocol server for NGDP/CASC installations.

Implements all protocol variants used by the Blizzard Agent and game
clients, plus community extensions for versioned (historical) builds.

## Endpoint Reference

### TCP Ribbit (port 1119)

Line-based protocol. The client sends a command string terminated by
`\r\n` or `\n`; the server responds with BPSV (v2) or MIME-wrapped
BPSV (v1).

#### v1 -- MIME-wrapped with SHA-256 checksum

| Command | Status |
|---------|--------|
| `v1/products/{product}/versions` | implemented |
| `v1/products/{product}/cdns` | implemented |
| `v1/products/{product}/bgdl` | implemented |
| `v1/summary` | implemented |

#### v2 -- raw BPSV

| Command | Status |
|---------|--------|
| `v2/products/{product}/versions` | implemented |
| `v2/products/{product}/cdns` | implemented |
| `v2/products/{product}/bgdl` | implemented |
| `v2/products/summary` | implemented |

### HTTP v1 (legacy path format)

Used by older game clients (pre-1.15.8) after binary patching. The
Blizzard Agent also uses this format on port 1119 over plain HTTP.

Blizzard URL pattern: `http://{region}.patch.battle.net:1119/{product}/{endpoint}`

| Route | Status |
|-------|--------|
| `GET /{product}/versions` | implemented |
| `GET /{product}/cdns` | implemented |
| `GET /{product}/bgdl` | implemented |

### HTTPS v2 (modern path format)

Used by newer game clients and community tools.

Blizzard URL pattern: `https://{region}.version.battle.net/v2/products/{product}/{endpoint}`

| Route | Status |
|-------|--------|
| `GET /v2/products/{product}/versions` | implemented |
| `GET /v2/products/{product}/cdns` | implemented |
| `GET /v2/products/{product}/bgdl` | implemented |
| `GET /v2/products/summary` | implemented |

### Versioned API (community extension)

Returns metadata for a specific historical build instead of the latest.
This allows patched clients running older versions to receive matching
build/CDN configs without being told to update.

| Route | Status |
|-------|--------|
| `GET /v2/products/{product}/versions/{build}` | implemented |
| `GET /v2/products/{product}/cdns/{build}` | implemented |
| `GET /v2/products/{product}/bgdl/{build}` | implemented |
| `GET /{product}/versions/{build}` | implemented |
| `GET /{product}/cdns/{build}` | implemented |
| `GET /{product}/bgdl/{build}` | implemented |

### Web UI (dashboard)

| Route | Status |
|-------|--------|
| `GET /` | implemented |
| `GET /{product}/builds` | implemented |

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

- `--http-bind` / `CASCETTE_RIBBIT_HTTP_BIND` (default: `127.0.0.1:8080`)
- `--tcp-bind` / `CASCETTE_RIBBIT_TCP_BIND` (default: `127.0.0.1:1119`)
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

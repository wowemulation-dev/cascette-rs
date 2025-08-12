# ngdp-client

Command-line interface for interacting with Blizzard's NGDP (Next Generation
Distribution Pipeline) services.

## Features

- Query product information from Ribbit protocol
- Manage local CASC storage (placeholder)
- Download content using TACT protocol (placeholder)
- Inspect NGDP data structures
- Download certificates by SKI/hash
- Configuration management
- Built-in caching for API responses
- Fallback from Ribbit to TACT on failures
- View historical build data from Wago Tools API

## Installation

### Install from crates.io

```bash
cargo install ngdp-client
```

### Install with Script (Unix/Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/wowemulation-dev/cascette-rs/main/install.sh | bash
```

### Install with Script (Windows PowerShell)

```powershell
irm https://raw.githubusercontent.com/wowemulation-dev/cascette-rs/main/install.ps1 | iex
```

### Build from source

```bash
git clone https://github.com/wowemulation-dev/cascette-rs
cd cascette-rs
cargo install --path ngdp-client
```

## Usage

### Basic Commands

```bash
# Show help
ngdp --help

# List all products
ngdp products list

# Filter products by name
ngdp products list --filter wow

# Show product versions
ngdp products versions wow

# Show all regions for a product
ngdp products versions wow --all-regions

# Get product information (specific region)
ngdp products info wow --region eu

# Get product information (all regions)
ngdp products info wow

# Show CDN configuration for a specific region
ngdp products cdns wow --region us

# Show all historical builds for a product
ngdp products builds wow

# Filter builds by version pattern
ngdp products builds wow --filter "11.1.7"

# Show only builds from last 7 days
ngdp products builds wow --days 7

# Limit results to 10 most recent builds
ngdp products builds wow --limit 10

# Show only background download builds
ngdp products builds wow --bgdl-only
```

### Output Formats

The CLI supports multiple output formats:

```bash
# Default text output
ngdp products list

# JSON output
ngdp products list -o json

# Pretty-printed JSON
ngdp products list -o json-pretty

# Raw BPSV format
ngdp products list -o bpsv
```

### Inspect Commands

```bash
# Parse and display BPSV data
ngdp inspect bpsv data.bpsv

# Show raw BPSV data
ngdp inspect bpsv data.bpsv --raw

# Inspect from URL
ngdp inspect bpsv https://example.com/data.bpsv
```

### Certificate Commands

```bash
# Download a certificate by SKI/hash
ngdp certs download 5168ff90af0207753cccd9656462a212b859723b

# Download and show certificate details
ngdp certs download 5168ff90af0207753cccd9656462a212b859723b --details

# Save certificate to file
ngdp certs download 5168ff90af0207753cccd9656462a212b859723b --output cert.pem

# Download certificate in DER format
ngdp certs download 5168ff90af0207753cccd9656462a212b859723b --output cert.der --cert-format der

# Get certificate details as JSON
ngdp certs download 5168ff90af0207753cccd9656462a212b859723b --details -o json
```

### Configuration

```bash
# Show current configuration
ngdp config show

# Get a specific config value
ngdp config get default_region

# Set a config value
ngdp config set default_region eu

# Reset configuration to defaults
ngdp config reset --yes
```

**Available Configuration Settings:**

- `default_region` - Default region for API requests (default: us)
- `cache_dir` - Directory for cached data (default: ~/.cache/ngdp)
- `cache_enabled` - Enable/disable caching (default: true)
- `cache_ttl` - Cache time-to-live in seconds (default: 1800)
- `timeout` - General request timeout in seconds (default: 30)
- `ribbit_timeout` - Ribbit-specific timeout (default: 30)
- `tact_timeout` - TACT-specific timeout (default: 30)
- `max_concurrent_downloads` - Max parallel downloads (default: 4)
- `retry_attempts` - Number of retry attempts (default: 3)
- `user_agent` - HTTP User-Agent string (default: ngdp-client/0.1.2)
- `verify_certificates` - SSL certificate verification (default: true)
- `proxy_url` - HTTP proxy URL (default: empty)
- `log_file` - Log output file (default: empty)
- `color_output` - Enable colored terminal output (default: true)
- `fallback_to_tact` - Auto-fallback from Ribbit to TACT (default: true)
- `use_community_cdn_fallbacks` - Use community CDN mirrors (default: true)
- `custom_cdn_fallbacks` - Comma-separated list of custom CDN hosts (default: empty)

### Caching and Fallback

The CLI includes built-in caching for both Ribbit and TACT API responses with automatic fallback:

```bash
# Disable caching for a single command
ngdp products list --no-cache

# Clear all cached data before running command
ngdp products list --clear-cache
```

**Fallback Behavior:**

- Primary: Ribbit protocol (TCP-based, official)
- Fallback: TACT HTTP protocol (when Ribbit fails)
- Both protocols return identical BPSV data
- Caching works transparently for both protocols
- SG region automatically falls back to US for TACT

**CDN Fallback Order:**

When downloading content from CDNs, the client tries hosts in this order:

1. **Primary CDNs** - Blizzard's official CDN servers
2. **Community CDNs** - Public mirrors (if `use_community_cdn_fallbacks` is true):
   - `cdn.arctium.tools`
   - `tact.mirror.reliquaryhq.com`
3. **Custom CDNs** - User-configured hosts from `custom_cdn_fallbacks`

To configure custom CDN fallbacks:

```bash
# Set custom CDN fallbacks (comma-separated)
ngdp config set custom_cdn_fallbacks "my-cdn1.example.com,my-cdn2.example.com"

# Disable community CDN fallbacks
ngdp config set use_community_cdn_fallbacks false
```

## Library Usage

The ngdp-client can also be used as a library:

```rust
use ngdp_client::{handle_products, OutputFormat, ProductsCommands};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cmd = ProductsCommands::List {
        filter: Some("wow".to_string()),
        region: "us".to_string(),
    };

    handle_products(cmd, OutputFormat::Json).await?;
    Ok(())
}
```

## Examples

See the `examples/` directory for more usage examples.

## Development

### Running Tests

```bash
cargo test
```

### Running Benchmarks

```bash
cargo bench
```

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

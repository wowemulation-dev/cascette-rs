# ngdp-client

Command-line interface for interacting with Blizzard's NGDP (Next Generation Data Pipeline) services.

## Features

- 📦 Query product information from Ribbit protocol
- 📂 Manage local CASC storage (placeholder)
- ⬇️ Download content using TACT protocol (placeholder)
- 🔍 Inspect NGDP data structures
- 🔐 Download certificates by SKI/hash
- ⚙️ Configuration management
- 💾 Built-in caching for API responses

## Installation

```bash
cargo install --path .
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

# Get product information
ngdp products info wow

# Show CDN configuration
ngdp products cdns wow
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

### Caching

The CLI includes built-in caching for Ribbit API responses:

```bash
# Disable caching for a single command
ngdp products list --no-cache

# Clear all cached data before running command
ngdp products list --clear-cache
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

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../LICENSE-MIT))

at your option.

## Acknowledgments

This crate is part of the cascette-rs project, providing tools for World of Warcraft
emulation development.

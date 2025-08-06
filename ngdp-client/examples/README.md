# ngdp-client Examples

This directory contains examples demonstrating how to use the `ngdp-client` crate both as a CLI tool and as a library.

## Available Examples

### `certificate_operations.rs`
Demonstrates certificate-related functionality:
- Downloading certificates by SKI (Subject Key Identifier)
- Certificate details extraction (subject, issuer, validity)
- PEM and DER format handling
- Certificate verification workflows
- Integration with Ribbit client for certificate fetching

### `fallback_demo.rs`
Shows automatic Ribbit to TACT fallback functionality:
- FallbackClient configuration and usage
- Automatic protocol switching (Ribbit → TACT)
- Regional compatibility handling (SG → US mapping)
- Error recovery and resilience
- Performance comparison between protocols

### `products_operations.rs`
Comprehensive product query operations:
- Product listing with filtering
- Version information retrieval
- CDN configuration analysis
- Build configuration inspection
- Cross-region product comparison

### `wago_builds.rs`
Integration with Wago Tools API for historical build data:
- Historical build retrieval for products
- Version pattern filtering
- Time-based build filtering
- Background download build identification
- Build metadata analysis

## Library Usage

The examples show how to use `ngdp-client` as a library in your own applications:

```rust
use ngdp_client::{
    cached_client::create_client,
    output::{OutputStyle, format_success}
};
use ribbit_client::Region;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a cached client
    let client = create_client(Region::US).await?;
    
    // Query product versions
    let versions = client.get_product_versions("wow").await?;
    
    // Process results
    for entry in &versions.entries {
        println!("Build: {} - {}", entry.build_id, entry.versions_name);
    }
    
    Ok(())
}
```

## CLI Integration

Examples also demonstrate advanced CLI patterns:
- Custom output formatting
- Progress reporting
- Error handling strategies
- Configuration management
- Caching integration

## Running Examples

To run any example:
```bash
cargo run --example <example_name> -p ngdp-client
```

For example:
```bash
cargo run --example products_operations -p ngdp-client
cargo run --example certificate_operations -p ngdp-client
cargo run --example fallback_demo -p ngdp-client
cargo run --example wago_builds -p ngdp-client
```

## Integration Features

The examples demonstrate integration with other crates:

### Caching Integration
- Transparent caching with `ngdp-cache`
- Performance improvements through cached responses
- Cache management and statistics

### Protocol Integration  
- Ribbit client integration for version queries
- TACT client for CDN configuration
- CDN client for content downloads
- BPSV parsing for data processing

### Output Formatting
- Beautiful terminal output with Unicode tables
- Color support with automatic detection
- Multiple output formats (text, JSON, BPSV)
- Progress indicators and status messages

## Error Handling

Examples show comprehensive error handling:
- Network connectivity issues
- Protocol-specific errors
- Data parsing failures
- Authentication problems
- Rate limiting responses

## Performance Optimization

Examples demonstrate performance features:
- Connection pooling and reuse
- Parallel operations where appropriate
- Caching for reduced API calls
- Streaming for large data sets
- Memory-efficient processing

## Configuration

Examples show various configuration options:
- Region selection and switching
- Protocol version preferences
- Timeout and retry settings
- Cache configuration
- Output format selection

Run examples with different configurations:
```bash
# With specific region
REGION=eu cargo run --example products_operations -p ngdp-client

# With debug logging
RUST_LOG=debug cargo run --example fallback_demo -p ngdp-client

# With custom cache settings
CACHE_TTL=3600 cargo run --example certificate_operations -p ngdp-client
```
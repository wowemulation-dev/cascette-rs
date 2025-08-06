# tact-client Examples

This directory contains examples demonstrating how to use the `tact-client` crate for accessing Blizzard's TACT HTTP endpoints.

## Available Examples

### `tact_basic_usage.rs`
Introduction to the TACT client with common operations:
- Creating clients for different protocol versions (V1/V2)
- Querying product versions and CDN configurations  
- Switching between regions
- Basic error handling and response parsing

### `tact_retry_handling.rs`
Demonstrates automatic retry configuration:
- Exponential backoff configuration
- Retry strategies for different error types
- Custom retry parameters (max retries, backoff timing, jitter)
- Handling non-retryable errors

### `explore_endpoints.rs`
Endpoint discovery and testing tool:
- Testing different TACT endpoints
- Response format analysis
- Error condition handling
- Protocol version comparison

### `compare_cdn_formats.rs`
Compares CDN host and server field formats:
- Legacy hosts field (bare hostnames)
- Modern servers field (full URLs)
- URL construction patterns
- Compatibility considerations

## Protocol Versions

### V1 Protocol
- Uses TCP connection to port 1119
- Base URL: `http://{region}.patch.battle.net:1119`
- Legacy protocol with established compatibility

### V2 Protocol  
- Uses HTTPS REST API
- Base URL: `https://{region}.version.battle.net/v2/products`
- Modern protocol with improved performance

## Available Endpoints

All endpoints support both protocol versions:

- `/{product}/versions` - Version manifest with build configurations
- `/{product}/cdns` - CDN configuration and hosts  
- `/{product}/bgdl` - Background downloader manifest

## Running Examples

To run any example:
```bash
cargo run --example <example_name> -p tact-client
```

For example:
```bash
cargo run --example tact_basic_usage -p tact-client
cargo run --example tact_retry_handling -p tact-client
cargo run --example explore_endpoints -p tact-client
```

## Supported Products

The examples work with all Blizzard products:
- `wow` - World of Warcraft Retail
- `wow_classic` - WoW Classic
- `wow_classic_era` - WoW Classic Era
- `wowt` - WoW Test/PTR
- `wowxptr` - WoW Experimental PTR
- `wowdev` - WoW Development
- And many more...

## Error Handling

Examples demonstrate comprehensive error handling:
- Network connectivity issues
- Invalid product names
- Region accessibility problems
- Protocol version compatibility
- Malformed response handling

## Performance Notes

Examples show performance considerations:
- Connection pooling benefits
- Retry strategy impact
- Protocol version differences
- Region selection effects

Use timing to compare performance:
```bash
time cargo run --example tact_basic_usage -p tact-client
```
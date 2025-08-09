# TACT Client

HTTP client for Blizzard's TACT (Trusted Application Content Transfer) protocol.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tact-client = "0.3"
tokio = { version = "1", features = ["full"] }
```

## Protocol Versions

### TACT v1 (TCP-based)

- Base URL: `http://{region}.patch.battle.net:1119`
- Regions: `us`, `eu`, `kr`, `cn`, `tw`

### TACT v2 (HTTPS-based)

- Base URL: `https://{region}.version.battle.net/v2/products`
- Regions: Same as v1

## Available Endpoints

### TACT v1 Endpoints

| Endpoint | Description | Response Format |
|----------|-------------|-----------------|
| `/{product}/versions` | Version manifest with build configs | Pipe-delimited table with headers |
| `/{product}/cdns` | CDN configuration and hosts | Pipe-delimited table with CDN URLs |
| `/{product}/bgdl` | Background downloader manifest | Pipe-delimited table (often empty) |

### TACT v2 Endpoints

The v2 protocol provides the same endpoints as v1:

- `/{product}/versions` - Same format as v1
- `/{product}/cdns` - Same format as v1
- `/{product}/bgdl` - Same format as v1

**Note**: TACT v2 appears to be a proxy to v1 endpoints, returning identical data formats.

## Response Formats

### Versions Response

```
Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16
## seqn = 3020098
us|e359107662e72559b4e1ab721b157cb0|48c7c7dfe4ea7df9dac22f6937ecbf47|3ca57fe7319a297346440e4d2a03a0cd|61559|11.1.7.61559|53020d32e1a25648c8e1eafd5771935f
```

### CDNs Response

```
Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
## seqn = 2241282
us|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com us.cdn.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4...|tpr/configs/data
```

## Usage Example

### Basic Usage (Raw Responses)

```rust
use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client (V2 is the default)
    let client = HttpClient::new(Region::US, ProtocolVersion::V2)?;

    // Or use the Default implementation
    let client = HttpClient::default();

    // Fetch versions
    let response = client.get_versions("wow").await?;
    let versions_data = response.text().await?;

    // Fetch CDN configuration
    let response = client.get_cdns("wow").await?;
    let cdn_data = response.text().await?;

    Ok(())
}
```

### Typed Responses (Recommended)

The client provides typed parsing functions for structured data access:

```rust
use tact_client::{HttpClient, parse_versions, parse_cdns, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HttpClient::default();

    // Fetch and parse versions
    let response = client.get_versions("wow").await?;
    let versions_data = response.text().await?;
    let versions = parse_versions(&versions_data)?;

    // Access typed fields
    for version in &versions {
        println!("Region: {}", version.region);
        println!("Build: {} ({})", version.build_id, version.versions_name);
        println!("Build Config: {}", version.build_config);
    }

    // Fetch and parse CDN configuration
    let response = client.get_cdns("wow").await?;
    let cdn_data = response.text().await?;
    let cdns = parse_cdns(&cdn_data)?;

    for cdn in &cdns {
        println!("Region: {}", cdn.name);
        println!("CDN Hosts: {:?}", cdn.hosts);
        println!("CDN Servers: {:?}", cdn.servers);
    }

    Ok(())
}
```

## Protocol Versions

The TACT client supports two protocol versions:

### V1 Protocol

- HTTP-based on port 1119 (`http://{region}.patch.battle.net:1119`)
- Legacy format, still supported

### V2 Protocol - Default

- HTTPS-based REST API (`https://{region}.version.battle.net/v2/products`)
- Modern, secure protocol
- **This is the default protocol**

```rust
// V2 is the default
let client = HttpClient::new(Region::US, ProtocolVersion::V2)?;

// Or explicitly use V1 if needed
let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;
```

## Supported Products

Tested and working products:

- `wow` - World of Warcraft
- `wow_classic` - WoW Classic
- `wowt` - WoW Test
- `wow_beta` - WoW Beta
- `agent` - Battle.net Agent
- `bna` - Battle.net App
- `pro` - Overwatch
- `s2` - StarCraft II
- `d3` - Diablo 3
- `hero` - Heroes of the Storm
- `hsb` - Hearthstone
- `w3` - Warcraft III

## Notes

1. BGDL (Background Downloader) returns empty responses for some products
2. Both v1 and v2 protocols return the same data format, suggesting v2 is a proxy/wrapper around v1
3. The actual file content is downloaded from CDN hosts listed in the CDN configuration
4. File paths on CDN use hash-based directory structure: `/{hash[0:2]}/{hash[2:4]}/{hash}`

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

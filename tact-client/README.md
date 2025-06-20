# TACT Client

HTTP client for Blizzard's TACT (Transfer And Content Transfer) protocol.

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

```rust
use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client for v1 protocol
    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;
    
    // Fetch versions
    let response = client.get_versions("wow").await?;
    let versions_data = response.text().await?;
    
    // Fetch CDN configuration
    let response = client.get_cdns("wow").await?;
    let cdn_data = response.text().await?;
    
    Ok(())
}
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
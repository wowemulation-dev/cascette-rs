# Ribbit Protocol - Detailed Technical Specification

## Overview

Ribbit is Blizzard's protocol for querying product versions, CDN configurations, and distributing game metadata. It operates over TCP and provides both signed (v1) and unsigned (v2) API endpoints.

## Connection Details

### Server Endpoints

| Region | Hostname | Port |
|--------|----------|------|
| US | us.version.battle.net | 1119 |
| EU | eu.version.battle.net | 1119 |
| KR | kr.version.battle.net | 1119 |
| TW | tw.version.battle.net | 1119 |
| CN | cn.version.battle.net | 1119 |

### Protocol

- **Transport**: TCP socket
- **Port**: 1119
- **Format**: Text-based with MIME-like responses

## API Commands

### V1 API (Signed Responses)

#### `/v1/summary`
Returns a list of all products and their sequence numbers.

**Request Format**:
```
v1/summary
```

**Response Format**:
```
## seqn = 123456
Product!STRING:0|Seqn!DEC:4|Flags!STRING:0
wow|654321|
wow_classic|654322|
```

#### `/v1/products/{product}/versions`
Returns version information for a specific product.

**Request Format**:
```
v1/products/wow/versions
```

**Response Format**:
```
## seqn = 654321
Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16
us|abc123...|def456...|789012...|52237|10.2.5.52237|345678...
eu|abc123...|def456...|789012...|52237|10.2.5.52237|345678...
```

**Field Descriptions**:
- **Region**: Geographic region code
- **BuildConfig**: Hash of build configuration file
- **CDNConfig**: Hash of CDN configuration file
- **KeyRing**: Encryption keys for protected content
- **BuildId**: Numeric build identifier
- **VersionsName**: Human-readable version string
- **ProductConfig**: Product-specific configuration hash

#### `/v1/products/{product}/cdns`
Returns CDN server information for a product.

**Request Format**:
```
v1/products/wow/cdns
```

**Response Format**:
```
## seqn = 654323
Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|level3.blizzard.com edgecast.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://edgecast.blizzard.com/?maxhosts=4|tpr/configs/data
eu|tpr/wow|level3.blizzard.com edgecast.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://edgecast.blizzard.com/?maxhosts=4|tpr/configs/data
```

**Field Descriptions**:
- **Name**: CDN region identifier
- **Path**: Base path for content files
- **Hosts**: Space-separated list of CDN hostnames
- **Servers**: Full CDN URLs with parameters
- **ConfigPath**: Path to configuration files

#### `/v1/products/{product}/bgdl`
Returns background download information.

**Request Format**:
```
v1/products/wow/bgdl
```

**Response Format**:
```
## seqn = 654324
Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16
us|abc123...|def456...|789012...|52238|10.2.6.52238|345678...
```

#### `/v1/certs/{hash}`
Retrieves X.509 certificates for signature verification.

**Request Format**:
```
v1/certs/abc123def456...
```

**Response**: Raw certificate data

#### `/v1/ocsp/{hash}`
Checks certificate revocation status via OCSP.

**Request Format**:
```
v1/ocsp/abc123def456...
```

**Response**: OCSP response data

### V2 API (Unsigned Responses)

V2 API provides the same endpoints but with simplified responses:
- `/v2/summary`
- `/v2/products/{product}/versions`
- `/v2/products/{product}/bgdl`

**Key Differences**:
- No MIME headers
- No signatures
- Direct content only
- Faster parsing

## Response Format Details

### V1 Response Structure

```
--MIME_boundary
Content-Type: text/plain
Content-Encoding: gzip
Content-MD5: {base64_md5}

{gzipped_content}

--MIME_boundary
Content-Type: application/octet-stream
Content-Encoding: raw
Content-MD5: {base64_md5}

{ASN.1_signature_blob}

--MIME_boundary--
```

### Response Headers

All responses include a seqn (sequence number) header:
```
## seqn = 123456
```

### Data Formats

#### BPSV (Binary Protocol Sequence Variable)

Column format specification:
```
ColumnName!TYPE:size|NextColumn!TYPE:size
```

Types:
- `STRING`: Variable-length string (size 0)
- `HEX`: Hexadecimal data (size in bytes)
- `DEC`: Decimal number (size in bytes)

### Signature Verification (V1)

1. **Extract Signature**:
   - Parse ASN.1 structure from second MIME part
   - Extract SubjectKeyIdentifier (SKI)
   - Extract signature data

2. **Retrieve Certificate**:
   - Use SKI to fetch certificate via `/v1/certs/{ski}`
   - Verify certificate chain

3. **Verify Signature**:
   - Compute SHA-256 of content
   - Verify signature against certificate
   - Check OCSP status if required

### Sequence Numbers

- **Purpose**: Track data freshness and updates
- **Behavior**: Always incrementing
- **Usage**: Cache validation and update detection

### Caching

#### Cache Location
- Windows: `C:\ProgramData\Battle.net\Agent\data\cache`
- macOS: `~/Library/Application Support/Battle.net/Agent/data/cache`
- Linux: `~/.local/share/Battle.net/Agent/data/cache`

#### Cache File Format
```
{command}-{argument}-{seqn}.bmime
```

Example: `v1-products-wow-versions-654321.bmime`

## Implementation Guide

### Connection Flow

```python
# Pseudo-code for Ribbit connection
socket = TCP.connect("us.version.battle.net", 1119)
socket.send("v1/products/wow/versions\r\n")
response = socket.receive_until_complete()
parsed = parse_mime_response(response)
if is_v1:
    verify_signature(parsed)
data = parse_bpsv_data(parsed.content)
```

### Error Handling

| Error | Description | Recovery |
|-------|-------------|----------|
| Connection Refused | Server unavailable | Try alternate region |
| Invalid Signature | Tampered response | Retry request |
| Outdated Seqn | Cached data stale | Request fresh data |
| Malformed Response | Protocol error | Log and retry |

### Best Practices

1. **Connection Management**:
   - Reuse TCP connections when possible
   - Implement connection pooling
   - Handle network interruptions gracefully

2. **Caching Strategy**:
   - Cache responses by seqn
   - Validate cache before use
   - Implement TTL for cached data

3. **Security**:
   - Always verify V1 signatures
   - Validate certificate chains
   - Check OCSP for certificate status

4. **Performance**:
   - Use V2 API when signatures not required
   - Batch multiple requests
   - Implement exponential backoff for retries

## Product Codes

Common Blizzard product codes:

| Product | Code |
|---------|------|
| World of Warcraft | wow, wowt, wow_beta |
| WoW Classic | wow_classic, wow_classic_beta |
| Diablo III | d3, d3t, d3b |
| Diablo IV | fenris, fenrisb |
| Overwatch | pro, prot |
| Hearthstone | hsb, hst |
| Heroes of the Storm | hero, herot |
| StarCraft II | s2, s2t, s2b |
| StarCraft Remastered | s1, s1t |
| Warcraft III Reforged | w3, w3t |

## Protocol Evolution

### Version History

1. **Initial Release**: Basic query protocol
2. **V1 Introduction**: Added signature support
3. **V2 Addition**: Simplified unsigned variant
4. **Current State**: Both V1 and V2 supported

### Future Considerations

- Potential HTTP/2 migration
- Enhanced compression options
- Expanded metadata fields
- GraphQL-style queries

## Troubleshooting

### Common Issues

1. **Empty Responses**: Check product code validity
2. **Signature Failures**: Verify certificate chain
3. **Connection Timeouts**: Try alternate regions
4. **Parsing Errors**: Validate BPSV format

### Debug Tools

```bash
# Test connection
telnet us.version.battle.net 1119

# Send request
echo -e "v1/summary\r\n" | nc us.version.battle.net 1119

# Parse response
ribbit-client --region us --product wow --command versions
```

## Security Considerations

1. **Man-in-the-Middle Protection**: V1 signatures prevent tampering
2. **Certificate Validation**: Always verify certificate chains
3. **Sequence Number Tracking**: Detect replay attacks
4. **TLS Migration**: Future versions may use TLS

## References

- [Battle.net Agent Source](https://github.com/Blizzard/bna)
- [WoWDev Wiki - Ribbit](https://wowdev.wiki/Ribbit)
- [NGDP Overview](https://wowdev.wiki/NGDP)
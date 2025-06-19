# Ribbit Protocol Documentation

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [TCP Communication](#tcp-communication)
  - [Connection Details](#connection-details)
  - [Protocol Format](#protocol-format)
- [Ribbit V1 Commands](#ribbit-v1-commands)
- [Ribbit V2 Commands](#ribbit-v2-commands)
- [Example Commands](#example-commands)
- [Response Format](#response-format)
  - [V1 Responses](#v1-responses)
  - [V2 Responses](#v2-responses)
- [Important Notes](#important-notes)

## Overview

Ribbit is Blizzard's TCP-based protocol for retrieving version information, CDN
configurations, and other metadata. It serves as the successor to the HTTP-based
TACT protocol v1, providing the same data through a different transport mechanism.

## Key Features

### Protocol & Communication

- TCP socket communication (not HTTP)
- Port 1119 connectivity
- Regional version servers
- Direct socket connections

### Versioning Capabilities

- Sequence number tracking
- Never-decreasing sequence numbers
- Update detection mechanism
- Product version retrieval

### Security Features

- ASN.1 base-64 message validation
- PKCS#7/CMS signature verification (MITM protection)
- Timestamped updates (V1)
- Certificate and OCSP support (SHA-1 fingerprints or Subject Key Identifier)

### Caching Support

- Local message caching
- Structured cache filenames
- Persistent cache storage
- Cached in Battle.net Agent data directory

### Message Formats

- V1: MIME format with full metadata
- V2: Simplified content-only format
- SHA-256 checksums (V1)
- Headers, boundaries, and signatures (V1)

## TCP Communication

Unlike TACT's HTTP-based approach, Ribbit uses direct TCP socket connections.

### Connection Details

Server addresses: `{region}.version.battle.net:1119`

Where `{region}` can be:

- `us` - United States
- `eu` - Europe
- `cn` - China (⚠️ See regional restrictions below)
- `kr` - Korea
- `tw` - Taiwan
- `sg` - Singapore

Port: `1119` (TCP)

#### Regional Restrictions

**Important**: The CN (China) region server (`cn.version.battle.net`) is typically
only accessible from within China due to network restrictions and firewall rules.
Attempting to connect from outside China will usually result in connection timeouts.

If you need to access CN region data from outside China, consider:

- Using a VPN with Chinese servers
- Accessing cached data from other sources
- Using alternative regions for testing purposes

### Protocol Format

- Commands are sent as plain text over TCP
- Each command must end with `\n` (LF)
- Commands follow the format: `{version}/{path}`
- No HTTP headers or additional protocol overhead
- Responses vary by protocol version:
  - V1: MIME-formatted with metadata and signatures
  - V2: Raw content only

### Request/Response Flow

1. Open TCP connection to `{region}.version.battle.net:1119`
2. Send command string terminated with `\n`
3. Read response data until connection closes
4. Parse response based on version (MIME for v1, raw TSV for v2)
5. Connection is closed by server after response

## Ribbit V1 Commands

Base format: `v1/{endpoint}`

| Command | Description |
|---------|-------------|
| `v1/summary` | Returns list of all products with sequence numbers and flags |
| `v1/products/{product}/versions` | Returns version information for a product |
| `v1/products/{product}/cdns` | Returns CDN configuration for a product |
| `v1/products/{product}/bgdl` | Returns background downloader information (often empty) |
| `v1/certs/{identifier}` | Returns certificate by SHA-1 fingerprint or Subject Key Identifier (SKI) |
| `v1/ocsp/{identifier}` | Returns certificate revocation status by SHA-1 fingerprint or Subject Key Identifier (SKI) |

## Ribbit V2 Commands

Base format: `v2/{endpoint}`

| Command | Description | Status |
|---------|-------------|--------|
| `v2/summary` | Returns list of endpoints with sequence numbers | Available |
| `v2/products/{product}/versions` | Returns version information | Available |
| `v2/products/{product}/cdns` | Returns CDN configuration | Not Yet Implemented |
| `v2/products/{product}/bgdl` | Returns background downloader information (often empty) | Available |

## Example Commands

```text
v1/products/wow/versions
v1/products/wow_beta/cdns
v2/products/wow/versions
v2/summary
v1/certs/5168ff90af0207753cccd9656462a212b859723b
v1/certs/782a8a710b950421127250a3e91b751ca356e202
```

## Response Format

### V1 Responses

V1 responses use MIME formatting with the following structure:

- MIME-formatted messages (parsed using standard MIME libraries)
- Include headers and boundaries
- Contain ASN.1 base-64 formatted signatures for validation
- Include SHA-256 checksums in epilogue
- Response body contains PSV data (same format as V2)

Example V1 response structure:

```text
MIME-Version: 1.0
Content-Type: multipart/alternative; boundary="..."
Subject: [Response name/type]

--...
Content-Type: text/plain
Content-Disposition: [chunk name]

[PSV data here]
--...
Content-Type: application/octet-stream
Content-Disposition: [signature chunk name]

[ASN.1 signature data]
--...--
Checksum: [64-character SHA-256 hash]
```

#### V1 Response Validation

The response includes a SHA-256 checksum in the epilogue:

- Format: `Checksum: {64-character-hex-hash}`
- Checksum covers all message content except the epilogue itself
- Validation: SHA-256 hash of message bytes (excluding last 76 bytes)

### V2 Responses

V2 responses return raw PSV data without MIME wrapping:

- Raw content only (PSV format)
- No additional metadata or signatures
- Simplified format for easier parsing
- Same data structure as V1 body content

### Certificate Responses

Certificate endpoints (`v1/certs/{identifier}`) return X.509 certificates in PEM
format:

- Standard MIME structure with single content chunk
- Content-Disposition: `cert`
- Certificate in PEM format (base64-encoded DER between BEGIN/END markers)
- Typically contains intermediate CA certificates for CDN verification
- **Accepts both SHA-1 fingerprints and Subject Key Identifiers (SKI)**
- Example SHA-1: `5168ff90af0207753cccd9656462a212b859723b` returns DigiCert SHA2
  High Assurance Server CA
- Example SKI: `782a8a710b950421127250a3e91b751ca356e202` returns CN=version.battle.net
  certificate
- Includes standard checksum validation in epilogue

### OCSP Responses

OCSP endpoints (`v1/ocsp/{identifier}`) return certificate revocation status:

- Standard MIME structure with single content chunk
- Content-Disposition: `ocsp`
- Content-Type: `application/ocsp-response`
- Response is base64-encoded ASN.1 format OCSP response
- Contains certificate validity status and timestamps
- **Accepts both SHA-1 fingerprints and Subject Key Identifiers (SKI)**
- Same identifiers work: `5168ff90af0207753cccd9656462a212b859723b` or `782a8a710b950421127250a3e91b751ca356e202`
- Includes standard checksum validation in epilogue

### PSV Data Format

Both V1 and V2 return data in PSV (Pipe-Separated Values) format with typed columns:

#### Versions Response

```text
Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16
us|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
```

Note: `VersionsName` has inconsistent type casing (`String:0` instead of `STRING:0`).

#### CDNs Response

```text
Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4&fallback=1 https://level3.ssl.blizzard.com/?maxhosts=4&fallback=1|tpr/configs/data
```

Note: The `Servers` column contains full URLs with query parameters for each host.

#### Summary Response

```text
Product!STRING:0|Seqn!DEC:4|Flags!STRING:0
agent|2237058|cdn
agent|3011139|
wow|2241282|cdn
wow|2868866|bgdl
wow|3014093|
```

Note: Flags indicate the type of sequence number - "cdn" for CDN config, "bgdl"
for background download, or empty for versions.

Column types:

- `STRING:0` - String value
- `HEX:16` - 16-byte hex value (MD5 hash)
- `DEC:4` - 4-byte decimal integer

## Caching

Ribbit responses are cached locally:

- **Cache Location**: `C:\ProgramData\Battle.net\Agent\data\cache` (Windows)
- **Cache Filename Format**: `{command}-{arguments}-{sequence_number}.bmime`
- **Examples**:
  - `summary-#-32324.bmime`
  - `cdn-wow_beta-14722.bmime`
  - `versions-wow-52237.bmime`

## Product Identifiers

Common product identifiers used with Ribbit:

### Battle.net Products

- `agent` - Battle.net Agent/Launcher

### World of Warcraft Products

- `wow` - World of Warcraft (Retail)
- `wow_beta` - World of Warcraft Beta
- `wow_classic` - World of Warcraft Classic
- `wow_classic_beta` - World of Warcraft Classic Beta
- `wow_classic_era` - World of Warcraft Classic Era
- `wow_classic_era_ptr` - World of Warcraft Classic Era PTR
- `wow_classic_ptr` - World of Warcraft Classic PTR
- `wowlivetest` - World of Warcraft Live Test
- `wowt` - World of Warcraft Public Test Realm (PTR)
- `wowxptr` - World of Warcraft Experimental PTR
- `wowz` - World of Warcraft Internal/Development

## Implementation Considerations

### Response Handling

1. **Empty Responses**: Some endpoints (particularly bgdl) may return only headers
   without data rows
2. **404 Errors**: Not all products have all endpoint types (e.g., wow_classic_era
   currently has no bgdl)
3. **Column Variations**: Different endpoints have different column sets:
   - Versions includes `KeyRing` and `ProductConfig` columns
   - CDNs includes `Servers` column with full URLs
4. **Type Inconsistencies**: Watch for `VersionsName!String:0` vs standard `STRING:0`

### Regional Considerations

- Standard regions: `us`, `eu`, `cn`, `kr`, `tw`
- Additional regions for some products: `sg` (Singapore)
- The `xx` region appears in data but is not a valid endpoint
- **CN Region Access**: The `cn.version.battle.net` server is restricted to access
  from within China only. Connections from outside China will timeout after ~10 seconds

### Path Patterns

- All WoW products share the same CDN path: `tpr/wow`
- Config path is consistently: `tpr/configs/data`

## Implementation Details

### TCP Client Implementation

1. **Connection Management**:
   - Create new TCP connection for each request
   - No connection pooling or keepalive
   - Server closes connection after sending response
   - Default receive buffer size for reading chunks

2. **Request Format**:
   - ASCII-encoded command string
   - Terminated with `\n` (not `\r\n`)
   - Example: `"v1/products/wow/versions\n"`

3. **Response Reading**:
   - Read in chunks until server closes connection
   - No Content-Length header or delimiter
   - Store complete response in memory for parsing

4. **Error Handling**:
   - No built-in retry mechanism
   - TCP connection errors surface directly
   - Invalid responses throw parsing exceptions
   - No authentication required
   - Connection timeouts recommended (10-30 seconds) to handle unreachable servers

### Response Parsing

1. **MIME Parsing (V1)**:
   - Use standard MIME library (e.g., MimeKit)
   - Extract multipart chunks by Content-Disposition
   - Access data chunk and signature chunk separately
   - Validate SHA-256 checksum in epilogue

2. **PSV/BPSV Parsing**:
   - Pipe-separated values with typed headers
   - First line contains headers with type annotations
   - Sequence number on separate line: `## seqn = {number}`
   - Parse headers by removing type suffix after `!`

## PKI and Signature Verification

### Key Discovery: SKI Usage

Ribbit signatures use Subject Key Identifier (SKI) instead of embedding certificates.
The SKI from PKCS#7/CMS signatures can be used directly with both certificate and
OCSP endpoints:

1. **Extract SKI from Signature**: Parse the PKCS#7 signature to find the SubjectKeyIdentifier
2. **Fetch Certificate**: Use `/v1/certs/{ski}` to retrieve the signer's certificate
3. **Check Status**: Use `/v1/ocsp/{ski}` to verify the certificate isn't revoked
4. **Extract Public Key**: Parse the certificate to get the public key for signature
   verification

### Example Workflow

```text
Signature contains: SubjectKeyIdentifier: 782a8a710b950421127250a3e91b751ca356e202
Certificate endpoint: /v1/certs/782a8a710b950421127250a3e91b751ca356e202
OCSP endpoint: /v1/ocsp/782a8a710b950421127250a3e91b751ca356e202
```

This approach eliminates the need for:

- Certificate stores or bundles
- Complex certificate matching logic
- Manual certificate management

## Important Notes

- Ribbit replaced HTTP-based TACT v1 endpoints as the primary protocol
- Sequence numbers track version changes for each endpoint
- Sequence numbers never decrease - they only increase or stay the same
- The same product identifiers used in TACT work with Ribbit
- As of June 2022, sequence numbers from summary may not match individual endpoints
- Implement connection timeouts (10-30 seconds) to handle unreachable servers
- Each request creates a new connection (no pooling)
- V2 commands return the same data as V1 but without MIME wrapping
- SKI from signatures can be used directly with cert/ocsp endpoints (major discovery)
- **CN region servers are only accessible from within China** - connections from
  other locations will timeout

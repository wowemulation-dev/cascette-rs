# BPSV Format Specification

## Overview

BPSV (Blizzard Pipe-Separated Values) is a structured data format used throughout
Blizzard's NGDP (Next Generation Distribution Pipeline) system. It serves as the
primary data exchange format for:

- 📦 Product version information
- 🌐 CDN configuration data
- 📋 Product summaries and metadata
- 🔧 Build configuration data

## Format Structure

### Basic Structure

```text
FieldName1!TYPE:length|FieldName2!TYPE:length|FieldName3!TYPE:length
## seqn = 12345
value1|value2|value3
value4|value5|value6
```

### Components

1. **Header Line**: Defines field names and types
2. **Sequence Number** (optional): Version tracking
3. **Data Rows**: Pipe-separated values

## Field Types

### STRING:length

String field type for text data.

- **Syntax**: `STRING:length` or `String:length` (case-insensitive)
- **Length**: 0 = unlimited, >0 = maximum character count
- **Examples**:
  - `Region!STRING:0` - Unlimited string
  - `Code!STRING:4` - Max 4 characters

### HEX:length

Hexadecimal field type for hashes and binary data.

- **Syntax**: `HEX:length` or `Hex:length` (case-insensitive)
- **Length**: Number of bytes in binary format (N bytes = N*2 hex characters)
- **Valid characters**: `0-9`, `a-f`, `A-F`
- **Examples**:
  - `Hash!HEX:16` - 16-byte hash = 32 hex characters (MD5)
  - `BuildConfig!HEX:32` - 32-byte hash = 64 hex characters (SHA256)

### DEC:length

Decimal integer field type.

- **Syntax**: `DEC:length`, `Decimal:length`, or `Dec:length` (case-insensitive)
- **Length**: Storage size in bytes (e.g., 4 = uint32, 8 = uint64)
  - Not enforced during parsing - any valid integer is accepted
  - Indicates the intended binary storage format
- **Range**: Full signed 64-bit integer range for parsing
- **Examples**:
  - `BuildId!DEC:4` - Build number (uint32, up to ~4.3 billion)
  - `Seqn!DEC:4` - Sequence number (uint32)

## Special Elements

### Sequence Numbers

Sequence numbers track data versions:

```text
## seqn = 12345
```

- Always on its own line
- Format: `## seqn = NUMBER`
- Used for versioning and cache invalidation

### Empty Values

Empty values are allowed for all field types:

```text
Region!STRING:0|BuildId!DEC:4|Flags!STRING:0
us|12345|cdn
eu||
kr|67890|
```

## Real-World Examples

### Product Versions (wow)

```text
Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!HEX:16
## seqn = 3016450
us|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
eu|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
```

### CDN Configuration

```text
Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
## seqn = 2241282
us|tpr/wow|us.cdn.blizzard.com level3.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4|tpr/configs/data
eu|tpr/wow|eu.cdn.blizzard.com level3.blizzard.com|http://eu.cdn.blizzard.com/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4|tpr/configs/data
```

### Product Summary

```text
Product!STRING:0|Seqn!DEC:7|Flags!STRING:0
## seqn = 3016579
agent|3011139|
agent_beta|1858435|cdn
anbs|2478338|cdn
anbsdev|2475394|cdn
```

## Parsing Rules

### Header Parsing

1. Split by pipe (`|`) character
2. For each field, split by exclamation (`!`)
3. Parse type specification after exclamation mark
4. Extract length from type specification

### Data Row Parsing

1. Split by pipe (`|`) character
2. Number of values must match header field count
3. Apply type validation to each value
4. Empty values are valid for all types

### Type Validation

- **STRING**: Length check if specified
- **HEX**: Valid hex characters, length check
- **DEC**: Valid integer parsing

## Usage in NGDP

BPSV is used by various NGDP endpoints:

### Ribbit Endpoints

- `/v1/products/{product}/versions` - Product version data
- `/v1/products/{product}/cdns` - CDN configuration
- `/v1/products/{product}/bgdl` - Background download info
- `/v1/summary` - All products summary

### TACT Endpoints

All TACT configuration endpoints return BPSV data:

- Version manifests
- CDN configurations
- Build metadata

## Implementation Notes

### Case Sensitivity

- Field names: Case-sensitive
- Field types: Case-insensitive (`STRING`, `String`, `string` are equivalent)
- Values: Preserved as-is

### Character Encoding

- UTF-8 encoding recommended
- ASCII-compatible for maximum compatibility

### Line Endings

- Unix (`\n`) or Windows (`\r\n`) line endings accepted
- Output should use platform-appropriate line endings

### Size Limits

- No hard limits on document size
- Practical limits based on use case (typically < 10MB)
- Row count typically < 10,000 for performance

## Error Conditions

### Invalid Header

- Missing field name or type
- Unknown field type
- Invalid type specification

### Row Validation Errors

- Column count mismatch
- Invalid value for field type
- Value exceeds length constraint

### Sequence Number Errors

- Invalid format
- Non-numeric value

## Best Practices

1. **Always validate field types** when parsing
2. **Preserve empty values** - they have meaning
3. **Use sequence numbers** for versioned data
4. **Keep field names descriptive** but concise
5. **Document field meanings** in code
6. **Handle all error cases** gracefully

## See Also

- [NGDP Overview](https://wowdev.wiki/NGDP)
- [TACT Protocol](https://wowdev.wiki/TACT)
- [Ribbit API Documentation](https://wowdev.wiki/Ribbit)

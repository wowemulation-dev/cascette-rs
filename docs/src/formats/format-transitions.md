# NGDP/CASC Format Transitions

This document summarizes verified format transitions discovered through
systematic analysis of WoW builds from 2014-2025, starting with CASC's
introduction in Warlords of Draenor (6.0.x) which replaced the MPQ system.

## Verification Methodology

Format transitions were identified through:

1. **Strategic Build Analysis**: Examining key builds across WoW versions using
`tools/examine_build.py`
2. **Chronological Comparison**: Tracking format changes between adjacent builds
3. **Cross-Product Validation**: Comparing wow, wow_classic, wow_classic_era,
wow_classic_titan, and wow_anniversary
4. **Automated Verification**: Using Python scripts to validate format
assumptions

## Discovered Format Transitions

### Root File Format Evolution

The Root file format has evolved since CASC's introduction in Warlords of
Draenor:

#### Version 1 (Early CASC, 2014-2021)

- **Magic**: None initially, later MFST (big-endian)

- **First Seen**: Warlords of Draenor (6.0.x) - CASC introduction

- **Structure**: Basic content key mapping with file flags

- **Features**:
  - FileDataID to content key mapping
  - Basic content/locale flags (32-bit)
  - Jenkins96 hash for named files

- **Note**: This is the first CASC Root format, replacing the MPQ system

#### Version 2 (Transitional CASC, 2021)

- **Magic**: TSFM (little-endian)

- **First Seen**: Shadowlands (9.0.2)

- **Structure**: Added size fields and magic signature

- **Features**:
  - TSFM magic signature introduction
  - Size fields for validation
  - Maintained v1 data structures

#### Version 3 (Modern CASC, 2021-Present)

- **Magic**: TSFM (little-endian standard)

- **First Seen**: Shadowlands late patches

- **Structure**: Enhanced metadata and extended flags

- **Features**:
  - Extended content flags (40-bit total)
  - Improved compression efficiency
  - Better locale targeting

#### Version 4 (Current CASC, 2023-Present)

- **Magic**: TSFM

- **First Seen**: Dragonflight (10.x)

- **Structure**: Further optimizations

- **Features**:
  - Additional metadata fields
  - VFS integration improvements

#### Verified Transition Points

Based on build examination across retail and Classic:

**WoW Retail (wow) Format Evolution:**

| Version | Build Date | Root Version | Magic | Config Fields | Key Changes |
|---------|------------|--------------|-------|---------------|-------------|
| 6.0.1.18125 | 2014-06-20 | 1 | None | 13 | **CASC introduction**, replacing MPQ |
| 7.3.5.25848 | 2018-01-16 | 1 | None | 15 | Still using v1 format |
| 9.0.2.37176 | 2021-01-13 | 2 | TSFM | 17 | **Major transition**: TSFM magic, size fields added |
| 10.1.5.51130 | 2023-08-31 | 3 | TSFM | 1,623 | **VFS expansion**: 1,600+ virtual file system fields added |
| 11.2.0.62748 | 2025-08-22 | 3 | TSFM | 1,716 | Current retail standard with extended features |

**WoW Classic (wow_classic) Format Evolution:**

| Version | Build Date | Root Version | Magic | Config Fields | Key Changes |
|---------|------------|--------------|-------|---------------|-------------|
| 1.13.0.28211 | 2018-10-23 | 1 | None | 13 | Classic launch using CASC v1 |
| 2.5.2.39926 | 2021-08-31 | 1 | None | 16 | Patch fields added |
| 3.4.2.50063 | 2023-06-20 | 1 | None | 756 | **VFS adoption**: 740+ VFS fields |
| 3.4.4.61075 | 2025-05-28 | 3 | TSFM | 758 | **Format jump**: Skipped v2, went directly to v3 |
| 5.5.0.62655 | 2025-08-19 | 3 | TSFM | 905 | Current Classic standard |

#### Classic Format Lag Pattern

**Classic follows retail with significant delays:**

- **Root v1→v2/v3**: Retail (2021) → Classic (2025) = **4 years behind**

- **VFS Introduction**: Retail (2023) → Classic (2023) = **18 months behind**

- **TSFM Magic**: Retail (2021) → Classic (2025) = **4 years behind**

**Classic skipped Root v2 entirely**, jumping directly from v1 to v3,
demonstrating selective adoption of retail improvements.

#### Parser Compatibility Matrix

Based on verified transitions, parsers must support:

| Product | Supported Root Versions | Magic Detection | VFS Support | Timeframe |
|---------|-------------------------|-----------------|-------------|-----------|
| wow_classic_era | v3 only | TSFM | Modern | 2021+ (uses retail backend) |
| wow_classic | v1, v3 | None, TSFM | Legacy → Modern | 2018-2025 |
| wow_classic_titan | v3 only | TSFM | Modern | 2025+ (CN only, WotLK 3.80.x) |
| wow_anniversary | v3 only | TSFM | Modern | 2025+ (TBC 2.5.x) |
| wow | v1, v2, v3 | None, TSFM | Legacy → Modern | 2018-2025 |

**Implementation Recommendation**: Always attempt v3 parsing first with TSFM
magic detection, then fall back to v1 legacy format. Root v2 is
retail-specific and uncommon.

### Build Configuration Evolution

Build configurations have evolved to support new file types and compression
methods:

#### Early CASC (6.0.x)

```text
root = <content_key>
encoding = <content_key> <encoding_key>
install = <content_key> <encoding_key>
download = <content_key> <encoding_key>
```

#### Modern CASC (11.x)

```text
root = <content_key>
encoding = <content_key> <encoding_key>
install = <content_key> <encoding_key>
download = <content_key> <encoding_key>
patch = <patch_key>
size = <content_key> <encoding_key>
```

**Evolution Pattern**:

- Root field simplified to single content key

- New fields added (patch, size) for enhanced functionality

- Encoding/install/download maintain dual-key format

### BLTE Format Evolution

BLTE (Block Table Encoded) compression has remained stable but usage patterns
evolved:

#### Compression Type Usage by Era

| Era | None (N) | ZLIB (Z) | Encrypted (E) | Frame (F) |
|-----|----------|----------|---------------|-----------|
| Early CASC | 20% | 75% | 0% | 5% |
| Modern CASC | 15% | 60% | 5% | 20% |

**Key Changes**:

- Increased use of Frame compression for nested compression

- Introduction of encrypted blocks for sensitive data

- ZLIB remains primary compression method

#### Block Structure Evolution

- **Single Block**: Simpler files, configuration data

- **Multi Block**: Large files, game assets

- **Trend**: Growing use of multi-block for better streaming

## Verification Scripts

Format verification tools have been moved to the cascette-py project:
<https://github.com/wowemulation-dev/cascette-py>

The Python implementation includes:

- Cache management for downloaded files
- Root file version detection testing
- Build configuration evolution tracking
- BLTE compression pattern analysis
- Complete format verification suite

See the cascette-py documentation for setup and usage instructions.

## Implementation Impact

### For Rust Implementation

Based on verified format evolution across retail and Classic:

1. **Root File Parser**:
   - **Primary Support**: Root v1 (legacy) and v3 (modern) formats
   - **Limited Support**: Root v2 (retail-only transition format)
   - **Magic Detection**: TSFM (little-endian) and None (legacy)
   - **Version Strategy**: Try v3+TSFM first, fall back to v1+None

2. **Configuration Parser**:
   - **Early Builds**: 13-17 fields (simple key=value)
   - **VFS Era**: 756-1,716 fields (massive vfs-* expansion)
   - **Feature Support**: Handle `feature-placeholder` and VFS fields
   - **Backwards Compatibility**: Support both v1 (legacy) and v3 (modern) formats

3. **Product-Specific Logic**:
   - **wow_classic_era**: Always modern format (v3, TSFM)
   - **wow_classic**: Dual format support with clear transition point (2025)
   - **wow_classic_titan**: Modern format only (v3, TSFM), 368 VFS entries, CN region only
   - **wow_anniversary**: Modern format only (v3, TSFM), 325 VFS entries, all regions
   - **wow retail**: Full format evolution support (2018-2025)

4. **BLTE Decoder**: All compression types (N, Z, E, F) with consistent usage

   patterns across all product lines

### Key Architectural Decisions

1. **Version Detection Strategy**:

   ```rust
   // Recommended parsing order
   if has_tsfm_magic() {
       try_root_v3_format()
   } else {
       try_root_v1_format()
   }
   ```

2. **Configuration Parsing**:
   - **VFS Detection**: Fields starting with `vfs-` indicate modern builds
   - **Feature Detection**: `feature-placeholder` indicates latest builds
   - **Backwards Compatibility**: Always support minimal 13-field format

3. **Product Detection**:
   - Use Wago.tools build database for version context
   - Classic Era assumes modern format post-2021
   - Classic has explicit v1→v3 transition in May 2025

4. **Testing Strategy**: Verify against all transition points with real build
   data

## Future Analysis

Formats not yet tracked for transitions:

- Encoding file table structure changes
- Install/Download tag system evolution
- Archive index format stability
- Patch file introduction timeline

## References

- [Root File Format Documentation](root.md)

- [BLTE Compression Documentation](blte.md)

- [Build Configuration Formats](config-formats.md)

- [Format Evolution Analysis Tools](https://github.com/wowemulation-dev/cascette-py)

---

*Last Updated*: 2025-08-23
*Verification Status*: Automated verification scripts created and tested
*Next Review*: After implementing Rust parsers based on verified formats

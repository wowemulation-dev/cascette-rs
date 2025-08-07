# NGDP Research Documentation

This directory contains detailed technical documentation for Blizzard's Next Generation Distribution Pipeline (NGDP) and its components.

## Documentation Structure

### Core Components

1. **[NGDP Architecture Overview](ngdp-architecture-overview.md)**
   - High-level system architecture
   - Component relationships
   - Technology stack overview

2. **[Ribbit Protocol](ribbit-protocol-detailed.md)**
   - API communication protocol
   - Server endpoints and commands
   - Response formats and signatures

3. **[TACT System](tact-system.md)**
   - Transfer And Content Transfer protocol
   - File encoding and compression
   - Content addressing scheme

4. **[CASC Storage](casc-storage.md)**
   - Content Addressable Storage Container
   - Local file organization
   - Index and archive formats

5. **[CDN Infrastructure](cdn-infrastructure.md)**
   - Content delivery network
   - File distribution patterns
   - CDN selection and fallback

### Workflows

6. **[Installation Process](installation-process.md)**
   - Initial game installation
   - File download workflow
   - Verification procedures

7. **[Update Process](update-process.md)**
   - Patch detection and download
   - Delta patching
   - Background downloads

### Technical Specifications

8. **[BLTE Format Detailed](blte-format-detailed.md)** 📄 **NEW**
   - Complete BLTE binary specification
   - All compression modes (N, Z, 4, F, E)
   - Encryption integration
   - Implementation guidelines

9. **[File Formats Detailed](file-formats-detailed.md)** 📄 **NEW**
   - Exact binary structures for all TACT files
   - Encoding, Root, Install, Download, Size files
   - Parsing algorithms and utilities

10. **[TVFS Specification](tvfs-specification.md)** 📄 **NEW**
    - TACT Virtual File System complete spec
    - Path table, VFS table, CFT structures
    - Modern manifest format

11. **[Encryption and Security](encryption-security.md)** 📄 **NEW**
    - Salsa20 and ARC4 implementations
    - Key management architecture
    - Signature verification details

12. **[Algorithms and Formulas](algorithms-formulas.md)** 📄 **NEW**
    - Jenkins hash (Lookup3) complete implementation
    - XOR bucket calculation
    - Compression algorithms
    - Performance optimizations

### Project Analysis

13. **[Project Implementation Map](project-implementation-map.md)** 📄 **UPDATED**
    - How cascette-rs crates map to NGDP components
    - Current implementation status
    - Architecture and data flow

14. **[Implementation Gaps](implementation-gaps.md)** 📄 **NEW**
    - Critical missing components
    - Priority roadmap for completion
    - Risk assessment and recommendations

## Quick Reference

### Key Technologies

- **NGDP**: Next Generation Distribution Pipeline
- **TACT**: Transfer And Content Transfer
- **CASC**: Content Addressable Storage Container
- **TVFS**: TACT Virtual File System
- **BLTE**: Block Table Encoded compression
- **BPSV**: Binary Protocol Sequence Variable

### Important Endpoints

- Ribbit API: `{region}.version.battle.net:1119`
- CDN: `http://{cdn-host}/{cdn-path}/`
- Wago Tools API: `https://wago.tools/api/`

## External References

- [WoWDev Wiki - NGDP](https://wowdev.wiki/NGDP)
- [WoWDev Wiki - TACT](https://wowdev.wiki/TACT)
- [WoWDev Wiki - CASC](https://wowdev.wiki/CASC)
- [WoWDev Wiki - Ribbit](https://wowdev.wiki/Ribbit)
- [Wago Tools API](https://wago.tools/apis)
# Cascette-RS Roadmap

## Project Vision

Cascette-RS aims to be a complete open-source replacement for Blizzard's NGDP (Next Generation Distribution Pipeline), capable of both reading and writing all NGDP formats to enable custom game content distribution.

## Completed Milestones ✅

### Phase 1: Foundation (v0.1.0 - v0.3.1) ✅

#### Core Infrastructure ✅
- **BPSV Parser & Writer** - Complete Blizzard Pipe-Separated Values support
- **Ribbit Client** - TCP protocol client with MIME parsing and signature verification
- **TACT Client** - HTTP/HTTPS client with retry logic and range requests
- **CDN Client** - Content delivery with fallback support
- **Cache System** - Generic caching with TTL management

#### Encryption & Compression ✅
- **BLTE Decompression** - All modes (N, Z, 4, F, E) with streaming support
- **BLTE Compression** - Full compression with encryption support
- **Key Service** - 19,000+ encryption keys with auto-loading
- **Salsa20/ARC4** - Complete encryption/decryption support

#### TACT Format Parsers ✅
- **Encoding File** - CKey ↔ EKey mapping with 40-bit integers
- **Install Manifest** - Tag-based file filtering
- **Download Manifest** - Priority-based downloads
- **Build/CDN Config** - Configuration parsing
- **Size File** - Installation size calculations
- **TVFS** - Virtual file system support
- **Root File** - FileDataID to CKey mapping

#### Local Storage ✅
- **CASC Storage** - Complete local game file management
- **Index Parsing** - .idx and .index file support
- **Archive Reader** - Memory-mapped archive access
- **Loose Files** - Individual file support
- **Verification** - Integrity checking and repair

#### CLI Tools ✅
- **Products Commands** - Query versions, CDNs, builds
- **Download Commands** - File downloads with resume support
- **Inspect Commands** - Visual data structure inspection
- **Storage Commands** - Local storage management
- **Keys Management** - Encryption key updates

### Phase 2: Installation Support (Current) 🟡

#### Client Installation ✅
- **Install Command** - Complete game client installation
- **Resume Support** - Interrupted download recovery
- **Repair Command** - Installation verification and fixing
- **.build.info Generation** - Client restoration support
- **HTTP-First Discovery** - Modern version discovery

## In Progress 🟡

### Write Support Implementation
- TACT format writers (Encoding, Install, Download, Size, Config, TVFS, Root)
- BPSV writer implementation
- CASC index writers (.idx, .index)
- Key generation service
- FileDataID management system

## Roadmap

### Phase 3: Content Creation (Q1 2025) 🔴

#### Content Management System
- **Asset Pipeline** - Convert industry formats to game formats
- **Database Management** - DBC/DB2 file handling
- **Version Control** - Branch management for PTR/Beta/Live
- **Tool Integrations** - Blender, Maya, level editors

#### Build System
- **NGDP Builder** - Package content into NGDP format
- **Manifest Generation** - Create all required manifests
- **Archive Creation** - Build CASC archives
- **Configuration Generation** - BuildConfig, CDNConfig

### Phase 4: Distribution (Q2 2025) 🔴

#### Server Implementation
- **Ribbit Server** - Central build orchestrator
- **CDN Server** - Content distribution endpoints
- **TACT HTTP Proxy** - HTTP API to Ribbit bridge
- **Build Distribution** - Push builds to CDN nodes

#### Advanced Features
- **Delta Patching** - Incremental updates
- **P2P Support** - Peer-to-peer distribution
- **Load Balancing** - Multi-CDN management
- **Monitoring** - Distribution metrics

### Phase 5: Production Ready (Q3 2025) 🔴

#### Enterprise Features
- **High Availability** - Redundancy and failover
- **Scalability** - Horizontal scaling support
- **Security** - Authentication and encryption
- **Compliance** - Audit logs and access control

#### Community Tools
- **Web Interface** - Browser-based management
- **API Documentation** - OpenAPI specifications
- **SDK Support** - Language bindings
- **Migration Tools** - Import from existing systems

## Success Metrics

### Technical Goals
- ✅ Parse all NGDP formats
- ✅ Local storage management
- ✅ Game client installation
- 🔴 Generate valid NGDP builds
- 🔴 Serve content to Battle.net clients
- 🔴 Support custom content creation

### Performance Targets
- ✅ Sub-second file lookups
- ✅ Streaming for large files
- ✅ Parallel downloads
- 🔴 10GB+ build generation
- 🔴 1000+ concurrent clients
- 🔴 99.9% uptime

### Community Adoption
- ✅ Comprehensive documentation
- ✅ Example programs
- ✅ CLI tools
- 🔴 GUI applications
- 🔴 Docker containers
- 🔴 Kubernetes operators

## Version History

### Released
- **v0.1.0** (2025-06-28) - Initial release with core functionality
- **v0.2.0** (2025-08-07) - Streaming, HTTP range requests, complete parsers
- **v0.3.0** (2025-08-06) - Ephemeral signing, installation improvements
- **v0.3.1** (2025-08-07) - Bug fixes and documentation updates

### Upcoming
- **v0.4.0** - Write support for all formats
- **v0.5.0** - Content management system
- **v0.6.0** - Build generation
- **v0.7.0** - Server implementation
- **v1.0.0** - Production ready

## Contributing

We welcome contributions! Priority areas:
1. Write support for TACT formats
2. Content management tools
3. Server implementation
4. Documentation improvements
5. Test coverage expansion

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
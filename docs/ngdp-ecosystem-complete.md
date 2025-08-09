# Complete NGDP Ecosystem Architecture

## Executive Summary

This document provides a comprehensive overview of the complete NGDP (Next Generation Distribution Pipeline) ecosystem, from content creation through distribution to client installation. It covers both the existing Blizzard implementation and the requirements for a complete replacement system.

## System Architecture Overview

```
Creative Pipeline → Content Management → Build System → Distribution → Game Clients
```

### Complete Flow Diagram

```
Creative Tools:                    Content Pipeline:                Distribution:

Blender/Maya ──┐
Level Editor ──┤
Quest Designer ─┼→ Content Manager → NGDP Builder → Ribbit Server → CDN Servers → Game Clients
Game Designer ──┤        ↓                               ↓                            ↓
Sound Tools ────┘   Version Control                TACT HTTP API              Battle.net Client
                         ↓                           (proxy)                        ↓
                    Database (DBC/DB2)                                         Local CASC
                    Asset Repository                                           Storage
                    Build Manifests
```

## Layer 1: Creative Tools Integration

### Purpose

Enable content creators to work with industry-standard tools while automatically converting to game-specific formats.

### Components

#### 3D Asset Pipeline

- **Input**: Blender, Maya, 3DS Max, ZBrush
- **Formats**: FBX, OBJ, DAE, USD
- **Output**: M2 (models), WMO (world models), ADT (terrain)
- **Processing**:
  - UV mapping validation
  - Bone limit enforcement
  - LOD generation
  - Collision mesh creation
  - Texture reference mapping

#### Texture Pipeline

- **Input**: Photoshop, Substance Painter, GIMP
- **Formats**: PNG, TGA, TIF, PSD
- **Output**: BLP (Blizzard Picture)
- **Processing**:
  - Mipmap generation
  - Compression selection (DXT1/3/5)
  - Alpha channel handling
  - Power-of-two sizing

#### Map/Level Design

- **Input**: Custom level editors, Noggit, terrain tools
- **Output**: ADT tiles, WDT world definitions
- **Features**:
  - Heightmap editing
  - Texture painting
  - Object placement
  - Liquid volumes
  - Navigation mesh generation

#### Game Data Design

- **Input**: Spreadsheets, custom tools, web interfaces
- **Output**: DBC/DB2 database files
- **Content**:
  - Items, spells, creatures
  - Quests, achievements
  - Game mechanics data
  - Localization strings

## Layer 2: Content Management System

### Purpose

Central repository for all game content with version control, validation, and build preparation.

### Core Components

#### Asset Repository

```rust
struct AssetRepository {
    models: HashMap<AssetId, Model>,
    textures: HashMap<AssetId, Texture>,
    sounds: HashMap<AssetId, Sound>,
    maps: HashMap<MapId, MapData>,
    dependencies: DependencyGraph,
}
```

**Features**:

- Content-addressable storage
- Dependency tracking
- Asset versioning
- Metadata management
- Search and query capabilities

#### Database Management

```rust
struct DatabaseManager {
    tables: HashMap<String, DBCTable>,
    schemas: HashMap<String, TableSchema>,
    relations: ForeignKeyMap,
    validators: ValidationRules,
}
```

**Capabilities**:

- DBC/DB2 parsing and generation
- Schema validation
- Referential integrity
- Bulk import/export
- Query interface

#### Version Control System

```rust
struct ContentVersionControl {
    branches: HashMap<String, Branch>,  // PTR, Beta, Live
    commits: CommitHistory,
    tags: HashMap<String, Tag>,
    merge_engine: MergeStrategy,
}
```

**Features**:

- Branching for different environments
- Atomic commits
- Conflict resolution
- Rollback capabilities
- Diff generation

#### Validation System

- Asset format validation
- Size and performance checks
- Reference integrity
- Naming convention enforcement
- Content policy compliance

## Layer 3: Build System (NGDP Builder)

### Purpose

Package validated content into NGDP-compatible format for distribution.

### Build Process

#### 1. Content Collection

```rust
impl ProductBuilder {
    async fn collect_content(&mut self) -> Result<ContentPackage> {
        // Pull from content manager
        let assets = self.content_manager.export_for_build(filter)?;
        let databases = self.database_manager.export_all()?;
        let manifests = self.generate_manifests()?;
        Ok(ContentPackage { assets, databases, manifests })
    }
}
```

#### 2. File Processing

- Calculate content hashes (CKey)
- Apply BLTE compression
- Generate encryption keys for sensitive content
- Create encoding mappings (CKey → EKey)
- Assign FileDataIDs

#### 3. Manifest Generation

**Root Manifest**: Maps FileDataID → CKey

```
Structure:
- Header with locale/platform flags
- FDID to CKey mappings
- Filename hash lookups
```

**Encoding File**: Maps CKey → EKey

```
Structure:
- Compression specifications
- CKey to EKey mappings
- File size information
```

**Install Manifest**: Installation file list

```
Structure:
- Required files for base install
- Installation priorities
- Tag-based filtering
```

**Download Manifest**: Update/patch files

```
Structure:
- Patch-specific files
- Download priorities
- Size requirements
```

#### 4. Archive Creation

- Package files into data.XXX archives
- Generate index files (.idx)
- Create CDN-ready structure
- Calculate all checksums

#### 5. Configuration Generation

- BuildConfig: References to all manifests
- CDNConfig: Archive information
- ProductConfig: Product metadata
- VersionsName: Human-readable version

## Layer 4: Distribution System

### Ribbit Server - Central Orchestrator

#### Purpose

Central point for build registration, version management, and CDN distribution.

#### Architecture

```rust
struct RibbitServer {
    // Build Management
    build_registry: BuildRegistry,
    build_queue: BuildQueue,

    // Version Control
    version_manager: VersionManager,
    region_configs: HashMap<Region, Config>,

    // Distribution
    cdn_distributor: CdnDistributor,
    cdn_endpoints: Vec<CdnEndpoint>,

    // API Handlers
    ribbit_handler: RibbitProtocolHandler,
    http_proxy: TactHttpProxy,
}
```

#### Responsibilities

**Build Ingestion**:

1. Receive build from NGDP Builder
2. Validate build integrity
3. Stage for distribution
4. Generate distribution metadata

**CDN Distribution**:

1. Push builds to CDN nodes
2. Verify successful replication
3. Handle node failures
4. Monitor distribution status

**Version Management**:

1. Track versions per product/region
2. Coordinate staged rollouts
3. Support rollback operations
4. Manage version promotion (PTR → Beta → Live)

**Client Services**:

1. Ribbit protocol responses
2. HTTP API via proxy
3. Version queries
4. CDN endpoint listing

### CDN Servers - Content Delivery

#### Purpose

Serve game content to clients with high availability and performance.

#### Architecture

```rust
struct CdnServer {
    // Storage
    storage_path: PathBuf,
    archive_manager: ArchiveManager,

    // Build Reception
    build_receiver: BuildReceiver,
    staging_area: StagingArea,

    // Serving
    http_server: HttpServer,
    cache_layer: CacheLayer,

    // Monitoring
    health_monitor: HealthMonitor,
    metrics: Metrics,
}
```

#### Features

**Build Reception**:

- Accept pushes from Ribbit
- Validate received content
- Stage before going live
- Atomic promotion to production

**Content Serving**:

- HTTP/HTTPS endpoints
- Range request support
- Path structure: `/data/{hash[0:2]}/{hash[2:4]}/{hash}`
- Compression support
- Cache headers

**High Availability**:

- Geographic distribution
- Load balancing
- Failover support
- Health monitoring

### TACT HTTP API Proxy

#### Purpose

Provide HTTP/JSON interface to Ribbit data for modern clients.

#### Implementation

```rust
impl TactApiProxy {
    async fn handle_request(&self, path: &str) -> Response {
        // Convert HTTP path to Ribbit command
        let ribbit_cmd = self.path_to_ribbit(path)?;

        // Query Ribbit server
        let ribbit_response = self.ribbit.query(ribbit_cmd).await?;

        // Transform to JSON
        let json = self.ribbit_to_json(ribbit_response)?;

        // Return with caching headers
        Response::json(json).with_cache_headers()
    }
}
```

## Layer 5: Client Systems

### Battle.net Client Integration

- Product discovery via Ribbit
- Version checking and updates
- Download orchestration
- Installation management
- Game launching

### CASC Local Storage

- Archive management
- Index maintenance
- File extraction
- Repair functionality
- Space optimization

## Security Considerations

### Content Protection

- Encryption for sensitive assets
- Key management system
- Secure key distribution
- Anti-tampering measures

### Distribution Security

- HTTPS for all transfers
- Certificate pinning
- Signature verification
- Checksum validation

### Access Control

- Authentication for content upload
- Regional restrictions
- Beta access management
- Development environment isolation

## Scalability Architecture

### Horizontal Scaling

- Multiple Ribbit servers with synchronization
- CDN node auto-scaling
- Load balancer distribution
- Database replication

### Performance Optimization

- Content deduplication
- Delta patching
- Compression optimization
- Parallel downloads
- Caching at all layers

### Monitoring and Operations

- Build pipeline monitoring
- Distribution metrics
- Client download analytics
- Error tracking and alerting

## Implementation Roadmap

### Phase 1: Core Infrastructure

1. Content Manager implementation
2. Basic NGDP Builder
3. Simple Ribbit server
4. Single CDN node

### Phase 2: Tool Integration

1. Blender addon development
2. Database editor tools
3. Web-based content portal
4. Version control integration

### Phase 3: Distribution Network

1. Multi-region Ribbit servers
2. CDN replication system
3. Load balancing
4. Monitoring dashboard

### Phase 4: Advanced Features

1. Delta patch generation
2. P2P distribution support
3. Advanced caching strategies
4. A/B testing capabilities

## Success Metrics

### Build System

- Build generation time < 30 minutes
- Zero failed builds due to system errors
- 100% content validation coverage

### Distribution

- CDN availability > 99.9%
- Download speeds > 10MB/s average
- Successful installation rate > 99%

### Content Pipeline

- Asset import success rate > 95%
- Validation catch rate > 99%
- Version control merge success > 90%

## Conclusion

A complete NGDP replacement requires not just the distribution infrastructure, but a comprehensive content pipeline from creation tools through to client delivery. The system must support:

1. **Creation**: Integration with industry-standard tools
2. **Management**: Version control and validation
3. **Building**: NGDP-compliant package generation
4. **Distribution**: Scalable, reliable content delivery
5. **Installation**: Client-side storage and management

This architecture enables full control over the game content lifecycle while maintaining compatibility with existing Battle.net clients and infrastructure.

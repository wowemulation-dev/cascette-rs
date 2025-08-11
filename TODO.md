# Cascette-RS TODO

> For completed work, see [ROADMAP.md](ROADMAP.md)

## Current Priorities

> **Note:** All critical format parsing gaps have been completed. See [ROADMAP.md](ROADMAP.md) for details.

## Priority 1: Write Support 🔴

## Priority 2: Architecture Improvements 🟡

### Dependency Injection for Testing

**Rationale:** Enable proper unit testing with mocked dependencies.

- [ ] HTTP Client trait abstraction (`ngdp-cdn/src/traits.rs`)
- [ ] Storage Backend trait for caching (`ngdp-cache/src/traits.rs`)  
- [ ] Time Provider abstraction for TTL testing (`ngdp-cache/src/time.rs`)
- [ ] Update all clients to use trait bounds

### Trait Abstractions

- [ ] Content Provider traits for data sources (`ngdp-cdn/src/providers.rs`)
- [ ] Parser/Writer traits for format-agnostic operations (`tact-parser/src/traits.rs`)
- [ ] Compression Strategy traits (`blte/src/traits.rs`)

### Metrics and Observability

- [ ] Metrics infrastructure (`ngdp-metrics/` crate)
- [ ] CDN, Cache, Parser, and System metrics
- [ ] Prometheus/StatsD exporters

### Configuration Management

- [ ] Unified configuration schema (`ngdp-config/` crate)
- [ ] TOML/YAML/JSON support with environment overlays
- [ ] Configuration CLI commands

### Testing Infrastructure

- [ ] Shared test fixtures and mock servers (`test-fixtures/`)
- [ ] Property-based testing with `proptest`
- [ ] Fuzz testing for network protocols

### TACT Format Writers

- [ ] Encoding File Writer (`tact-parser/src/encoding/writer.rs`)
- [ ] Install Manifest Writer (`tact-parser/src/install/writer.rs`)
- [ ] Download Manifest Writer (`tact-parser/src/download/writer.rs`)
- [ ] Size File Writer (`tact-parser/src/size/writer.rs`)
- [ ] Config File Writer (`tact-parser/src/config/writer.rs`)
- [ ] TVFS Writer (`tact-parser/src/tvfs/writer.rs`)
- [ ] Root Manifest Writer (`tact-parser/src/root/writer.rs`)

### Support Components

- [ ] BPSV Writer (`ngdp-bpsv/src/writer.rs`)
- [ ] Key Service Extensions (`ngdp-crypto/src/key_service.rs`)
- [ ] FileDataID Manager (`tact-parser/src/fdid_manager.rs`)
- [ ] CASC Index Writers (`casc-storage/src/index/writers/`)

## Priority 3: Content Management System 🟡

### Core Components

- [ ] Asset Pipeline (`content-manager/src/pipeline.rs`)
  - Model importers, texture converters, audio processors
- [ ] Database Management (`content-manager/src/database.rs`)
  - DBC/DB2 parsing, schema validation, query engine
- [ ] Version Control (`content-manager/src/versioning.rs`)
  - Branch management, atomic commits, change tracking

### Tool Integrations

- [ ] Blender Integration (`content-manager/src/integrations/blender.rs`)
- [ ] Quest Designer Integration (`content-manager/src/integrations/quest.rs`)
- [ ] Map Editor Integration (`content-manager/src/integrations/map.rs`)

## Priority 4: Build System 🟡

### NGDP Builder (`ngdp-builder/`)

- [ ] Product Builder Core - Generate CKeys, EKeys, manifests
- [ ] Content Processing Pipeline - Compression, encryption, checksums
- [ ] CASC Storage Generator - Archives, indices, build info

## Priority 5: Server Implementation 🟡

### Server Components

- [ ] Ribbit Server (`ngdp-server/src/ribbit.rs`)
  - Build management, protocol handling, orchestration
- [ ] CDN Server (`ngdp-server/src/cdn.rs`)
  - Content serving, caching, load balancing
- [ ] TACT HTTP Proxy (`ngdp-server/src/tact_proxy.rs`)
  - HTTP to Ribbit bridge, JSON responses

## Priority 6: CLI UX Improvements 🔵

### Pipe Handling

- [ ] Graceful broken pipe handling (`ngdp-client/src/main.rs`)
- [ ] Signal handling for Unix/Windows (`ngdp-client/src/signals.rs`)
- [ ] Progress indicators and streaming output (`ngdp-client/src/output.rs`)

## Priority 7: Advanced Features 🔵

### BLTE Enhancements

- [ ] Parallel compression (`blte/src/parallel.rs`)
- [ ] Write trait implementation (`blte/src/writer.rs`)

### CLI Enhancements

- [ ] Compression commands (`ngdp-client/src/commands/compress.rs`)
- [ ] Build commands (`ngdp-client/src/commands/build.rs`)

### Research Areas 🔍

- [ ] Build System Research (asset conversion, FileDataID algorithms)
- [ ] CDN Infrastructure Research (distribution, caching, geographic patterns)
- [ ] Protocol Analysis (Battle.net communication, update detection)

## Testing & Documentation

### Testing

- [ ] Unit tests for writers and components
- [ ] Integration tests for round-trip operations
- [ ] Performance tests for large files and concurrent clients

### Documentation

- [ ] API documentation for writer interfaces
- [ ] User guides for content creation and deployment
- [ ] Examples for each TACT format and workflow

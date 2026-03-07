# Battle.net Agent

The Battle.net Agent is a local HTTP service that manages game installations and
updates. It runs on port 1120 and provides an API for downloading, installing,
and managing Blizzard products.

## Overview

The agent serves as the bridge between Blizzard's CDN infrastructure and the
local CASC storage. It handles:

- Product installation and updates
- Download management and prioritization
- Local CASC storage maintenance
- Installation verification and repair

## HTTP API

The agent exposes a REST API on `http://127.0.0.1:1120`. All responses use
`application/json` unless noted otherwise. The default port is 1120, with
fallback ports 6881-6883 if 1120 is unavailable.

### Agent Info

```text
GET /agent
```

Returns agent metadata and an authorization token for launcher communication.

**Response:**

```json
{
  "agent_version": "0.2.0",
  "authorization": "...",
  "allow_commands": false,
  "port": 1120,
  "uptime_seconds": 3600
}
```

### Version

```text
GET /version
```

Returns version information.

**Response:**

```json
{
  "agent_version": "0.2.0",
  "product_version": "0.2.0",
  "build_date": "2025-01-01"
}
```

### Hardware

```text
GET /hardware
```

Returns hardware information (flat fields matching the `Hardware` protobuf message).

**Response:**

```json
{
  "cpu_arch": 1,
  "cpu_num_cores": 16,
  "cpu_speed": 4500,
  "memory": 34359738368,
  "num_gpus": 1,
  "gpu_1": {
    "vendor_id": 0,
    "device_id": 0,
    "shared_memory": 0,
    "video_memory": 0,
    "system_memory": 0,
    "integrated": false,
    "name": "AMD Radeon RX 7900 XTX"
  },
  "gpu_2": {},
  "gpu_3": {},
  "cpu_vendor": "",
  "cpu_brand": "AMD Ryzen 9 7950X"
}
```

`cpu_arch`: `0` = x86, `1` = x86_64. `memory` is total RAM in bytes.
`cpu_speed` is in MHz. Up to three GPU sub-messages are present (`gpu_1`,
`gpu_2`, `gpu_3`); unused slots are empty objects.

### Health Check

```text
GET /health
```

Cascette extension. Returns service health status.

**Response:**

```json
{
  "status": "ok",
  "version": "0.2.0",
  "uptime_seconds": 3600
}
```

### Metrics

```text
GET /metrics
```

Cascette extension. Returns Prometheus text format metrics. Response content type
is `text/plain`.

## Product Endpoints

### List Products

```text
GET /game
```

Returns all registered products.

**Response:**

```json
[
  {
    "result_uri": "/game/wow_classic",
    "uid": "wow_classic",
    "region": "us",
    "product_code": "wow_classic",
    "install_dir": "/opt/games/wow_classic",
    "subpath": null,
    "conflict_install_dir": null
  }
]
```

`subpath` and `conflict_install_dir` are omitted when null.

### Get Product

```text
GET /game/{product}
```

Returns details for a single product.

**Response:**

```json
{
  "result_uri": "/game/wow_classic",
  "uid": "wow_classic",
  "region": "us",
  "product_code": "wow_classic",
  "install_dir": "/opt/games/wow_classic"
}
```

Returns HTTP 400 with `{"error": 2312}` if the product UID is not found.

### Size Estimate

```text
GET /size_estimate?uid={product}
```

Returns estimated installation size for a product.

**Response:**

```json
{
  "uid": "wow",
  "estimated_bytes": 107374182400,
  "status": "available"
}
```

## Operation Endpoints

All operation endpoints accept a JSON body and return HTTP 200 on success.

### Install

```text
POST /install/{product}
```

Starts a product installation. The product must not already be installed.

**Request:**

```json
{
  "uid": "wow",
  "priority": 700,
  "install_path": "/opt/games/wow",
  "region": "us",
  "locale": "enUS"
}
```

All fields except `uid` are optional. `priority` defaults to 700.

**Response:**

```json
{
  "response_uri": "/install/wow",
  "result_uri": "/install/wow",
  "uid": "wow",
  "priority": 700
}
```

### Update

```text
POST /update/{product}
```

Starts a product update. The product must be installed.

**Request:**

```json
{
  "uid": "wow",
  "priority": 700
}
```

**Response:** same structure as install.

### Repair

```text
POST /repair/{product}
```

Verifies all files and re-downloads corrupted or missing ones. The product
must be in `installed` or `corrupted` status.

**Request:**

```json
{
  "uid": "wow",
  "priority": 700
}
```

**Response:** same structure as install.

### Uninstall

```text
POST /uninstall/{product}
```

Removes a product installation.

**Request:**

```json
{
  "uid": "wow",
  "priority": 700
}
```

**Response:** same structure as install.

### Backfill

```text
POST /backfill/{product}
```

Low-priority background download to fill in optional data.

**Request:**

```json
{
  "uid": "wow",
  "priority": 200
}
```

`priority` defaults to 200. **Response:** same structure as install.

### Extract (cascette extension)

```text
POST /extract/{product}
```

Extracts CASC content to a directory tree. The product must be installed.
Uses stored build/CDN config hashes to fetch the install manifest from CDN,
then reads files from local CASC storage and writes them to `output_path`.

**Request:**

```json
{
  "uid": "wow_classic_era",
  "output_path": "/tmp/extracted",
  "pattern": "Interface/*",
  "priority": 700
}
```

`pattern` is optional. Supports `*` wildcards. `priority` defaults to 700.

**Response:** same structure as install.

### Operation Progress

```text
GET /install/{product}
```

Polls the current operation status for a product. This endpoint serves dual
purpose: `POST` starts an install, `GET` returns progress.

**Response (active operation):**

```json
{
  "uid": "wow",
  "operation_id": "550e8400-e29b-41d4-a716-446655440000",
  "operation_type": "install",
  "state": "downloading",
  "progress": {
    "phase": "downloading",
    "percentage": 45.2,
    "bytes_done": 48480698368,
    "bytes_total": 107374182400,
    "files_done": 1200,
    "files_total": 3500,
    "current_file": "data/data.001",
    "speed_bps": 52428800,
    "eta_seconds": 1124
  },
  "error": null
}
```

**Response (idle):**

```json
{
  "uid": "wow",
  "state": "idle"
}
```

**Operation states:** `queued`, `initializing`, `downloading`, `verifying`,
`complete`, `failed`, `cancelled`.

## Session Endpoints

### List Sessions

```text
GET /gamesession
```

Returns all active game sessions.

**Response:**

```json
{
  "sessions": []
}
```

### Get Session

```text
GET /gamesession/{product}
```

Returns session info for a product.

**Response:**

```json
{
  "uid": "wow",
  "active": true,
  "pid": 12345
}
```

### Start Session

```text
POST /gamesession/{product}
```

Registers a game session. Used by the launcher to track running game processes.

**Request:**

```json
{
  "uid": "wow",
  "pid": 12345
}
```

If `pid` is supplied and the product has known executables, the PID is
validated against live game process PIDs. A PID not belonging to a known
game process returns HTTP 400 with `{"error": 2312}`.

## Download Configuration

### Get Download Settings

```text
GET /download
```

Returns current download speed settings.

**Response:**

```json
{
  "max_speed_bps": 0,
  "paused": false,
  "current_speed_bps": 52428800
}
```

`max_speed_bps` of 0 means unlimited.

### Set Download Settings

```text
POST /download
```

Updates download speed limits.

**Request:**

```json
{
  "max_speed_bps": 10485760,
  "paused": false
}
```

## Product Options

### Get Options

```text
GET /option
```

Returns product user options (language, region preferences).

**Response:**

```json
{
  "options": {}
}
```

### Set Options

```text
POST /option
```

Updates product user options.

**Request:**

```json
{
  "uid": "wow",
  "language": "enUS",
  "region": "us"
}
```

Returns HTTP 400 with `{"error": 2312}` if `uid` is given but the product
is not found.

## Installation Flow

When installing a product, the agent:

1. Queries Ribbit for product version information
2. Downloads build and CDN configuration
3. Fetches encoding and root manifests
4. Downloads required archives from CDN
5. Writes data to local CASC storage
6. Updates local indices

## CLI Usage

```text
cascette-agent [OPTIONS]
```

| Flag | Environment Variable | Default | Description |
|------|---------------------|---------|-------------|
| `--port` | `CASCETTE_AGENT_PORT` | 1120 | HTTP listen port |
| `--db_path` | `CASCETTE_AGENT_DB_PATH` | platform default | SQLite database path |
| `--locale` | `CASCETTE_AGENT_LOCALE` | `enUS` | Default locale |
| `--bind_addr` | `CASCETTE_AGENT_BIND_ADDR` | `127.0.0.1` | Bind address |
| `--loglevel` | `CASCETTE_AGENT_LOG_LEVEL` | `info` | Log level |
| `--patchfreq` | `CASCETTE_AGENT_PATCH_FREQ` | `300` | Patch check interval (seconds) |
| `--max_concurrent_operations` | `CASCETTE_AGENT_MAX_CONCURRENT` | `1` | Max concurrent operations |
| `--request_timeout_secs` | `CASCETTE_AGENT_REQUEST_TIMEOUT` | `30` | HTTP request timeout |
| `--version_server_url` | `CASCETTE_AGENT_VERSION_SERVER_URL` | - | Version server URL override |
| `--show` | - | `false` | Show agent window (Windows) |
| `--allowcommands` | - | `false` | Allow API command execution |
| `--skipupdate` | - | `false` | Skip self-update check |
| `--session` | - | - | Session ID for launcher |

Port fallback order: 1120, 6881, 6882, 6883. If `--port` is specified, only
that port is tried.

## cascette-agent

`cascette-agent` is a replacement implementation of the Battle.net Agent. It
provides the same HTTP API on port 1120 and can be used as a drop-in replacement
for:

- Downloading products from official Blizzard CDNs
- Fallback to community archive mirrors (cdn.arctium.tools)
- Managing local CASC installations

### Differences from Official Agent

- Open source implementation
- Supports community CDN mirrors
- Cross-platform (Linux, macOS, Windows)
- No Battle.net account required for public content
- No protobuf product database (uses structured SQL tables)
- No authorization/signature verification (local-only)
- No Windows service management (planned)
- No telemetry reporting

## References

- [CDN Architecture](../protocols/cdn.md)
- [Ribbit Protocol](../protocols/ribbit.md)
- [CASC Local Storage](local-storage.md)

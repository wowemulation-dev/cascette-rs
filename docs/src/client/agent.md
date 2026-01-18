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

The agent exposes a REST API on `http://127.0.0.1:1120`.

### Endpoints

Documentation of the agent's HTTP endpoints is pending.

<!-- TODO: Document endpoints
- GET /agent - Agent status
- POST /install - Start installation
- GET /progress - Download progress
- etc.
-->

## Installation Flow

When installing a product, the agent:

1. Queries Ribbit for product version information
2. Downloads build and CDN configuration
3. Fetches encoding and root manifests
4. Downloads required archives from CDN
5. Writes data to local CASC storage
6. Updates local indices

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

## References

- [CDN Architecture](../protocols/cdn.md)
- [Ribbit Protocol](../protocols/ribbit.md)
- [CASC Local Storage](local-storage.md)

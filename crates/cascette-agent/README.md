# cascette-agent

Local HTTP agent service compatible with Blizzard Agent.exe (version 3.13.3).

Provides a REST API on port 1120 for managing game product installations,
updates, repairs, and verification.

## Usage

```bash
# Start with default settings (port 1120)
cascette-agent

# Custom port
cascette-agent --port=8080

# Custom database path
cascette-agent --db_path=/path/to/agent.db

# Debug logging
cascette-agent --loglevel=debug
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/agent` | Agent info |
| GET | `/game` | List products |
| GET | `/game/{product}` | Product details |
| POST | `/install/{product}` | Start installation |
| POST | `/update/{product}` | Start update |
| POST | `/repair/{product}` | Start repair |
| POST | `/uninstall/{product}` | Start uninstall |
| POST | `/backfill/{product}` | Start backfill |
| GET | `/version` | Version info |
| GET | `/hardware` | System info |
| GET/POST | `/gamesession/{product}` | Game sessions |
| GET/POST | `/download` | Download config |
| GET/POST | `/option` | Product options |
| GET | `/size_estimate` | Install size |
| GET | `/health` | Health check |
| GET | `/metrics` | Prometheus metrics |

## Configuration

All configuration is passed via environment variables or CLI flags. The flags
mirror the Blizzard Agent.exe command-line interface.

### CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | `1120` | HTTP listen port (fallback: 6881–6883) |
| `--db_path` | platform data dir | Path to the SQLite state database |
| `--loglevel` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `--locale` | `enUS` | Default locale for installations |
| `--region` | `us` | Default region for Ribbit queries |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CASCETTE_AGENT_CDN_HOSTS` | Comma-separated list of CDN host overrides (e.g. `localhost:8000,mirror.example.com`) |
| `CASCETTE_AGENT_CDN_PATH` | CDN base path (default: `tpr/wow`). Override when serving a product with a different path. |
| `CASCETTE_CACHE_DIR` | Directory for protocol metadata cache (Ribbit responses, build/CDN configs). Does **not** cache game archive data. |

> **Note**: `CASCETTE_CACHE_DIR` caches protocol-level metadata only. It does
> not redirect where game archive data is fetched from. To use a local copy of
> game data, see [Local CDN Mirror](#local-cdn-mirror) below.

## Local CDN Mirror

If you have a local copy of CDN data (e.g. on an external drive), you can point
the agent at it using `CASCETTE_AGENT_CDN_HOSTS`. The agent fetches game data
via standard TACT HTTP requests, so any HTTP file server that exposes the flat
TACT directory layout works as a local mirror.

### Mirror Layout

TACT CDN data uses a flat two-level directory structure:

```
<cdn_path>/
  config/<aa>/<bb>/<hash>
  data/<aa>/<bb>/<hash>
  patch/<aa>/<bb>/<hash>
```

For WoW products the default `cdn_path` is `tpr/wow`. A drive mirror at
`/run/media/user/NGDP/mirrors/cdn.blizzard.com/` would contain:

```
/run/media/user/NGDP/mirrors/cdn.blizzard.com/tpr/wow/data/00/01/<hash>
/run/media/user/NGDP/mirrors/cdn.blizzard.com/tpr/wow/config/2c/91/<hash>
...
```

### Serving the Mirror

Serve the mirror root with any HTTP file server. Example using Python:

```bash
cd /run/media/user/NGDP/mirrors/cdn.blizzard.com
python3 -m http.server 8000
```

Then start the agent with the mirror as the CDN host:

```bash
CASCETTE_AGENT_CDN_HOSTS=localhost:8000 cascette-agent
```

### Multiple Hosts

Specify multiple hosts as a comma-separated list. The agent tries them left to
right for each request:

```bash
CASCETTE_AGENT_CDN_HOSTS=localhost:8000,cdn.arctium.tools cascette-agent
```

### Fallback Behavior

Override hosts are **prepended** to the Ribbit-advertised endpoint list for both
standard and historical installs. If a file is missing from your local mirror,
the agent automatically falls back to community mirrors and the official Blizzard
CDN. A partial local mirror works without any additional configuration.

### Historical Installs

When installing a specific build by passing `build_config` and `cdn_config` to
`/install/{product}`, the override hosts are also prepended before the community
mirror and official CDN fallback list:

```bash
CASCETTE_AGENT_CDN_HOSTS=localhost:8000 cascette-agent
# POST /install/wow_classic {"build_config": "2c91...", "cdn_config": "c54b..."}
# -> tries localhost:8000 first, falls back to casc.wago.tools, cdn.arctium.tools, official CDN
```

### Non-WoW Products

For products with a different CDN path (e.g. `tpr/bnt001` for the Blizzard
launcher), override the path as well:

```bash
CASCETTE_AGENT_CDN_HOSTS=localhost:8000 \
CASCETTE_AGENT_CDN_PATH=tpr/bnt001 \
cascette-agent
```

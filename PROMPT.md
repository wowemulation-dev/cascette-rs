# Agent Integration Test: wow_classic 1.13.2.31650

Verify that cascette-agent produces an installation matching the reference at
`~/Downloads/battle.net/wow_classic/1.13.2.31650.windows-win64/`.

## Parameters

| Field | Value |
|-------|-------|
| Product code | `wow_classic` |
| Version | `1.13.2.31650` |
| Region | `eu` |
| Build config | `2c915a9a226a3f35af6c65fcc7b6ca4a` |
| CDN config | `c54b41b3195b9482ce0d3c6bf0b86cdb` |
| Install key | `5090256c2742e6652de8aef3641c6eb1` |
| CDN path | `tpr/wow` |
| Tags (from `.build.info`) | `Windows x86_64 EU? acct-BGR? geoip-BG? enUS speech?` / `Windows x86_64 EU? acct-BGR? geoip-BG? enUS text?` |
| Install manifest tags | `Windows`, `x86_64`, `enUS` (simple tags, not compound) |

The reference installation at `~/Downloads/battle.net/wow_classic/1.13.2.31650.windows-win64/` is an
**upgraded** installation — it contains a small number of stale files from prior versions
that do not need to be replicated. The comparison must account for this.

The agent database is stored at `~/.local/share/cascette/agent/` and can be inspected
with `tursodb`.

## Reverse Engineering Resources

- Management docs: `~/Repos/github.com/wowemulation-dev/management/src/reverse-engineering/agent-3.13.3/`
  - Tag system: `manifests/installation-manifest-tags.md`, `manifests/tag-system-detailed-analysis.md`
  - Install workflow: `manifests/installation-workflow-diagram.md`
  - TACT/CDN: `tact/` (encoding, batch-download, container-storage, loose-file-placement, etc.)
  - Agent HTTP API: `agent/http-router.md`, `agent/http-client.md`
  - SQLite schema: `agent/sqlite-schema.md`
- Binary Ninja MCP: use for decompiling Agent.exe when docs are insufficient

## Procedure

### 0. Pre-flight cleanup

Stop any leftover services and remove stale state from prior runs before
starting. Leftover agent processes, HTTP servers, or partial installations
cause confusing failures.

```bash
# Stop any running cascette-agent
pgrep -x cascette-agent && kill $(pgrep -x cascette-agent) || echo "No agent running"

# Stop any local CDN HTTP server
pgrep -f "range_http_server.py" && kill $(pgrep -f "range_http_server.py") || echo "No HTTP server running"

# Verify nothing is still bound to the agent port
ss -tlnp | grep :1120 && echo "WARNING: port 1120 still in use" || echo "Port 1120 free"

# Remove agent state database
rm -rf ~/.local/share/cascette/agent/

# Remove test wine prefixes from prior runs
rm -rf ~/Downloads/wine_wow_classic_test
rm -rf ~/Downloads/wine_wow_classic_upgrade
rm -rf ~/Downloads/wine_wow_classic_1137
```

Only proceed to step 1 after confirming all processes are stopped and state is
cleared.

### 1. Sync the local CDN mirror

The local mirror at `/run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com/`
is populated from the frankfurt server (`tactic.wowemu.dev`). It is fully
synchronized for `wow_classic` and `wow_classic_era`. Only new builds are added
incrementally, so syncs are fast once the mirror is current.

First, confirm the drive is mounted:

```bash
mount | grep NGDP
```

If it is not mounted, mount it before continuing.

Then sync from the server. `rsync -u` skips files already present on the
destination, making incremental syncs quick:

```bash
rsync -avzru -e ssh \
  root@tactic.wowemu.dev:/opt/ngdp/tact/ \
  /run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com/ \
  --exclude lost+found
```

Files for products not covered by the mirror fall back automatically to
community mirrors and the official Blizzard CDN.

### 2. Prepare the environment

Create a dedicated WINE prefix for the installation. The client will live at
`drive_c/users/Public/Games/WoW Classic/` inside the prefix, which is also
where `cascette-agent` will install directly.

```bash
# Clear all agent state from any previous run
rm -rf ~/.local/share/cascette/agent/

# Create a fresh win64 WINE prefix
WINEARCH=win64 \
  WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_test \
  winecfg
# Set Windows version to Windows 10 in the dialog, then close it.

# Install Gecko and Mono runtimes
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_test \
  wine "/home/$USER/.cache/wine/wine-gecko-2.47.4-x86.msi" /quiet
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_test \
  wine "/home/$USER/.cache/wine/wine-gecko-2.47.4-x86_64.msi" /quiet
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_test \
  wine "/home/$USER/.cache/wine/wine-mono-10.4.1-x86.msi" /quiet

# Create the installation target directory inside the prefix
mkdir -p "/home/$USER/Downloads/wine_wow_classic_test/drive_c/users/Public/Games/WoW Classic"
```

Note: always use absolute paths for `WINEPREFIX`. WINE resolves `~` relative
to its own fake filesystem root, not the real home directory.

### 3. Build and run cascette-agent

Build the agent binary:

```bash
cd ~/Repos/github.com/wowemulation-dev/cascette-rs
cargo build --release -p cascette-agent
```

Serve the local CDN mirror over HTTP **with Range request support**. The
mirror lives at `/run/media/$(whoami)/NGDP/mirrors/cdn.blizzard.com/`.
Run this as a background process (no shell redirects) so its request log
is visible in the task output:

```bash
python3 ~/Repos/github.com/wowemulation-dev/cascette-rs/tools/range_http_server.py \
  /run/media/$(whoami)/NGDP/mirrors/cdn.blizzard.com 8000
```

Run this command with `run_in_background: true` in the Bash tool. The
server logs every request to stderr; these appear in the task output and
can be read later with `TaskOutput`.

The server takes a root directory as its first argument — no `cd` needed.
It uses a thread pool (16 workers by default) to handle the agent's
concurrent connections. Pass `--threads N` to adjust.

Do **not** use `python3 -m http.server` — it is single-threaded, ignores
Range headers, and returns full files (HTTP 200), which causes every
byte-range archive fetch to download the entire ~1 GiB archive instead of
the requested slice.

Then start the agent as a background process, pointing it at the local
mirror. Do **not** use shell redirects (`> file 2>&1`) — let stdout/stderr
flow into the task output so logs are observable via `TaskOutput`:

```bash
cd ~/Repos/github.com/wowemulation-dev/cascette-rs

RUST_LOG=cascette_agent=debug,cascette_installation=debug,cascette_protocol::cdn=debug \
  CASCETTE_AGENT_CDN_HOSTS=localhost:8000 \
  ./target/release/cascette-agent \
  --loglevel=debug
```

Run this command with `run_in_background: true` in the Bash tool. After
starting, wait a moment then confirm it is listening:

```bash
curl -s http://127.0.0.1:1120/agent | jq .
```

`CASCETTE_AGENT_CDN_HOSTS=localhost:8000` prepends the local mirror before the
community mirrors and official Blizzard CDN. Files missing from the mirror fall
back automatically — a partial mirror is fine.

The `cascette_protocol::cdn=debug` log target enables per-request logging in
the CDN client. Every HTTP request logs the target URL and host; responses
log the status code and byte count. Failover events are logged at `warn`
level. Use this to verify that requests hit the local mirror and do not
silently fall through to remote CDN endpoints.

The agent auto-detects `localhost` and `127.*` as HTTP (plain), so no `http://`
prefix is required. For remote mirrors you can use `http://host:port` or
`https://host` explicitly; bare hostnames default to HTTPS.

### 4. Register and install

Register `wow_classic` via `/register`. The agent auto-chains an install
operation when a product is newly registered, so no separate `/install` call
is needed. The `build_config` and `cdn_config` fields are cascette extensions
that pin the install to an exact historical build.

```bash
# Register the product and chain the install automatically.
# install_dir points directly into the WINE prefix.
curl -s -X POST http://127.0.0.1:1120/register \
  -H 'Content-Type: application/json' \
  -d '{
    "instructions_product": "NGDP",
    "instructions_patch_url": "http://us.patch.battle.net:1119/wow_classic",
    "uid": "wow_classic",
    "product": "wow_classic",
    "install_dir": "/home/'"$USER"'/Downloads/wine_wow_classic_test/drive_c/users/Public/Games/WoW Classic",
    "region": "eu",
    "subfolder": "_classic_",
    "primary_locale_hint": "enUS",
    "build_config": "2c915a9a226a3f35af6c65fcc7b6ca4a",
    "cdn_config":   "c54b41b3195b9482ce0d3c6bf0b86cdb"
  }' | jq .
```

The `subfolder` field is required for loose file placement. Without it,
`game_subfolder` is `None` and install manifest entries (executables, DLLs)
are not written to the filesystem. Agent.exe receives this from the launcher;
the value comes from the `.product.db` protobuf (`game_subfolder` field 13)
or the product config's `shared_container_default_subfolder`.

### 5. Monitor progress

```bash
# Poll operation state
curl -s http://127.0.0.1:1120/game/wow_classic | jq .

# Query the operations table directly
tursodb ~/.local/share/cascette/agent/agent.db \
  "SELECT operation_id, state, progress FROM operations ORDER BY created_at DESC LIMIT 5;"
```

Both the agent and the HTTP server run as background tasks without shell
redirects. Their logs (stdout/stderr) are captured by the task runner and
can be read at any time with `TaskOutput`. Use this to inspect CDN
requests, failover warnings, and progress updates.

**Verify CDN override is working:** Check that requests hit the local mirror
(`localhost:8000`) and do not fall through to remote endpoints. The CDN
client logs every request at `debug` level with the target host. Read the
agent's task output to confirm:

- CDN requests target `localhost:8000`
- No failover warnings appear (these indicate the file is missing from the
  local mirror and fell through to a remote endpoint)
- The HTTP server's task output shows corresponding request lines

If requests target a host other than `localhost:8000` without a preceding
failover warning, the CDN host override is not applied correctly.

### 6. Compare results

Once the operation reaches `complete`, diff the output against the reference:

```bash
REFERENCE=~/Downloads/battle.net/wow_classic/1.13.2.31650.windows-win64
OUTPUT=~/Downloads/wine_wow_classic_test/drive_c/users/Public/Games/WoW\ Classic

# Files present in output but not reference (unexpected extras)
comm -23 <(find "$OUTPUT" -type f | sed "s|$OUTPUT/||" | sort) \
         <(find "$REFERENCE" -type f | sed "s|$REFERENCE/||" | sort)

# Files present in reference but missing from output (missing files)
# Note: some reference files are stale from upgrades and may legitimately be absent
comm -13 <(find "$OUTPUT" -type f | sed "s|$OUTPUT/||" | sort) \
         <(find "$REFERENCE" -type f | sed "s|$REFERENCE/||" | sort)

# Compare file sizes for files present in both
diff <(find "$OUTPUT"    -type f -exec du -b {} \; | sed "s|$OUTPUT/||"    | sort -k2) \
     <(find "$REFERENCE" -type f -exec du -b {} \; | sed "s|$REFERENCE/||" | sort -k2)
```

### 7. WINE smoke-test

Run the client in the test WINE prefix to verify it starts correctly:

```bash
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_test \
  wine "/home/$USER/Downloads/wine_wow_classic_test/drive_c/users/Public/Games/WoW Classic/_classic_/Wow.exe" \
  -console
```

Manually verify:
- The client starts and shows the language selection dialog.
- After selecting a language, the login screen loads.
- No crash dialogs or missing-file errors in the console output.

If the client shows an "updated version available" dialog, `.patch.result` is
missing or contains a non-zero value. If DLLs or locale files are missing,
check the install manifest tag filtering.

### 8. On failure: diagnose and fix

```bash
# Stop the agent (find its PID since it runs as a background task)
pgrep -x cascette-agent && kill $(pgrep -x cascette-agent) || echo "No agent running"

# Inspect the failed operation
tursodb ~/.local/share/cascette/agent/agent.db \
  "SELECT state, error, progress FROM operations ORDER BY created_at DESC LIMIT 1;"
```

Read the agent's task output (`TaskOutput`) to inspect error messages and
the CDN request log leading up to the failure.

Then:

1. Read the relevant management docs under
   `~/Repos/github.com/wowemulation-dev/management/src/reverse-engineering/agent-3.13.3/`.
2. If docs are insufficient, use the Binary Ninja MCP to decompile Agent.exe.
3. Fix the issue in `cascette-agent` or the crate it depends on
   (`cascette-installation`, `cascette-protocol`, `cascette-formats`, etc.).
4. Record the root cause and fix in the "Known Issues and Lessons Learned"
   section of this file before moving on.
5. Clear agent state, rebuild, and restart from step 1.

```bash
rm -rf ~/.local/share/cascette/agent/
cargo build --release -p cascette-agent
```

Repeat until the output matches the reference installation.

---

# Phase 2: wow_classic 1.13.7.38631 — Update and Smoke-Test

**Prerequisite:** Phase 1 must complete with a fully validated 1.13.2.31650 installation before starting this phase.

## Parameters

| Field | Value |
|-------|-------|
| Product code | `wow_classic` |
| Version | `1.13.7.38631` |
| Region | `eu` |
| Build config | `4ffc9fd8dd2bf6a604313908898aa78c` |
| CDN config | `9cf97a7504d69ef3e25a843244d9efcd` |
| CDN path | `tpr/wow` |
| Install manifest tags | `Windows`, `x86_64`, `enUS` (simple tags, not compound) |

## Procedure

### Phase 2.1: Upgrade from 1.13.2 to 1.13.7

Create a new WINE prefix and copy the validated 1.13.2 installation into it as
the upgrade base. Then point `cascette-agent` at that directory with the 1.13.7
configs to apply the update in-place.

```bash
# Create a fresh win64 WINE prefix for the upgrade test
WINEARCH=win64 \
  WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_upgrade \
  winecfg
# Set Windows version to Windows 10, then close.

WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_upgrade \
  wine "/home/$USER/.cache/wine/wine-gecko-2.47.4-x86.msi" /quiet
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_upgrade \
  wine "/home/$USER/.cache/wine/wine-gecko-2.47.4-x86_64.msi" /quiet
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_upgrade \
  wine "/home/$USER/.cache/wine/wine-mono-10.4.1-x86.msi" /quiet

# Copy the validated 1.13.2 installation into the prefix
mkdir -p "/home/$USER/Downloads/wine_wow_classic_upgrade/drive_c/users/Public/Games/WoW Classic"
cp -a "/home/$USER/Downloads/wine_wow_classic_test/drive_c/users/Public/Games/WoW Classic/." \
      "/home/$USER/Downloads/wine_wow_classic_upgrade/drive_c/users/Public/Games/WoW Classic/"

# Clear agent state
rm -rf ~/.local/share/cascette/agent/
```

Start the agent as a background process (no shell redirects). Run this
command with `run_in_background: true` in the Bash tool:

```bash
RUST_LOG=cascette_agent=debug,cascette_installation=debug,cascette_protocol::cdn=debug \
  CASCETTE_AGENT_CDN_HOSTS=localhost:8000 \
  ./target/release/cascette-agent \
  --loglevel=debug
```

Wait a moment for the agent to start, then register and trigger the upgrade:

```bash
# Register and trigger upgrade — install_dir points into the prefix.
# /register auto-chains the install operation for newly registered products.
curl -s -X POST http://127.0.0.1:1120/register \
  -H 'Content-Type: application/json' \
  -d '{
    "instructions_product": "NGDP",
    "instructions_patch_url": "http://us.patch.battle.net:1119/wow_classic",
    "uid": "wow_classic",
    "product": "wow_classic",
    "install_dir": "/home/'"$USER"'/Downloads/wine_wow_classic_upgrade/drive_c/users/Public/Games/WoW Classic",
    "region": "eu",
    "subfolder": "_classic_",
    "primary_locale_hint": "enUS",
    "build_config": "4ffc9fd8dd2bf6a604313908898aa78c",
    "cdn_config":   "9cf97a7504d69ef3e25a843244d9efcd"
  }' | jq .
```

Monitor progress the same way as Phase 1 (step 5).

### Phase 2.2: Fresh install of 1.13.7

Create a second WINE prefix and install 1.13.7 from scratch into it. This
produces the reference to diff against the upgraded prefix.

```bash
# Create a fresh win64 WINE prefix for the 1.13.7 reference install
WINEARCH=win64 \
  WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_1137 \
  winecfg
# Set Windows version to Windows 10, then close.

WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_1137 \
  wine "/home/$USER/.cache/wine/wine-gecko-2.47.4-x86.msi" /quiet
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_1137 \
  wine "/home/$USER/.cache/wine/wine-gecko-2.47.4-x86_64.msi" /quiet
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_1137 \
  wine "/home/$USER/.cache/wine/wine-mono-10.4.1-x86.msi" /quiet

mkdir -p "/home/$USER/Downloads/wine_wow_classic_1137/drive_c/users/Public/Games/WoW Classic"

# Clear agent state
rm -rf ~/.local/share/cascette/agent/
```

Start the agent as a background process (no shell redirects). Run this
command with `run_in_background: true` in the Bash tool:

```bash
RUST_LOG=cascette_agent=debug,cascette_installation=debug,cascette_protocol::cdn=debug \
  CASCETTE_AGENT_CDN_HOSTS=localhost:8000 \
  ./target/release/cascette-agent \
  --loglevel=debug
```

Wait a moment for the agent to start, then confirm it is listening:

```bash
curl -s http://127.0.0.1:1120/agent | jq .

# Register and install directly into the prefix.
# /register auto-chains the install operation for newly registered products.
curl -s -X POST http://127.0.0.1:1120/register \
  -H 'Content-Type: application/json' \
  -d '{
    "instructions_product": "NGDP",
    "instructions_patch_url": "http://us.patch.battle.net:1119/wow_classic",
    "uid": "wow_classic",
    "product": "wow_classic",
    "install_dir": "/home/'"$USER"'/Downloads/wine_wow_classic_1137/drive_c/users/Public/Games/WoW Classic",
    "region": "eu",
    "subfolder": "_classic_",
    "primary_locale_hint": "enUS",
    "build_config": "4ffc9fd8dd2bf6a604313908898aa78c",
    "cdn_config":   "9cf97a7504d69ef3e25a843244d9efcd"
  }' | jq .
```

Monitor progress until `complete`, then compare the fresh install against the upgraded prefix:

```bash
FRESH="/home/$USER/Downloads/wine_wow_classic_1137/drive_c/users/Public/Games/WoW Classic"
UPGRADED="/home/$USER/Downloads/wine_wow_classic_upgrade/drive_c/users/Public/Games/WoW Classic"

# Files in upgraded but not in fresh (stale files from 1.13.2 not removed)
comm -23 <(find "$UPGRADED" -type f | sed "s|$UPGRADED/||" | sort) \
         <(find "$FRESH"    -type f | sed "s|$FRESH/||"    | sort)

# Files in fresh but missing from upgraded (files that should have been added)
comm -13 <(find "$UPGRADED" -type f | sed "s|$UPGRADED/||" | sort) \
         <(find "$FRESH"    -type f | sed "s|$FRESH/||"    | sort)

# Size comparison for files present in both
diff <(find "$UPGRADED" -type f -exec du -b {} \; | sed "s|$UPGRADED/||" | sort -k2) \
     <(find "$FRESH"    -type f -exec du -b {} \; | sed "s|$FRESH/||"    | sort -k2)
```

Stale files left over from 1.13.2 are acceptable in the upgraded directory. Missing or size-mismatched files are failures that require diagnosis.

### Phase 2.3: WINE smoke-test

Each output directory is tested by running the client in its own WINE prefix.
This is a manual check — the goal is to confirm the client launches, asks for
a language, and reaches the login screen without crash dialogs or missing-file
errors.

Both directories live in their own WINE prefixes created in Phases 2.1 and 2.2.

**Smoke-test the upgraded installation** (prefix already exists from Phase 2.1):

```bash
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_upgrade \
  wine "/home/$USER/Downloads/wine_wow_classic_upgrade/drive_c/users/Public/Games/WoW Classic/_classic_/Wow.exe" \
  -console
```

**Smoke-test the fresh 1.13.7 installation** (prefix already exists from Phase 2.2):

```bash
WINEPREFIX=/home/$USER/Downloads/wine_wow_classic_1137 \
  wine "/home/$USER/Downloads/wine_wow_classic_1137/drive_c/users/Public/Games/WoW Classic/_classic_/Wow.exe" \
  -console
```

Manually verify for each:
- The client starts, shows the language selection dialog, then reaches the login screen.
- No crash dialog appears.
- No missing file errors in the console output.

Clean up both prefixes after testing:

```bash
rm -rf /home/$USER/Downloads/wine_wow_classic_upgrade \
       /home/$USER/Downloads/wine_wow_classic_1137
```

### Phase 2.4: On failure — diagnose and fix

Follow the same diagnosis steps as Phase 1 step 7. Additionally:

- For upgrade-specific failures, compare the delta between the two build configs:
  the patch manifest (`patch_config` in the build config) lists which files changed
  between versions and is the primary source of truth for what an update must deliver.
- If the WINE smoke-test fails with missing DLL or file-not-found errors, cross-check
  the install manifest tag filtering — a wrong tag set can omit required locale or
  platform files.

## Updating This File

When a run reveals new root causes, workarounds, or behavioral quirks — update
this file before ending the session. Add entries to the "Known Issues and
Lessons Learned" section. If the fix changes how the procedure should be run,
update the relevant step. Keep entries factual and specific: what failed, why,
and what the fix was. Remove entries that are no longer relevant (e.g. a bug
that has since been fixed in the codebase).

## Known Issues and Lessons Learned

### Tag filtering

The `.build.info` Tags field uses compound conditional tags
(`Windows x86_64 EU? enUS speech?`). The **install manifest** uses simple tags
(`Windows`, `x86_64`, `enUS`). The `TagQuery` struct produces simple tags via
`tag_names()` for manifest filtering, and compound tags via `build_info_tags()`
for `.build.info`. Do not pass the compound tag syntax to the install pipeline.

Tag system docs: `management/src/reverse-engineering/agent-3.13.3/manifests/`.

### Startup recovery

If the agent is killed mid-install, the operation is left in `downloading` or
`verifying` state in the database. On next startup `Database::migrate()` resets
these to `queued` automatically. It also resets the corresponding product status
(`installing` → `available`, `updating` → `installed`) so the install executor
can re-enter its initial state transition on resume. The install pipeline
resumes from its checkpoint (completed encoding keys are saved). Do not manually
edit the DB to reset state — just restart the agent.

### Historical build CDN endpoint ordering

When passing explicit `build_config`/`cdn_config`, CDN host overrides
(`CASCETTE_AGENT_CDN_HOSTS`) are **prepended** before community mirrors and
the official Blizzard CDN. A partial local mirror works without listing
fallback hosts explicitly.

For standard installs (no explicit configs), the same prepend behavior applies
— override hosts come first, Ribbit-advertised endpoints follow as fallback.

### Agent database location

Linux: `~/.local/share/cascette/agent/agent.db`

### Progress phase labels

The operation `progress` field cycles through phases:
`resolving` → `downloading indices` → `downloading` → `verifying` → `complete`.

### Product code note

`wow_classic` was the product code for versions up to 1.13.x. Version 1.14.0
and later use `wow_classic_era`. Use `wow_classic` for this test target.

### Registration endpoint (not /install)

The `/install/{product}` endpoint requires the product to already exist via
`/register`. Do not POST directly to `/install/wow_classic` for a fresh
install — the handler returns error 2312 (AGENT_ERROR_INVALID_REQUEST) if the
product is not found in the registry. Use `/register` instead: it creates the
product entry and automatically chains an install operation for new products.

### Local CDN mirror HTTP scheme

`CASCETTE_AGENT_CDN_HOSTS` defaults to HTTPS for all hostnames. When using
a local plain-HTTP mirror (e.g., `localhost:8000`), the agent now auto-detects
`localhost` and `127.*` addresses and uses HTTP without requiring an explicit
`http://` prefix. You can also pass a full URL: `http://localhost:8000`.

### Startup recovery — product status reset

When the agent is killed mid-install, the product status is left in
`installing` in the DB. On restart, `reset_inflight_operations` resets the
operation back to `queued` but previously did not reset the product status,
causing the executor to fail with "invalid state transition: installing ->
installing". Fixed: the migration now resets `installing` → `available` and
`updating` → `installed` for all interrupted products.

### Install manifest files must not go into CASC archives

Install manifest entries (executables, DLLs, locale packs) are **loose files**
that must be written directly to `<install_dir>/<subfolder>/<path>`, not into
CASC archive storage. Agent.exe's `LooseFileStreamer::WriteOutput` (0x7103aa)
decodes BLTE data on-the-fly and writes to a `.tmp` file, then atomically
renames to the final path with 3 retries. The download manifest entries are the
ones that go into CASC archives.

Previously, both artifact types were merged into one list and all routed
through `Installation::write_raw_blte()`, causing loose files to end up in
`Data/data/data.NNN` archives and triggering "archive size exceeded" errors.

Fixed by adding `is_loose_file: bool` to `ArtifactDescriptor`. The download
executor now routes:
- `is_loose_file = true`: BLTE data passed to `LooseFileHandler::write_loose_file()`
  which decodes and writes directly to filesystem (temp file + atomic rename).
- `is_loose_file = false`: written to CASC via `write_raw_blte()`, then
  optionally hardlinked/copied to the product directory.

Relevant RE docs: `management/src/reverse-engineering/agent-3.13.3/tact/loose-file-placement.md`

### Archive segment size limit is 1 GiB

Each `data.NNN` archive file must not exceed `SEGMENT_SIZE = 0x4000_0000`
(1 GiB). This is dictated by the IDX format: `FileOffsetBits = 30` means
the within-archive byte offset is stored in 30 bits, giving a maximum
addressable offset of 1 GiB. Real WoW Classic installations confirm this —
no `data.NNN` file exceeds 1 GiB.

Previously `MAX_ARCHIVE_SIZE` was 256 GiB, causing u32 overflow when
computing archive offsets. Fixed to `SEGMENT_SIZE`.

### Size manifest usage

The size manifest (`DS` magic) provides estimated file sizes for progress
reporting and disk space pre-allocation. Agent.exe uses individual `esize`
values as fallback when `cSize` (compressed size) is unavailable. The
`header.total_size` field is the sum of all esizes — a 40-bit value
representing the total install footprint.

The size manifest is **not** cross-referenced against the download manifest.
File classification uses the install manifest, not the size manifest.

Relevant RE docs: `management/src/reverse-engineering/agent-3.13.3/tact/size-manifest.md`

### Range requests require a capable HTTP server

`python3 -m http.server` ignores `Range` headers and returns the full file
with HTTP 200. The CDN client's `download_range()` method uses byte-range
requests to fetch individual BLTE blobs from ~1 GiB CDN archive files. If
the server returns 200 instead of 206, the full archive is downloaded for
every blob, resulting in tens of gigabytes of wasted I/O and corrupted
CASC data.

Fixed in `cascette-protocol/src/cdn/mod.rs`: `download_range()` now rejects
HTTP 200 responses with `RangeNotSupported`, causing fallback to the next
CDN endpoint. Use `tools/range_http_server.py` instead of
`python3 -m http.server` for local mirrors.

### Subfolder required in /register for loose file placement

The `/register` request must include `"subfolder": "_classic_"` (or the
appropriate value for the product). Without it, `InstallConfig.game_subfolder`
is `None`, `LooseFileHandler` is never created, and install manifest entries
(executables, DLLs, locale packs) are not written to the filesystem.

Agent.exe receives the subfolder from three sources (highest priority first):
1. `/register` request body
2. `.product.db` protobuf field 13 (`game_subfolder`)
3. Product config `shared_container_default_subfolder`

cascette currently only supports source 1. The launcher always sends this
field, so omitting it in test procedures causes silent failure.

### install_path must be base directory (not subfolder-appended)

The register handler previously computed `install_path = install_dir + subfolder`,
then `LooseFileHandler` appended the subfolder again, creating a double path like
`WoW Classic/_classic_/_classic_/Wow.exe`. Fixed: `install_path` is now just
`install_dir` (the base directory where `Data/` lives). The subfolder is only
joined at the `LooseFileHandler` level for loose file placement.

Agent.exe stores `install_dir` as-is in the product record. The subfolder is
appended when constructing the product directory for loose files, not at
registration time.

### Path normalization for cross-platform support

Install manifest paths use Windows backslash separators (`Utils\cef.pak`,
`UTILS\LOCALES\DE.PAK`). On Linux, backslashes are valid filename characters,
so `PathBuf::join("Utils\\cef.pak")` creates a file named `Utils\cef.pak`
instead of a file `cef.pak` inside a `Utils/` directory.

Fixed by using `normalize_install_path()` in `LooseFileHandler`, which:
1. Converts `\` to `/`
2. On case-sensitive filesystems (Linux/macOS), uppercases directory components
   to handle the manifest's mixed case (`Utils` vs `UTILS`)

### .flavor.info format (fixed)

Agent.exe writes `.flavor.info` as a single-column BPSV table:
```
Product Flavor!STRING:0
wow_classic
```

cascette now writes this format correctly.

### .product.db completeness (open issue)

The reference `.product.db` (407 bytes) contains a full `ProductInstall`
protobuf with version string, install key, tags, timestamps, and progress
fields. Our `.product.db` (416 bytes) has the core fields (product code,
install path, region, locale, subfolder, build config) but is missing the
richer metadata. This does not prevent the client from launching but differs
from Agent.exe output.

### Launcher CDN path override rewriting

CDN host overrides (`CASCETTE_AGENT_CDN_HOSTS`) carry the game product's
CDN path (e.g. `tpr/wow`). When used for the `bts` (bootstrapper) product
install, the override endpoints' `path` field must be rewritten to the
Ribbit-resolved `tpr/bnt004` path. Without this, launcher configs and data
are fetched from `tpr/wow/config/...` instead of `tpr/bnt004/config/...`,
causing 404s from the local mirror.

Fixed in `resolve_bootstrapper_metadata_inner()` in `helpers.rs`: override
endpoints now have their `path` replaced with the bts product's CDN path.

### Version string in .build.info

The `.build.info` `Version` column must contain the build version (e.g.
`1.13.2.31650`). Without it, the client shows an "updated version available"
dialog and refuses to start. Older build configs lack `client-version` but
have `build-name` (e.g. `WOW-31650patch1.13.2_Retail`). The version is
extracted as `{version}.{build_id}` via regex on the `build-name` field.

Fixed in `build_info.rs`: `version_from_build_name()` parses the version
from `build-name` when `client-version` is absent.

### .patch.result required for client startup

The client reads `.patch.result` at the installation root on startup. If
the file is missing, the client contacts the live patch server and shows
an update dialog when a newer build exists. Writing `0` to this file
signals that no update is pending.

Fixed in `layout/mod.rs`: `write_layout()` now writes `.patch.result`
containing `0` after all other layout files.

### Validation against reverse engineering

When fixing issues discovered during integration testing, validate the fix
against the reverse-engineered Agent.exe documentation at
`~/Repos/github.com/wowemulation-dev/management/src/reverse-engineering/agent-3.13.3/`.
If the docs are insufficient, use the Binary Ninja MCP to decompile the
relevant function in Agent.exe directly. Do not merge fixes without verifying
they match Agent.exe behavior.

### BTS launcher manifest uses product-specific tags

The bts launcher install manifest (Region=="launcher" in bts/versions) contains
launchers for ALL games. It uses non-standard tag groups:

| Tag Group | Standard Game Manifest | BTS Launcher Manifest |
|-----------|----------------------|----------------------|
| Platform | Windows, OSX | Windows, OSX |
| Architecture | x86_64, x86_32, arm64 | product codes: wow, d3, hero, s1, s2, w3, bna, etc. |
| Locale | enUS, deDE, etc. | launcher, installer |

Each entry is tagged `[platform, product_code, "launcher"]`. Standard
architecture/locale tags (`x86_64`, `enUS`) do not exist in this manifest
and are silently ignored by `TagTable::FindTag`.

The correct tag query for the launcher binary operation must include the
`product_tag` (e.g., `"wow"`) to select only the product's launcher. Without
it, the filter reduces to `["Windows", "launcher"]` which selects all 22
game launchers.

The bootstrapper manifest (Region==product_tag, e.g., "wow") has only 2 tags
(`OSX`, `Windows`) and is already product-specific via Ribbit row selection.
No product tag is needed in its tag query.

Fixed in `build_launcher_install_config()` in `helpers.rs`: accepts
`bts_product_tag` parameter. The launcher binary operation passes the
product tag; the bootstrapper operation passes `None`.

### Per-endpoint archive fallback for CDN locality

`download_with_archive_fallback` in `metadata.rs` previously tried the loose
blob on ALL endpoints before falling back to archive resolution. If the local
mirror returned 404, the function tried remote CDN endpoints before
considering archives — even though the archives were available locally.

Fixed: archive-based resolution is now attempted immediately after each
endpoint's loose blob fails, before moving to the next endpoint. This
prevents unnecessary requests to remote CDN hosts when data is available
in local archives.

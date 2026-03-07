# cascette-agent Examples

## agent_verification

Exercises agent AppState initialization and router creation. Optionally runs a maintenance dry-run on a local WoW installation.

```bash
cargo run -p cascette-agent --example agent_verification
```

**Environment variables:**

- `CASCETTE_WOW_PATH` -- Path to a local WoW installation root (optional). When set, runs a maintenance dry-run reporting verified and corrupted entries.
- `CASCETTE_CDN_HOSTS` -- Comma-separated CDN hostnames (optional, defaults to community mirrors).

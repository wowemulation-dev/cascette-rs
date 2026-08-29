# cascette-client-storage Examples

All examples in this crate require the `local-install` feature flag and a local WoW installation. Set the `CASCETTE_WOW_PATH` environment variable to the installation root directory.

```bash
export CASCETTE_WOW_PATH=/path/to/wow_classic/1.13.2.31650.windows-win64
```

## dump_build_info

Parses and prints the `.build.info` file from a local WoW installation, showing all entries and the active entry details (branch, product, version, build key, CDN key, hosts, tags).

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example dump_build_info
```

## dump_index

Loads all `.idx` index files from the Data directory, prints per-bucket statistics with sample entries, and runs round-trip verification on every index entry.

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example dump_index
```

## dump_local_headers

Reads the 30-byte LocalHeader entries from the first 480 bytes of each `data.NNN` segment file. Prints non-zero headers with encoding key, BLTE size, flags, and checksums. Runs round-trip verification.

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example dump_local_headers
```

## dump_segments

Dumps segment headers from all `data.NNN` files in the Data directory. Prints per-bucket header details and runs round-trip verification on each segment header.

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example dump_segments
```

## dump_shmem

Reads `.shmem` control block files from a local WoW installation. Prints version, initialization state, data size, PID tracking info (v5), and runs round-trip verification. The `.shmem` file is only present while the game client or Blizzard Agent is running.

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example dump_shmem
```

## extract_by_content_key

Extracts a file from a local CASC installation by content key (CKey) or encoding key (EKey). When a hex CKey is passed as an argument, it follows the CKey -> EKey -> archive data chain. When no argument is given, it reads the first index entry by encoding key and prints a hex dump.

```bash
# Extract by content key
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example extract_by_content_key -- <ckey_hex>

# Read first index entry (no argument)
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example extract_by_content_key
```

## local_verification

Runs verification against a WoW Classic 1.13.2.31650 installation using pinned hashes. Tests `.build.info` parsing, index loading, encoding table reading and lookup, and local config file parsing (build config and CDN config).

```bash
CASCETTE_WOW_PATH=~/Downloads/battle.net/wow_classic/1.13.2.31650.windows-win64 \
  cargo run -p cascette-client-storage --features local-install --example local_verification
```

Requires a WoW Classic 1.13.2.31650 installation with matching build config `2c915a9a226a3f35af6c65fcc7b6ca4a`.

## read_archives

Opens a local installation, loads indices and archives, then reads a sample of entries (first 5 and last 5) from the full storage stack. Prints encoding key, archive location, and a hex dump of each entry.

```bash
CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage --features local-install --example read_archives
```

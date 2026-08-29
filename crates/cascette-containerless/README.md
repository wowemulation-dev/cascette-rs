# cascette-containerless

Containerless storage backend for modern Blizzard titles (Overwatch 2, Diablo IV, CoD).

## Storage Model

Instead of CASC `.data` archives with `.idx` indices, files are stored as
loose files on disk with an encrypted SQLite database for metadata.

- **Loose files** -- Individual files stored at `{root}/{ekey[0..2]}/{ekey[2..4]}/{ekey}`
- **SQLite database** -- Encrypted with Salsa20, contains file entries, build metadata, and tags
- **Residency tracking** -- In-memory tracking of which files are locally available

## Usage

```rust
use cascette_containerless::{ContainerlessStorage, ContainerlessConfig};
use std::path::PathBuf;

let config = ContainerlessConfig::new(PathBuf::from("/opt/game/Data/data"));
let storage = ContainerlessStorage::open(config).await?;
```

## License

MIT OR Apache-2.0

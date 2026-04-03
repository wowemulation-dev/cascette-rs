# cascette-installation

CASC installation pipeline: install, extract, verify, and repair operations.

## Operations

- **Install** -- Populate `Data/data/`, `Data/indices/`, `Data/config/`,
  `.build.info` from CDN. Matches agent.exe behavior.
- **Extract** -- Write game files from CASC storage to product subdirectories.
- **Verify** -- Check installation integrity (existence, size, or full
  MD5 + BLTE verification).
- **Repair** -- Verify then re-download failures.

## Usage

```rust
use cascette_installation::config::InstallConfig;
use cascette_installation::InstallPipeline;
use std::path::PathBuf;

let config = InstallConfig::new(
    "wow_classic_era".to_string(),
    PathBuf::from("/opt/wow"),
    "tpr/wow".to_string(),
);
let pipeline = InstallPipeline::new(config);
```

## License

MIT OR Apache-2.0

# cascette-installation Examples

## install_product

Demonstrates `InstallConfig` creation, CDN endpoint configuration, tuning parameters, `InstallPipeline` construction, `ProgressEvent` variants, and the expected CASC directory layout. Does not perform a real installation.

```sh
cargo run -p cascette-installation --example install_product --features local-install
```

No prerequisites. Dry-run only -- no CDN access or local installation required.

## verify_installation

Demonstrates `VerifyConfig` with the three `VerifyMode` levels (Existence, Size, Full) and `VerifyPipeline` construction. If `CASCETTE_WOW_PATH` is set, runs verification against a real CASC installation. Otherwise shows configuration only.

```sh
cargo run -p cascette-installation --example verify_installation --features local-install
```

Dry-run by default. To verify a real installation:

```sh
CASCETTE_WOW_PATH=~/Downloads/battle.net/wow_classic/1.13.2.31650.windows-win64 \
  cargo run -p cascette-installation --example verify_installation --features local-install
```

Requires a local WoW Classic (or other CASC) installation with `Data/data/` directory and index files.

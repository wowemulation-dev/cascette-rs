# Project Setup

This page covers the requirements and setup for developing cascette-rs.

## Requirements

### Rust Toolchain

- **Minimum Supported Rust Version (MSRV)**: 1.92.0
- **Edition**: Rust 2024

Install the required toolchain:

```bash
rustup install 1.92.0
rustup default 1.92.0
```

Required components:

```bash
rustup component add rustfmt clippy
```

For WASM development:

```bash
rustup target add wasm32-unknown-unknown
```

### Development Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| `cargo-nextest` | Test runner | `cargo install cargo-nextest` |
| `cargo-deny` | Dependency auditing | `cargo install cargo-deny` |
| `cargo-llvm-cov` | Code coverage | `cargo install cargo-llvm-cov` |
| `mdbook` | Documentation | `cargo install mdbook` |

### Optional Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| `ripgrep` | Code search | `cargo install ripgrep` or system package |
| `hyperfine` | Benchmarking | `cargo install hyperfine` |
| `cargo-watch` | Auto-rebuild | `cargo install cargo-watch` |

## Repository Structure

```text
cascette-rs/
├── crates/                    # Workspace members
│   ├── cascette-crypto/       # Cryptographic primitives
│   ├── cascette-formats/      # Binary format parsers
│   └── ...
├── docs/                      # mdBook documentation
│   ├── src/                   # Documentation source
│   └── book.toml              # mdBook configuration
├── deny.toml                  # cargo-deny configuration
├── Cargo.toml                 # Workspace manifest
└── CLAUDE.md                  # AI assistant guidance
```

## First-Time Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/wowemulation-dev/cascette-rs.git
   cd cascette-rs
   ```

2. Verify the toolchain:

   ```bash
   rustc --version  # Should be 1.92.0 or later
   cargo --version
   ```

3. Build the workspace:

   ```bash
   cargo build --workspace
   ```

4. Run tests:

   ```bash
   cargo nextest run --workspace
   ```

5. Verify lints pass:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets
   ```

## IDE Configuration

### VS Code

Recommended extensions:

- `rust-analyzer` - Rust language support
- `Even Better TOML` - TOML file support
- `crates` - Dependency version management

Settings (`.vscode/settings.json`):

```json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.allTargets": true,
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### JetBrains (RustRover/IntelliJ)

- Install the Rust plugin
- Enable "Run rustfmt on save"
- Configure clippy as the external linter

## Quality Gate

All changes must pass the CI workflow before merging. Run these checks locally:

```bash
# Full CI check (run before committing)
cargo fmt --all -- --check && \
cargo clippy --workspace --all-targets && \
cargo test --workspace && \
cargo doc --workspace --no-deps
```

Individual checks:

| Command | Purpose |
|---------|---------|
| `cargo fmt --all -- --check` | Format verification |
| `cargo clippy --workspace --all-targets` | Lint checks |
| `cargo test --workspace` | Unit and integration tests |
| `cargo doc --workspace --no-deps` | Documentation build |
| `cargo deny check` | Dependency audit |

### WASM Compatibility

Core libraries must compile to WASM:

```bash
cargo check --target wasm32-unknown-unknown -p cascette-crypto
cargo check --target wasm32-unknown-unknown -p cascette-formats
```

## Documentation

Build and serve the documentation locally:

```bash
# Build HTML documentation
mdbook build docs

# Serve locally with auto-reload
mdbook serve docs --open
```

The documentation will be available at `http://localhost:3000`.

## Workspace Configuration

The workspace uses strict linting. Key settings from `Cargo.toml`:

```toml
[workspace.lints.clippy]
# Lint groups
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }

# Safety lints (higher priority)
unwrap_used = { level = "warn", priority = 2 }
panic = { level = "warn", priority = 2 }
expect_used = { level = "warn", priority = 2 }
```

Library code should avoid `unwrap()`, `expect()`, and `panic!()`. Use `Result`
types and proper error handling instead.

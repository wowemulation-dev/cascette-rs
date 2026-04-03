//! Integration tests that decode real `.product.db` files.
//!
//! Per-install `.product.db` files contain a single serialized
//! `ProductInstall` message (not wrapped in `Database`). The main
//! `product.db` (SQLite-backed) wraps multiple installs in a `Database`.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use prost::Message;

use cascette_proto::proto_database::ProductInstall;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures")
}

/// Decode a `.product.db` file as a `ProductInstall` and verify fields.
fn verify_product_db(path: &Path) {
    let data = std::fs::read(path).unwrap();
    assert!(!data.is_empty(), "file is empty: {}", path.display());

    let pi = ProductInstall::decode(data.as_slice()).unwrap();
    assert!(pi.uid.is_some(), "uid missing");
    assert!(pi.product_code.is_some(), "product_code missing");

    let settings = pi.settings.as_ref().unwrap();
    assert!(settings.install_path.is_some(), "install_path missing");

    let state = pi
        .cached_product_state
        .as_ref()
        .unwrap()
        .base_product_state
        .as_ref()
        .unwrap();
    assert_eq!(state.installed, Some(true));
    assert_eq!(state.playable, Some(true));
}

/// Decode and re-encode, then decode again and compare field equality.
fn verify_roundtrip(path: &Path) {
    let data = std::fs::read(path).unwrap();
    let pi = ProductInstall::decode(data.as_slice()).unwrap();
    let reencoded = pi.encode_to_vec();
    let pi2 = ProductInstall::decode(reencoded.as_slice()).unwrap();
    assert_eq!(pi, pi2, "roundtrip mismatch for {}", path.display());
}

// ─── Fixture-based tests (always run) ───────────────────────────────

#[test]
fn decode_fixture_wow_classic_1_13_2() {
    let path = fixtures_dir().join("wow_classic_1.13.2.product.db");
    verify_product_db(&path);
    verify_roundtrip(&path);

    let data = std::fs::read(&path).unwrap();
    let pi = ProductInstall::decode(data.as_slice()).unwrap();
    assert_eq!(pi.uid.as_deref(), Some("wow_classic"));
    assert_eq!(pi.product_code.as_deref(), Some("wow_classic"));

    let base = pi.cached_product_state.unwrap().base_product_state.unwrap();
    assert_eq!(base.current_version_str.as_deref(), Some("1.13.2.31650"));
}

#[test]
fn decode_fixture_wow_classic_1_14_0() {
    let path = fixtures_dir().join("wow_classic_1.14.0.product.db");
    verify_product_db(&path);
    verify_roundtrip(&path);

    // Product was renamed from wow_classic to wow_classic_era around 1.14.
    let data = std::fs::read(&path).unwrap();
    let pi = ProductInstall::decode(data.as_slice()).unwrap();
    assert_eq!(pi.product_code.as_deref(), Some("wow_classic_era"));
}

#[test]
fn decode_fixture_wow_classic_1_15_2_macos() {
    let path = fixtures_dir().join("wow_classic_1.15.2_macos.product.db");
    verify_product_db(&path);
    verify_roundtrip(&path);

    let data = std::fs::read(&path).unwrap();
    let pi = ProductInstall::decode(data.as_slice()).unwrap();
    assert_eq!(pi.product_code.as_deref(), Some("wow_classic_era"));
}

// ─── Local data tests (ignored, for manual runs) ────────────────────

#[test]
#[ignore = "requires local test data at ~/Downloads/wow_classic/"]
fn decode_all_local_product_dbs() {
    let home = std::env::var("HOME").unwrap();
    let root = PathBuf::from(home).join("Downloads/wow_classic");
    if !root.exists() {
        return;
    }

    let mut count = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let entry = entry.unwrap();
        let db_path = entry.path().join(".product.db");
        if db_path.exists() {
            verify_product_db(&db_path);
            verify_roundtrip(&db_path);
            count += 1;

            let data = std::fs::read(&db_path).unwrap();
            let pi = ProductInstall::decode(data.as_slice()).unwrap();
            eprintln!(
                "{}: uid={:?} code={:?} version={:?}",
                entry.file_name().to_string_lossy(),
                pi.uid,
                pi.product_code,
                pi.cached_product_state
                    .as_ref()
                    .and_then(|s| s.base_product_state.as_ref())
                    .and_then(|b| b.current_version_str.as_deref()),
            );
        }
    }
    assert!(
        count > 0,
        "no .product.db files found in {}",
        root.display()
    );
    eprintln!("verified {count} .product.db files");
}

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for patch-related config fields.
//!
//! The patch config fixture contains key=value fields extracted from a WoW
//! Classic Era build config (patch, patch-index, patch-size, build-name).
//! These are parsed using BuildConfig since they follow the same format.

use cascette_formats::config::BuildConfig;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/config")
        .leak()
}

fn load_fixture(filename: &str) -> Vec<u8> {
    let path = fixtures_dir().join(filename);
    std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

#[test]
fn patch_config_classic_era_parse() {
    let data = load_fixture("wow_classic_era_patch_config.txt");
    let config = BuildConfig::parse(data.as_slice()).expect("Should parse patch config fields");

    // Verify patch-related fields are present.
    assert!(config.patch().is_some());
    assert!(config.patch().is_some());
}

#[test]
fn patch_config_classic_era_has_expected_fields() {
    let data = load_fixture("wow_classic_era_patch_config.txt");
    let text = String::from_utf8(data).expect("Should be valid UTF-8");

    assert!(text.contains("patch ="));
    assert!(text.contains("patch-index ="));
    assert!(text.contains("build-name ="));
}

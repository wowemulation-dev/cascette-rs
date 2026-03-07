#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for SizeManifest parsing using real CDN data.
//!
//! Tests parse a truncated size manifest (DS) downloaded from Blizzard CDN
//! for WoW Classic Era.

use cascette_formats::size::SizeManifest;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/size")
        .leak()
}

fn load_fixture(filename: &str) -> Vec<u8> {
    let path = fixtures_dir().join(filename);
    std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

#[test]
fn size_manifest_classic_era_v1_parse() {
    let data = load_fixture("wow_classic_era_v1_100entries.size");
    let manifest = SizeManifest::parse(&data).expect("Should parse DS v1 manifest");

    assert_eq!(manifest.header.version, 1);
    assert_eq!(manifest.header.ekey_size, 9);
    assert_eq!(manifest.header.num_files, 100);
    assert_eq!(manifest.header.num_tags, 29);
    assert_eq!(manifest.entries.len(), 100);
    assert_eq!(manifest.tags.len(), 29);
}

#[test]
fn size_manifest_classic_era_v1_entries_valid() {
    let data = load_fixture("wow_classic_era_v1_100entries.size");
    let manifest = SizeManifest::parse(&data).expect("Should parse");

    for entry in &manifest.entries {
        assert_eq!(entry.key.len(), 9, "EKey should be 9 bytes");
    }
}

#[test]
fn size_manifest_classic_era_v1_round_trip() {
    let data = load_fixture("wow_classic_era_v1_100entries.size");
    let manifest = SizeManifest::parse(&data).expect("Should parse");

    let built = manifest.build().expect("Should build");
    let reparsed = SizeManifest::parse(&built).expect("Should re-parse built data");

    assert_eq!(reparsed.header.version, manifest.header.version);
    assert_eq!(reparsed.header.ekey_size, manifest.header.ekey_size);
    assert_eq!(reparsed.header.num_files, manifest.header.num_files);
    assert_eq!(reparsed.header.num_tags, manifest.header.num_tags);
    assert_eq!(reparsed.entries.len(), manifest.entries.len());
}

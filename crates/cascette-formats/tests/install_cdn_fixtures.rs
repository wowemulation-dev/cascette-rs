#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for install manifest parsing using real CDN data
//!
//! Tests parse real install manifests downloaded from Blizzard CDN via
//! cascette-py. Install manifests are small enough to include in full.

use cascette_formats::install::{InstallManifest, TagType};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/install")
        .leak()
}

fn read_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

// --- Classic Era 1.15.7 V1 install manifest tests ---

#[test]
fn install_cdn_classic_era_v1_parse() {
    let data = read_fixture("classic_era_1.15.7_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic Era V1 install manifest parse should succeed");

    assert_eq!(manifest.header.version, 1);
    assert_eq!(manifest.header.entry_count, 240);
    assert_eq!(manifest.header.tag_count, 29);
    assert_eq!(manifest.header.ckey_length, 16);
}

#[test]
fn install_cdn_classic_era_v1_header_size() {
    let data = read_fixture("classic_era_1.15.7_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic Era V1 install manifest parse should succeed");

    // V1 header is 10 bytes
    assert_eq!(manifest.header.header_size(), 10);
    // No V2 extended fields
    assert!(manifest.header.content_key_size.is_none());
    assert!(manifest.header.entry_count_v2.is_none());
}

#[test]
fn install_cdn_classic_era_v1_entries() {
    let data = read_fixture("classic_era_1.15.7_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic Era V1 install manifest parse should succeed");

    assert_eq!(manifest.entries.len(), 240);

    // V1 entries should not have file_type
    for entry in &manifest.entries {
        assert!(
            entry.file_type.is_none(),
            "V1 entries should not have file_type"
        );
        // All entries should have non-empty paths
        assert!(!entry.path.is_empty(), "Entry path should not be empty");
        // Content keys should be 16 bytes
        assert_eq!(entry.content_key.as_bytes().len(), 16);
    }
}

#[test]
fn install_cdn_classic_era_v1_tags() {
    let data = read_fixture("classic_era_1.15.7_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic Era V1 install manifest parse should succeed");

    assert_eq!(manifest.tags.len(), 29);

    let tag_names: Vec<&str> = manifest.tags.iter().map(|t| t.name.as_str()).collect();

    // Should have standard platform and architecture tags
    assert!(tag_names.contains(&"Windows"), "Should have Windows tag");
    assert!(tag_names.contains(&"x86_64"), "Should have x86_64 tag");
    assert!(tag_names.contains(&"enUS"), "Should have enUS locale tag");

    // Each tag bitmask should be ceil(240/8) = 30 bytes
    for tag in &manifest.tags {
        assert_eq!(
            tag.bit_mask.len(),
            30,
            "Tag '{}' bitmask should be 30 bytes for 240 entries",
            tag.name
        );
    }
}

#[test]
fn install_cdn_classic_era_v1_tag_types() {
    let data = read_fixture("classic_era_1.15.7_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic Era V1 install manifest parse should succeed");

    // Verify presence of each tag type category
    assert!(
        manifest
            .tags
            .iter()
            .any(|t| t.tag_type == TagType::Platform),
        "Should have platform tags"
    );
    assert!(
        manifest
            .tags
            .iter()
            .any(|t| t.tag_type == TagType::Architecture),
        "Should have architecture tags"
    );
    assert!(
        manifest.tags.iter().any(|t| t.tag_type == TagType::Locale),
        "Should have locale tags"
    );
}

#[test]
fn install_cdn_classic_era_v1_round_trip() {
    let data = read_fixture("classic_era_1.15.7_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic Era V1 install manifest parse should succeed");

    let rebuilt = manifest.build().expect("Build should succeed");
    let reparsed =
        InstallManifest::parse(&rebuilt).expect("Re-parse of rebuilt data should succeed");

    assert_eq!(manifest.header, reparsed.header);
    assert_eq!(manifest.entries.len(), reparsed.entries.len());
    assert_eq!(manifest.tags.len(), reparsed.tags.len());

    for (orig, rebuilt) in manifest.entries.iter().zip(reparsed.entries.iter()) {
        assert_eq!(orig.path, rebuilt.path);
        assert_eq!(orig.content_key, rebuilt.content_key);
        assert_eq!(orig.file_size, rebuilt.file_size);
    }
}

// --- Classic 4.4.0 V1 install manifest tests ---

#[test]
fn install_cdn_classic_440_v1_parse() {
    let data = read_fixture("classic_4.4.0_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic 4.4.0 V1 install manifest parse should succeed");

    assert_eq!(manifest.header.version, 1);
    assert_eq!(manifest.header.entry_count, 182);
    assert_eq!(manifest.header.tag_count, 27);
}

#[test]
fn install_cdn_classic_440_v1_entries() {
    let data = read_fixture("classic_4.4.0_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic 4.4.0 V1 install manifest parse should succeed");

    assert_eq!(manifest.entries.len(), 182);

    // Check that file paths look reasonable (should contain game files)
    let has_exe = manifest.entries.iter().any(|e| {
        let p = std::path::Path::new(&e.path);
        p.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("app"))
    });
    assert!(has_exe, "Should contain at least one executable file");
}

#[test]
fn install_cdn_classic_440_v1_round_trip() {
    let data = read_fixture("classic_4.4.0_v1.install");
    let manifest = InstallManifest::parse(&data)
        .expect("Classic 4.4.0 V1 install manifest parse should succeed");

    let rebuilt = manifest.build().expect("Build should succeed");
    let reparsed =
        InstallManifest::parse(&rebuilt).expect("Re-parse of rebuilt data should succeed");

    assert_eq!(manifest.header, reparsed.header);
    assert_eq!(manifest.entries.len(), reparsed.entries.len());
    assert_eq!(manifest.tags.len(), reparsed.tags.len());
}

// --- Cross-product comparison ---

#[test]
fn install_cdn_cross_product_tag_consistency() {
    let era_data = read_fixture("classic_era_1.15.7_v1.install");
    let era = InstallManifest::parse(&era_data).expect("Parse should succeed");

    let cata_data = read_fixture("classic_4.4.0_v1.install");
    let cata = InstallManifest::parse(&cata_data).expect("Parse should succeed");

    // Both should have common tags (Windows, enUS, x86_64 at minimum)
    let era_names: Vec<&str> = era.tags.iter().map(|t| t.name.as_str()).collect();
    let cata_names: Vec<&str> = cata.tags.iter().map(|t| t.name.as_str()).collect();

    for common_tag in &["Windows", "enUS", "x86_64"] {
        assert!(
            era_names.contains(common_tag),
            "Classic Era should have {common_tag} tag"
        );
        assert!(
            cata_names.contains(common_tag),
            "Classic 4.4.0 should have {common_tag} tag"
        );
    }
}

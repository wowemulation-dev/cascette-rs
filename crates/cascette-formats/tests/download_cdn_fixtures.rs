#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for download manifest parsing using real CDN data
//!
//! Tests parse real download manifests downloaded from Blizzard CDN via
//! cascette-py. The V1 fixture is truncated to 100 entries with tag
//! bitmasks shortened to match.

use cascette_formats::download::{DownloadManifest, DownloadTag, TagType};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/download")
        .leak()
}

fn read_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

// --- V1 Classic Era download manifest tests ---

#[test]
fn download_cdn_v1_parse() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    assert_eq!(manifest.header.version(), 1);
    assert_eq!(manifest.header.entry_count(), 100);
    assert_eq!(manifest.header.tag_count(), 29);
    assert_eq!(manifest.header.ekey_length(), 16);
    assert!(!manifest.header.has_checksum());
    assert_eq!(manifest.header.flag_size(), 0);
}

#[test]
fn download_cdn_v1_header_size() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    // V1 header is exactly 11 bytes
    assert_eq!(manifest.header.header_size(), 11);
}

#[test]
fn download_cdn_v1_entries() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    assert_eq!(manifest.entries.len(), 100);

    // All entries should have 16-byte encoding keys
    for entry in &manifest.entries {
        assert_eq!(entry.encoding_key.as_bytes().len(), 16);
        // File sizes should be reasonable (non-zero for real data)
        // Note: some entries may have 0 size
    }
}

#[test]
fn download_cdn_v1_tags() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    assert_eq!(manifest.tags.len(), 29);

    // Collect tag names
    let tag_names: Vec<&str> = manifest.tags.iter().map(|t| t.name.as_str()).collect();

    // Should have platform tags
    assert!(tag_names.contains(&"Windows"), "Should have Windows tag");

    // Should have architecture tags
    assert!(tag_names.contains(&"x86_64"), "Should have x86_64 tag");

    // Should have locale tags (enUS at minimum)
    assert!(tag_names.contains(&"enUS"), "Should have enUS locale tag");

    // Each tag bitmask should be ceil(100/8) = 13 bytes
    for tag in &manifest.tags {
        assert_eq!(
            tag.bit_mask.len(),
            13,
            "Tag '{}' bitmask should be 13 bytes for 100 entries",
            tag.name
        );
    }
}

#[test]
fn download_cdn_v1_tag_types() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    // Verify known tag types from the fixture
    let find_tag =
        |name: &str| -> Option<&DownloadTag> { manifest.tags.iter().find(|t| t.name == name) };

    // Platform tags should be type 1
    if let Some(tag) = find_tag("Windows") {
        assert_eq!(
            tag.tag_type,
            TagType::Platform,
            "Windows should be platform type"
        );
    }

    // Architecture tags should be type 2
    if let Some(tag) = find_tag("x86_64") {
        assert_eq!(
            tag.tag_type,
            TagType::Architecture,
            "x86_64 should be architecture type"
        );
    }

    // Locale tags should be type 3
    if let Some(tag) = find_tag("enUS") {
        assert_eq!(tag.tag_type, TagType::Locale, "enUS should be locale type");
    }
}

#[test]
fn download_cdn_v1_round_trip() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    // Build back to bytes
    let rebuilt = manifest.build().expect("Build should succeed");

    // Re-parse and compare
    let reparsed =
        DownloadManifest::parse(&rebuilt).expect("Re-parse of rebuilt data should succeed");

    assert_eq!(manifest.header, reparsed.header);
    assert_eq!(manifest.entries.len(), reparsed.entries.len());
    assert_eq!(manifest.tags.len(), reparsed.tags.len());

    for (orig, rebuilt) in manifest.entries.iter().zip(reparsed.entries.iter()) {
        assert_eq!(orig.encoding_key, rebuilt.encoding_key);
        assert_eq!(orig.file_size, rebuilt.file_size);
        assert_eq!(orig.priority, rebuilt.priority);
    }

    for (orig, rebuilt) in manifest.tags.iter().zip(reparsed.tags.iter()) {
        assert_eq!(orig.name, rebuilt.name);
        assert_eq!(orig.tag_type, rebuilt.tag_type);
        assert_eq!(orig.bit_mask, rebuilt.bit_mask);
    }
}

#[test]
fn download_cdn_v1_validation_passes() {
    let data = read_fixture("classic_era_1.15.7_v1_100entries.download");
    let manifest =
        DownloadManifest::parse(&data).expect("V1 download manifest parse should succeed");

    // Validation should pass for real CDN data
    assert!(manifest.header.validate().is_ok());
}

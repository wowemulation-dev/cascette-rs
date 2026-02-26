#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for BuildConfig parsing using real CDN data
//!
//! Tests parse real build config files downloaded from Blizzard CDN
//! for WoW Classic Era, WoW Classic, and WoW Retail.

use cascette_formats::config::BuildConfig;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/config")
        .leak()
}

/// Load all fixture build config files
fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = fixtures_dir();
    let files = [
        ("wow_classic_era", "wow_classic_era_build_config.txt"),
        ("wow_classic", "wow_classic_build_config.txt"),
        ("wow", "wow_build_config.txt"),
    ];
    files
        .iter()
        .map(|(name, filename)| {
            let path = dir.join(filename);
            let data = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
            (*name, data)
        })
        .collect()
}

#[test]
fn build_config_cdn_parse_all() {
    let fixtures = fixture_files();
    assert!(!fixtures.is_empty(), "Should have fixture files");

    for (name, data) in &fixtures {
        let result = BuildConfig::parse(&data[..]);
        assert!(
            result.is_ok(),
            "Failed to parse {name}: {}",
            result.unwrap_err()
        );
        let config = result.unwrap();
        println!(
            "{name}: root={}, build_name={:?}, vfs_entries={}",
            config.root().unwrap_or("(none)"),
            config.build_name(),
            config.vfs_entries().len(),
        );
    }
}

#[test]
fn build_config_cdn_required_fields() {
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert!(config.root().is_some(), "{name}: should have root");
        assert!(config.encoding().is_some(), "{name}: should have encoding");
        assert!(
            config.encoding_key().is_some(),
            "{name}: should have encoding key"
        );
        assert!(!config.install().is_empty(), "{name}: should have install");
        assert!(
            !config.download().is_empty(),
            "{name}: should have download"
        );
        assert!(config.patch().is_some(), "{name}: should have patch");
        assert!(
            !config.patch_index().is_empty(),
            "{name}: should have patch-index"
        );
    }
}

#[test]
fn build_config_cdn_build_metadata() {
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert!(
            config.build_name().is_some(),
            "{name}: should have build-name"
        );
        assert!(
            config.build_uid().is_some(),
            "{name}: should have build-uid"
        );
        assert!(
            config.build_product().is_some(),
            "{name}: should have build-product"
        );
        assert_eq!(
            config.build_product(),
            Some("WoW"),
            "{name}: build-product should be WoW"
        );
    }
}

#[test]
fn build_config_cdn_known_build_uids() {
    let dir = fixtures_dir();

    let data = std::fs::read(dir.join("wow_classic_era_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert_eq!(config.build_uid(), Some("wow_classic_era"));

    let data = std::fs::read(dir.join("wow_classic_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert_eq!(config.build_uid(), Some("wow_classic"));

    let data = std::fs::read(dir.join("wow_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert_eq!(config.build_uid(), Some("wow"));
}

#[test]
fn build_config_cdn_hash_format() {
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Root should be a valid 32-char hex hash
        let root = config.root().unwrap();
        assert_eq!(root.len(), 32, "{name}: root should be 32 hex chars");
        assert!(
            root.chars().all(|c| c.is_ascii_hexdigit()),
            "{name}: root should be hex"
        );

        // Encoding info should have valid hashes
        let enc = config.encoding().unwrap();
        assert_eq!(
            enc.content_key.len(),
            32,
            "{name}: encoding content_key should be 32 hex chars"
        );
        assert!(
            enc.encoding_key.is_some(),
            "{name}: encoding should have encoding_key"
        );
    }
}

#[test]
fn build_config_cdn_size_fields() {
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let size = config.size();
        assert!(size.is_some(), "{name}: should have size field");

        let enc = config.encoding().unwrap();
        assert!(enc.size.is_some(), "{name}: encoding should have size");
        // Encoding sizes are at least several MB for all WoW products
        assert!(
            enc.size.unwrap() > 1_000_000,
            "{name}: encoding size should be > 1MB"
        );
    }
}

#[test]
fn build_config_cdn_feature_placeholder() {
    // All current WoW products have feature-placeholder = true
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert!(
            config.feature_placeholder(),
            "{name}: should have feature-placeholder = true"
        );
    }
}

#[test]
fn build_config_cdn_vfs_entries() {
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let vfs_root = config.vfs_root();
        assert!(vfs_root.is_some(), "{name}: should have vfs-root");

        let entries = config.vfs_entries();
        assert!(!entries.is_empty(), "{name}: should have VFS entries");

        // VFS entries should start at index 1
        assert_eq!(entries[0].0, 1, "{name}: first VFS entry should be index 1");

        // All entries should have content keys
        for (i, info) in &entries {
            assert_eq!(
                info.content_key.len(),
                32,
                "{name}: vfs-{i} content_key should be 32 hex chars"
            );
        }
    }
}

#[test]
fn build_config_cdn_known_vfs_counts() {
    let dir = fixtures_dir();

    let data = std::fs::read(dir.join("wow_classic_era_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert!(
        config.vfs_entries().len() > 200,
        "WoW Classic Era should have 200+ VFS entries, got {}",
        config.vfs_entries().len()
    );

    let data = std::fs::read(dir.join("wow_classic_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert!(
        config.vfs_entries().len() > 400,
        "WoW Classic should have 400+ VFS entries, got {}",
        config.vfs_entries().len()
    );

    let data = std::fs::read(dir.join("wow_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert!(
        config.vfs_entries().len() > 800,
        "WoW Retail should have 800+ VFS entries, got {}",
        config.vfs_entries().len()
    );
}

#[test]
fn build_config_cdn_partial_priority_retail_only() {
    let dir = fixtures_dir();

    // Classic Era and Classic should NOT have partial priority
    let data = std::fs::read(dir.join("wow_classic_era_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert!(
        config.build_partial_priority().is_empty(),
        "Classic Era should not have partial priority"
    );

    let data = std::fs::read(dir.join("wow_classic_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    assert!(
        config.build_partial_priority().is_empty(),
        "Classic should not have partial priority"
    );

    // Retail SHOULD have partial priority
    let data = std::fs::read(dir.join("wow_build_config.txt")).unwrap();
    let config = BuildConfig::parse(&data[..]).unwrap();
    let priorities = config.build_partial_priority();
    assert!(
        !priorities.is_empty(),
        "Retail should have partial priority entries"
    );

    // Each entry should have a valid hash key and positive priority
    for entry in &priorities {
        assert_eq!(
            entry.key.len(),
            32,
            "Priority key should be 32 hex chars: {}",
            entry.key
        );
        assert!(
            entry.priority > 0,
            "Priority value should be positive: {}",
            entry.priority
        );
    }
}

#[test]
fn build_config_cdn_validate() {
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let result = config.validate();
        assert!(
            result.is_ok(),
            "{name}: validation failed: {}",
            result.unwrap_err()
        );
    }
}

#[test]
fn build_config_cdn_round_trip() {
    for (name, data) in &fixture_files() {
        let original = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let rebuilt_data = original.build();

        let reparsed = BuildConfig::parse(&rebuilt_data[..])
            .unwrap_or_else(|e| panic!("Re-parse failed for {name}: {e}"));

        // Verify structural equality
        assert_eq!(original.root(), reparsed.root(), "{name}: root mismatch");
        assert_eq!(
            original.build_name(),
            reparsed.build_name(),
            "{name}: build-name mismatch"
        );
        assert_eq!(
            original.build_uid(),
            reparsed.build_uid(),
            "{name}: build-uid mismatch"
        );
        assert_eq!(
            original.encoding_key(),
            reparsed.encoding_key(),
            "{name}: encoding key mismatch"
        );
        assert_eq!(
            original.vfs_entries().len(),
            reparsed.vfs_entries().len(),
            "{name}: VFS entry count mismatch"
        );
        assert_eq!(
            original.feature_placeholder(),
            reparsed.feature_placeholder(),
            "{name}: feature-placeholder mismatch"
        );
        assert_eq!(
            original.build_partial_priority().len(),
            reparsed.build_partial_priority().len(),
            "{name}: partial priority count mismatch"
        );
    }
}

#[test]
fn build_config_cdn_dual_hash_format() {
    // Verify the dual-hash format: content_key encoding_key
    // All system files use this format with parallel *-size keys
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Install should have encoding key
        let installs = config.install();
        assert!(!installs.is_empty(), "{name}: should have install entries");
        assert!(
            installs[0].encoding_key.is_some(),
            "{name}: install should have encoding key"
        );

        // Download should have encoding key
        let downloads = config.download();
        assert!(
            !downloads.is_empty(),
            "{name}: should have download entries"
        );
        assert!(
            downloads[0].encoding_key.is_some(),
            "{name}: download should have encoding key"
        );

        // VFS root should have encoding key
        let vfs_root = config.vfs_root().unwrap();
        assert!(
            vfs_root.encoding_key.is_some(),
            "{name}: vfs-root should have encoding key"
        );
    }
}

#[test]
fn build_config_cdn_get_raw() {
    // Verify the generic get() accessor works for any key
    for (name, data) in &fixture_files() {
        let config = BuildConfig::parse(&data[..])
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let root_values = config.get("root");
        assert!(root_values.is_some(), "{name}: get(root) should work");
        assert_eq!(
            root_values.unwrap().len(),
            1,
            "{name}: root should have 1 value"
        );

        let encoding_values = config.get("encoding");
        assert!(
            encoding_values.is_some(),
            "{name}: get(encoding) should work"
        );
        assert_eq!(
            encoding_values.unwrap().len(),
            2,
            "{name}: encoding should have 2 values"
        );
    }
}

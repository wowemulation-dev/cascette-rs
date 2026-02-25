#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for TVFS parsing using real CDN data
//!
//! Tests parse real TVFS manifest files downloaded from Blizzard CDN
//! for WoW Retail, WoW Classic, and WoW Classic Era.

use cascette_formats::CascFormat;
use cascette_formats::tvfs::TvfsFile;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/tvfs")
        .leak()
}

/// Load all fixture TVFS files (decompressed .bin only)
fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = fixtures_dir();
    let files = [
        ("wow_retail", "wow_dbd6a1911a9dd025.bin"),
        ("wow_classic", "wow_classic_cbd15a9f67c4d28d.bin"),
        ("wow_classic_era", "wow_classic_era_04ca19154f0c48b1.bin"),
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
fn tvfs_cdn_parse_all() {
    let fixtures = fixture_files();
    assert!(!fixtures.is_empty(), "Should have fixture files");

    for (name, data) in &fixtures {
        let result = TvfsFile::parse(data);
        assert!(
            result.is_ok(),
            "Failed to parse {name}: {}",
            result.unwrap_err()
        );
        let tvfs = result.unwrap();
        println!(
            "{name}: {} files, {} vfs entries, {} container entries",
            tvfs.path_table.files.len(),
            tvfs.vfs_table.entries.len(),
            tvfs.container_table.entries.len(),
        );
    }
}

#[test]
fn tvfs_cdn_header_fields() {
    for (name, data) in &fixture_files() {
        let tvfs = TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert_eq!(tvfs.header.format_version, 1, "{name}: version");
        assert_eq!(tvfs.header.header_size, 46, "{name}: header_size");
        assert_eq!(tvfs.header.ekey_size, 9, "{name}: ekey_size");
        assert_eq!(tvfs.header.pkey_size, 9, "{name}: pkey_size");
        assert_eq!(tvfs.header.flags, 0x07, "{name}: flags should be 0x07");
        assert!(
            tvfs.header.includes_content_keys(),
            "{name}: should include content keys"
        );
        assert!(
            tvfs.header.has_encoding_spec(),
            "{name}: should have encoding spec"
        );
        assert!(
            tvfs.header.has_patch_support(),
            "{name}: should have patch support"
        );
    }
}

#[test]
fn tvfs_cdn_est_table() {
    for (name, data) in &fixture_files() {
        let tvfs = TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let est = tvfs
            .est_table
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: should have EST table"));

        // All CDN files have 2 ESpec entries
        assert_eq!(est.specs.len(), 2, "{name}: EST entry count");
        assert_eq!(est.specs[0], "b:256K*=z", "{name}: first ESpec");
        assert_eq!(est.specs[1], "b:{256K*=z}", "{name}: second ESpec");
    }
}

#[test]
fn tvfs_cdn_table_offsets_valid() {
    for (name, data) in &fixture_files() {
        let tvfs = TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));
        let h = &tvfs.header;
        let len = data.len() as u32;

        // All table ranges should fit within the file
        assert!(
            h.path_table_offset + h.path_table_size <= len,
            "{name}: path table out of bounds"
        );
        assert!(
            h.vfs_table_offset + h.vfs_table_size <= len,
            "{name}: vfs table out of bounds"
        );
        assert!(
            h.cft_table_offset + h.cft_table_size <= len,
            "{name}: cft table out of bounds"
        );

        if let (Some(est_off), Some(est_sz)) = (h.est_table_offset, h.est_table_size) {
            assert!(est_off + est_sz <= len, "{name}: est table out of bounds");
        }
    }
}

#[test]
fn tvfs_cdn_known_entry_counts() {
    let dir = fixtures_dir();

    // WoW Retail
    let data = std::fs::read(dir.join("wow_dbd6a1911a9dd025.bin")).unwrap();
    let tvfs = TvfsFile::parse(&data).unwrap();
    assert!(
        tvfs.path_table.files.len() > 800,
        "WoW Retail should have 800+ file entries, got {}",
        tvfs.path_table.files.len()
    );

    // WoW Classic
    let data = std::fs::read(dir.join("wow_classic_cbd15a9f67c4d28d.bin")).unwrap();
    let tvfs = TvfsFile::parse(&data).unwrap();
    assert!(
        tvfs.path_table.files.len() > 400,
        "WoW Classic should have 400+ file entries, got {}",
        tvfs.path_table.files.len()
    );

    // WoW Classic Era
    let data = std::fs::read(dir.join("wow_classic_era_04ca19154f0c48b1.bin")).unwrap();
    let tvfs = TvfsFile::parse(&data).unwrap();
    assert!(
        tvfs.path_table.files.len() > 200,
        "WoW Classic Era should have 200+ file entries, got {}",
        tvfs.path_table.files.len()
    );
}

#[test]
fn tvfs_cdn_round_trip() {
    for (name, data) in &fixture_files() {
        let original =
            TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let rebuilt_data = original
            .build()
            .unwrap_or_else(|e| panic!("Build failed for {name}: {e}"));

        // Re-parse the rebuilt data
        let reparsed = TvfsFile::parse(&rebuilt_data)
            .unwrap_or_else(|e| panic!("Re-parse failed for {name}: {e}"));

        // Verify structural equality
        assert_eq!(
            original.header.format_version, reparsed.header.format_version,
            "{name}: version mismatch"
        );
        assert_eq!(
            original.header.flags, reparsed.header.flags,
            "{name}: flags mismatch"
        );
        assert_eq!(
            original.path_table.files.len(),
            reparsed.path_table.files.len(),
            "{name}: file entry count mismatch"
        );
        assert_eq!(
            original.container_table.entries.len(),
            reparsed.container_table.entries.len(),
            "{name}: container entry count mismatch"
        );
    }
}

#[test]
fn tvfs_cdn_blte_load() {
    let dir = fixtures_dir();
    let blte_files = [
        ("wow_retail", "wow_dbd6a1911a9dd025.blte"),
        ("wow_classic", "wow_classic_cbd15a9f67c4d28d.blte"),
        ("wow_classic_era", "wow_classic_era_04ca19154f0c48b1.blte"),
    ];

    for (name, filename) in &blte_files {
        let path = dir.join(filename);
        let blte_data = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

        let result = TvfsFile::load_from_blte(&blte_data);
        assert!(
            result.is_ok(),
            "Failed to load BLTE for {name}: {}",
            result.unwrap_err()
        );
        let tvfs = result.unwrap();
        assert_eq!(tvfs.header.format_version, 1, "{name}: version from BLTE");
        assert!(
            !tvfs.path_table.files.is_empty(),
            "{name}: should have file entries from BLTE"
        );
    }
}

#[test]
fn tvfs_cdn_file_paths_nonempty() {
    for (name, data) in &fixture_files() {
        let tvfs = TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // All file entries should have non-empty paths
        for file in &tvfs.path_table.files {
            assert!(!file.path.is_empty(), "{name}: file entry has empty path");
        }

        // Print first few paths for verification
        for file in tvfs.path_table.files.iter().take(5) {
            println!("{name}: {}", file.path);
        }
    }
}

#[test]
fn tvfs_cdn_vfs_spans_reference_valid_cft() {
    for (name, data) in &fixture_files() {
        let tvfs = TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let cft_size = tvfs.header.cft_table_size;

        for (i, vfs_entry) in tvfs.vfs_table.entries.iter().enumerate() {
            for (j, span) in vfs_entry.spans.iter().enumerate() {
                assert!(
                    span.cft_offset < cft_size,
                    "{name}: VFS entry {i} span {j} CFT offset {} >= CFT size {}",
                    span.cft_offset,
                    cft_size
                );
            }
        }
    }
}

#[test]
fn tvfs_cdn_cft_entry_size() {
    // With flags 0x07 (INCLUDE_CKEY | ENCODING_SPEC | PATCH_SUPPORT),
    // ekey=9, pkey=9, entry should be:
    // EKey(9) + EncodedSize(4) + CKey(9) + EstOffs(1) + CftOffs(2) = 25
    // (EstOffsSize=1 because EST is small, CftOffsSize=2 because CFT < 0xFFFF)
    for (name, data) in &fixture_files() {
        let tvfs = TvfsFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let entry_size = tvfs.header.cft_entry_size();
        let cft_size = tvfs.header.cft_table_size as usize;
        let expected_count = cft_size / entry_size;

        assert_eq!(
            tvfs.container_table.entries.len(),
            expected_count,
            "{name}: CFT entry count mismatch (entry_size={entry_size}, cft_size={cft_size})"
        );
    }
}

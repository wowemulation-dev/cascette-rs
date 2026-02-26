#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for Patch Archive parsing using real CDN data
//!
//! Tests parse real patch manifest files downloaded from Blizzard CDN
//! for WoW Classic Era, WoW Retail, and StarCraft 2. Validates parsing,
//! block structure, extended header, sort order, and round-trip building.

use cascette_formats::CascFormat;
use cascette_formats::patch_archive::PatchArchive;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/patch_archive")
        .leak()
}

/// Load all fixture patch manifest files
fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = fixtures_dir();
    let files = [
        ("wow_classic_era", "e3fffe04f64007852408b86e44d91e5a.bin"),
        ("wow_retail", "aaad2399821319140599c508abd54c9c.bin"),
        ("starcraft2", "071290388e1f3b898157c372f03bc435.bin"),
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

// --- Parse all CDN patch manifests ---

#[test]
fn patch_archive_cdn_parse_all() {
    let fixtures = fixture_files();
    assert!(!fixtures.is_empty(), "Should have fixture files");

    for (name, data) in &fixtures {
        let result = PatchArchive::parse(data);
        assert!(
            result.is_ok(),
            "Failed to parse {name}: {}",
            result.unwrap_err()
        );
        let archive = result.unwrap();
        assert!(!archive.blocks.is_empty(), "{name}: should have blocks");
        println!(
            "{name}: {} blocks, {} file entries",
            archive.blocks.len(),
            archive.total_file_entries()
        );
    }
}

// --- Header fields ---

#[test]
fn patch_archive_cdn_header_fields() {
    for (name, data) in &fixture_files() {
        let archive =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert_eq!(archive.header.version, 2, "{name}: version");
        assert_eq!(archive.header.file_key_size, 16, "{name}: file_key_size");
        assert_eq!(archive.header.old_key_size, 16, "{name}: old_key_size");
        assert_eq!(archive.header.patch_key_size, 16, "{name}: patch_key_size");
        assert_eq!(
            archive.header.block_size_bits, 16,
            "{name}: block_size_bits"
        );
        assert_eq!(archive.header.flags, 0x02, "{name}: flags should be 0x02");
        assert!(
            archive.header.has_extended_header(),
            "{name}: should have extended header"
        );
    }
}

// --- Extended header (encoding info) ---

#[test]
fn patch_archive_cdn_encoding_info() {
    for (name, data) in &fixture_files() {
        let archive =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let info = archive
            .encoding_info
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: should have encoding info"));

        // All should have non-zero keys
        assert_ne!(info.encoding_ckey, [0u8; 16], "{name}: encoding CKey");
        assert_ne!(info.encoding_ekey, [0u8; 16], "{name}: encoding EKey");

        // Sizes should be non-zero
        assert!(info.decoded_size > 0, "{name}: decoded_size");
        assert!(info.encoded_size > 0, "{name}: encoded_size");
        assert!(
            info.encoded_size <= info.decoded_size,
            "{name}: encoded_size should be <= decoded_size"
        );

        // ESpec should be a valid block table spec
        assert!(!info.espec.is_empty(), "{name}: espec should not be empty");
        println!("{name}: espec=\"{}\"", info.espec);
    }
}

// --- Known entry counts ---

#[test]
fn patch_archive_cdn_known_entry_counts() {
    let dir = fixtures_dir();

    // WoW Classic Era
    let data = std::fs::read(dir.join("e3fffe04f64007852408b86e44d91e5a.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();
    assert_eq!(archive.total_file_entries(), 50, "WoW CE file entry count");

    // WoW Retail
    let data = std::fs::read(dir.join("aaad2399821319140599c508abd54c9c.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();
    assert_eq!(
        archive.total_file_entries(),
        112,
        "WoW Retail file entry count"
    );

    // StarCraft 2
    let data = std::fs::read(dir.join("071290388e1f3b898157c372f03bc435.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();
    assert_eq!(archive.total_file_entries(), 19, "SC2 file entry count");
}

// --- Block sort validation ---

#[test]
fn patch_archive_cdn_blocks_sorted() {
    for (name, data) in &fixture_files() {
        let archive =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert!(
            archive.validate_block_sort_order().is_ok(),
            "{name}: blocks should be sorted by CKey"
        );
    }
}

// --- File entries within blocks have patches ---

#[test]
fn patch_archive_cdn_entries_have_patches() {
    for (name, data) in &fixture_files() {
        let archive =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        for (bi, block) in archive.blocks.iter().enumerate() {
            for (ei, entry) in block.file_entries.iter().enumerate() {
                assert!(
                    !entry.patches.is_empty(),
                    "{name}: block {bi} entry {ei} should have at least one patch"
                );
                assert_ne!(
                    entry.target_ckey, [0u8; 16],
                    "{name}: block {bi} entry {ei} target CKey should be non-zero"
                );
            }
        }
    }
}

// --- Decoded sizes are reasonable ---

#[test]
fn patch_archive_cdn_decoded_sizes() {
    for (name, data) in &fixture_files() {
        let archive =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        for entry in archive.all_file_entries() {
            // decoded_size stored as uint40, max ~1 TB
            assert!(
                entry.decoded_size <= 0xFF_FFFF_FFFF,
                "{name}: decoded_size {0} exceeds uint40 range",
                entry.decoded_size
            );

            for patch in &entry.patches {
                assert!(
                    patch.source_decoded_size <= 0xFF_FFFF_FFFF,
                    "{name}: source decoded_size exceeds uint40 range"
                );
                assert!(
                    patch.patch_size > 0,
                    "{name}: patch size should be non-zero"
                );
            }
        }
    }
}

// --- Flatten entries ---

#[test]
fn patch_archive_cdn_flatten_entries() {
    for (name, data) in &fixture_files() {
        let archive =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let flat = archive.flatten_entries();

        // Total flat entries should be sum of all patches across all file entries
        let expected_patches: usize = archive.all_file_entries().map(|e| e.patches.len()).sum();
        assert_eq!(
            flat.len(),
            expected_patches,
            "{name}: flat entry count should match total patches"
        );

        // Each flat entry should have non-zero keys
        for entry in &flat {
            assert_ne!(entry.old_content_key, [0u8; 16]);
            assert_ne!(entry.new_content_key, [0u8; 16]);
            assert_ne!(entry.patch_encoding_key, [0u8; 16]);
        }
    }
}

// --- Header hash verification ---

#[test]
fn patch_archive_cdn_header_hash_matches_filename() {
    let dir = fixtures_dir();
    let files = [
        (
            "wow_classic_era",
            "e3fffe04f64007852408b86e44d91e5a",
            "e3fffe04f64007852408b86e44d91e5a.bin",
        ),
        (
            "wow_retail",
            "aaad2399821319140599c508abd54c9c",
            "aaad2399821319140599c508abd54c9c.bin",
        ),
        (
            "starcraft2",
            "071290388e1f3b898157c372f03bc435",
            "071290388e1f3b898157c372f03bc435.bin",
        ),
    ];

    for (name, expected_hex, filename) in &files {
        let data = std::fs::read(dir.join(filename)).unwrap();
        let archive =
            PatchArchive::parse(&data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Decode the expected content key from the filename
        let expected_bytes: Vec<u8> = (0..32)
            .step_by(2)
            .map(|i| u8::from_str_radix(&expected_hex[i..i + 2], 16).unwrap())
            .collect();
        let mut expected = [0u8; 16];
        expected.copy_from_slice(&expected_bytes);

        // Verify the header hash matches the CDN content key
        assert!(
            archive.verify_header_hash(&data, &expected).is_ok(),
            "{name}: header hash should match CDN content key {expected_hex}"
        );
    }
}

#[test]
fn patch_archive_cdn_header_hash_rejects_wrong_key() {
    let dir = fixtures_dir();
    let data = std::fs::read(dir.join("e3fffe04f64007852408b86e44d91e5a.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();

    let wrong_key = [0xFFu8; 16];
    let result = archive.verify_header_hash(&data, &wrong_key);
    assert!(result.is_err(), "Should reject wrong content key");
}

#[test]
fn patch_archive_cdn_header_region_sizes() {
    let dir = fixtures_dir();

    // WoW CE: 10 + (2*16 + 9 + espec_len) + 1*(16+20) = 142
    let data = std::fs::read(dir.join("e3fffe04f64007852408b86e44d91e5a.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();
    assert_eq!(archive.header_region_size(), 142, "WoW CE header region");

    // WoW Retail: 10 + (2*16 + 9 + espec_len) + 1*(16+20) = 151
    let data = std::fs::read(dir.join("aaad2399821319140599c508abd54c9c.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();
    assert_eq!(
        archive.header_region_size(),
        151,
        "WoW Retail header region"
    );

    // SC2: 10 + (2*16 + 9 + espec_len) + 1*(16+20) = 148
    let data = std::fs::read(dir.join("071290388e1f3b898157c372f03bc435.bin")).unwrap();
    let archive = PatchArchive::parse(&data).unwrap();
    assert_eq!(archive.header_region_size(), 148, "SC2 header region");
}

// --- Round-trip: parse -> build -> parse ---

#[test]
fn patch_archive_cdn_round_trip() {
    for (name, data) in &fixture_files() {
        let original =
            PatchArchive::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Rebuild from parsed data
        let rebuilt_data = original
            .build()
            .unwrap_or_else(|e| panic!("Build failed for {name}: {e}"));

        // Re-parse the rebuilt data
        let reparsed = PatchArchive::parse(&rebuilt_data)
            .unwrap_or_else(|e| panic!("Re-parse failed for {name}: {e}"));

        // Verify entry counts match
        assert_eq!(
            original.total_file_entries(),
            reparsed.total_file_entries(),
            "{name}: file entry count should match after round-trip"
        );

        // Verify all file entries match
        let orig_entries: Vec<_> = original.all_file_entries().collect();
        let repr_entries: Vec<_> = reparsed.all_file_entries().collect();

        for (i, (orig, repr)) in orig_entries.iter().zip(repr_entries.iter()).enumerate() {
            assert_eq!(
                orig.target_ckey, repr.target_ckey,
                "{name}: entry {i} target CKey mismatch"
            );
            assert_eq!(
                orig.decoded_size, repr.decoded_size,
                "{name}: entry {i} decoded size mismatch"
            );
            assert_eq!(
                orig.patches.len(),
                repr.patches.len(),
                "{name}: entry {i} patch count mismatch"
            );

            for (pi, (op, rp)) in orig.patches.iter().zip(repr.patches.iter()).enumerate() {
                assert_eq!(
                    op.source_ekey, rp.source_ekey,
                    "{name}: entry {i} patch {pi} source EKey mismatch"
                );
                assert_eq!(
                    op.source_decoded_size, rp.source_decoded_size,
                    "{name}: entry {i} patch {pi} source decoded size mismatch"
                );
                assert_eq!(
                    op.patch_ekey, rp.patch_ekey,
                    "{name}: entry {i} patch {pi} patch EKey mismatch"
                );
                assert_eq!(
                    op.patch_size, rp.patch_size,
                    "{name}: entry {i} patch {pi} patch size mismatch"
                );
                assert_eq!(
                    op.patch_index, rp.patch_index,
                    "{name}: entry {i} patch {pi} patch index mismatch"
                );
            }
        }

        // Verify encoding info matches
        match (&original.encoding_info, &reparsed.encoding_info) {
            (Some(orig), Some(repr)) => {
                assert_eq!(
                    orig.encoding_ckey, repr.encoding_ckey,
                    "{name}: encoding CKey"
                );
                assert_eq!(
                    orig.encoding_ekey, repr.encoding_ekey,
                    "{name}: encoding EKey"
                );
                assert_eq!(orig.decoded_size, repr.decoded_size, "{name}: decoded size");
                assert_eq!(orig.encoded_size, repr.encoded_size, "{name}: encoded size");
                assert_eq!(orig.espec, repr.espec, "{name}: espec");
            }
            (None, None) => {}
            _ => panic!("{name}: encoding info presence mismatch after round-trip"),
        }
    }
}

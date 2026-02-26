#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for Patch Index parsing using real CDN data
//!
//! Tests parse real patch index files downloaded from Blizzard CDN
//! for WoW Classic Era, WoW Classic, and WoW Retail. Validates parsing,
//! header fields, entry structure, block consistency, and round-trip building.

use cascette_formats::CascFormat;
use cascette_formats::patch_index::PatchIndex;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/patch_index")
        .leak()
}

/// Load all fixture files
fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = fixtures_dir();
    let files = [
        ("wow_classic_era", "wow_classic_era_patch_index.bin"),
        ("wow_classic", "wow_classic_patch_index.bin"),
        ("wow_retail", "wow_retail_patch_index.bin"),
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

// --- Parse all fixtures ---

#[test]
fn patch_index_cdn_parse_all() {
    for (name, data) in &fixture_files() {
        let result = PatchIndex::parse(data);
        assert!(
            result.is_ok(),
            "Failed to parse {name}: {}",
            result.unwrap_err()
        );
        let index = result.unwrap();
        assert!(!index.entries.is_empty(), "{name}: should have entries");
        println!(
            "{name}: {} entries, key_size={}",
            index.entries.len(),
            index.key_size
        );
    }
}

// --- Header fields ---

#[test]
fn patch_index_cdn_header_fields() {
    for (name, data) in &fixture_files() {
        let index =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert_eq!(index.header.version, 1, "{name}: version");
        assert_eq!(
            index.header.data_size as usize,
            data.len(),
            "{name}: data_size matches file size"
        );
        assert_eq!(index.header.header_size, 43, "{name}: header_size");
        assert_eq!(index.key_size, 16, "{name}: key_size");

        // All fixtures have 3 blocks: type 1, type 2, type 8
        assert_eq!(index.header.blocks.len(), 3, "{name}: block count");
        assert_eq!(index.header.blocks[0].block_type, 1, "{name}: block 0 type");
        assert_eq!(index.header.blocks[1].block_type, 2, "{name}: block 1 type");
        assert_eq!(index.header.blocks[2].block_type, 8, "{name}: block 2 type");

        // Block 1 (config) is always 7 bytes
        assert_eq!(
            index.header.blocks[0].block_size, 7,
            "{name}: config block size"
        );
    }
}

// --- Known entry counts ---

#[test]
fn patch_index_cdn_known_entry_counts() {
    let dir = fixtures_dir();

    let data = std::fs::read(dir.join("wow_classic_era_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();
    assert_eq!(index.entries.len(), 112, "WoW CE entry count");

    let data = std::fs::read(dir.join("wow_classic_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();
    assert_eq!(index.entries.len(), 141, "WoW Classic entry count");

    let data = std::fs::read(dir.join("wow_retail_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();
    assert_eq!(index.entries.len(), 242, "WoW Retail entry count");
}

// --- Entry field validation ---

#[test]
fn patch_index_cdn_entry_keys_nonzero() {
    for (name, data) in &fixture_files() {
        let index =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        for (i, entry) in index.entries.iter().enumerate() {
            assert_ne!(
                entry.source_ekey, [0u8; 16],
                "{name}: entry {i} source_ekey is zero"
            );
            assert_ne!(
                entry.target_ekey, [0u8; 16],
                "{name}: entry {i} target_ekey is zero"
            );
            assert_ne!(
                entry.patch_ekey, [0u8; 16],
                "{name}: entry {i} patch_ekey is zero"
            );
        }
    }
}

#[test]
fn patch_index_cdn_entry_sizes_reasonable() {
    for (name, data) in &fixture_files() {
        let index =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        for (i, entry) in index.entries.iter().enumerate() {
            assert!(
                entry.source_size > 0,
                "{name}: entry {i} source_size is zero"
            );
            assert!(
                entry.target_size > 0,
                "{name}: entry {i} target_size is zero"
            );
            assert!(
                entry.encoded_size > 0,
                "{name}: entry {i} encoded_size is zero"
            );
            // encoded_size should be <= target_size (compressed <= decoded)
            assert!(
                entry.encoded_size <= entry.target_size,
                "{name}: entry {i} encoded_size ({}) > target_size ({})",
                entry.encoded_size,
                entry.target_size
            );
        }
    }
}

// --- Suffix offset is always 1 in known files ---

#[test]
fn patch_index_cdn_suffix_offset_value() {
    for (name, data) in &fixture_files() {
        let index =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        for (i, entry) in index.entries.iter().enumerate() {
            assert_eq!(
                entry.suffix_offset, 1,
                "{name}: entry {i} suffix_offset should be 1"
            );
        }
    }
}

// --- Unique patch keys ---

#[test]
fn patch_index_cdn_unique_patch_keys() {
    let dir = fixtures_dir();

    // WoW CE: 50 unique patch keys among 112 entries
    let data = std::fs::read(dir.join("wow_classic_era_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();
    let uniq = index.unique_patch_ekeys();
    assert_eq!(uniq.len(), 50, "WoW CE unique patch keys");

    // WoW Classic: 66 unique patch keys among 141 entries
    let data = std::fs::read(dir.join("wow_classic_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();
    let uniq = index.unique_patch_ekeys();
    assert_eq!(uniq.len(), 66, "WoW Classic unique patch keys");

    // WoW Retail: 112 unique patch keys among 242 entries
    let data = std::fs::read(dir.join("wow_retail_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();
    let uniq = index.unique_patch_ekeys();
    assert_eq!(uniq.len(), 112, "WoW Retail unique patch keys");
}

// --- Block 2 and Block 8 consistency ---

#[test]
fn patch_index_cdn_block2_block8_consistent() {
    for (name, data) in &fixture_files() {
        let index =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Parse block 8 separately
        let b8_idx = 2; // block type 8 is always the third block
        let b8_offset = index.header.block_offset(b8_idx) as usize;
        let b8_size = index.header.blocks[b8_idx].block_size as usize;
        let b8_data = &data[b8_offset..b8_offset + b8_size];

        let (b8_key_size, b8_entries) =
            cascette_formats::patch_index::parser::parse_block8(b8_data)
                .unwrap_or_else(|e| panic!("Block 8 parse failed for {name}: {e}"));

        assert_eq!(b8_key_size, index.key_size, "{name}: key_size mismatch");
        assert_eq!(
            b8_entries.len(),
            index.entries.len(),
            "{name}: entry count mismatch between block 2 and 8"
        );

        // Entries should be identical
        for (i, (b2, b8)) in index.entries.iter().zip(b8_entries.iter()).enumerate() {
            assert_eq!(b2, b8, "{name}: entry {i} differs between block 2 and 8");
        }
    }
}

// --- Query methods ---

#[test]
fn patch_index_cdn_query_by_source() {
    let dir = fixtures_dir();
    let data = std::fs::read(dir.join("wow_classic_era_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();

    // Use the first entry's source key for lookup
    let first_source = index.entries[0].source_ekey;
    let results = index.find_by_source_ekey(&first_source);
    assert!(
        !results.is_empty(),
        "Should find at least one entry by source key"
    );
    assert_eq!(results[0].source_ekey, first_source);
}

#[test]
fn patch_index_cdn_query_by_patch_key() {
    let dir = fixtures_dir();
    let data = std::fs::read(dir.join("wow_classic_era_patch_index.bin")).unwrap();
    let index = PatchIndex::parse(&data).unwrap();

    // Use the first entry's patch key for lookup
    let first_patch = index.entries[0].patch_ekey;
    let results = index.find_by_patch_ekey(&first_patch);
    assert!(
        !results.is_empty(),
        "Should find at least one entry by patch key"
    );
}

// --- Round-trip: parse -> build -> parse ---

#[test]
fn patch_index_cdn_round_trip() {
    for (name, data) in &fixture_files() {
        let original =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let rebuilt_data = original
            .build()
            .unwrap_or_else(|e| panic!("Build failed for {name}: {e}"));

        let reparsed = PatchIndex::parse(&rebuilt_data)
            .unwrap_or_else(|e| panic!("Re-parse failed for {name}: {e}"));

        assert_eq!(
            original.entries.len(),
            reparsed.entries.len(),
            "{name}: entry count mismatch after round-trip"
        );

        for (i, (orig, repr)) in original
            .entries
            .iter()
            .zip(reparsed.entries.iter())
            .enumerate()
        {
            assert_eq!(orig, repr, "{name}: entry {i} mismatch after round-trip");
        }

        assert_eq!(
            original.key_size, reparsed.key_size,
            "{name}: key_size mismatch"
        );
    }
}

// --- Block size consistency ---

#[test]
fn patch_index_cdn_block_sizes() {
    for (name, data) in &fixture_files() {
        let index =
            PatchIndex::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Block 2 size should be: 5 + entry_count * 61
        let expected_b2 = 5 + index.entries.len() * 61;
        assert_eq!(
            index.header.blocks[1].block_size as usize, expected_b2,
            "{name}: block 2 size"
        );

        // Block 8 size should be: 14 + entry_count * 61
        let expected_b8 = 14 + index.entries.len() * 61;
        assert_eq!(
            index.header.blocks[2].block_size as usize, expected_b8,
            "{name}: block 8 size"
        );

        // Total file size should equal header_size + sum of block sizes
        let total: usize = index.header.header_size as usize
            + index
                .header
                .blocks
                .iter()
                .map(|b| b.block_size as usize)
                .sum::<usize>();
        assert_eq!(total, data.len(), "{name}: total file size");
    }
}

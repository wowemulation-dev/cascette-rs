#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for CDN archive index parsing using real CDN data
//!
//! Tests parse real archive index files downloaded from Blizzard CDN
//! for WoW Classic Era and StarCraft 2. Validates parsing, footer
//! integrity, entry sorting, and round-trip building.

use cascette_formats::archive::{ArchiveIndex, ArchiveIndexBuilder};
use std::io::Cursor;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/archive")
        .leak()
}

/// Load all fixture index files
fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = fixtures_dir();
    let files = [
        (
            "wow_classic_era_large",
            "0017a402f556fbece46c38dc431a2c9b.index",
        ),
        (
            "wow_classic_era_small",
            "00b79cc0eebdd26437c7e92e57ac7f5c.index",
        ),
        ("starcraft2", "s2_00872b40344ef1a3dac4aff09588603c.index"),
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

// --- Parse all CDN archive indices ---

#[test]
fn archive_cdn_parse_all() {
    let fixtures = fixture_files();
    assert!(!fixtures.is_empty(), "Should have fixture files");

    for (name, data) in &fixtures {
        let result = ArchiveIndex::parse(&mut Cursor::new(data));
        assert!(
            result.is_ok(),
            "Failed to parse {name}: {}",
            result.unwrap_err()
        );
        let index = result.unwrap();
        assert!(!index.entries.is_empty(), "{name}: should have entries");
        println!(
            "{name}: {} entries, {} chunks, element_count={}",
            index.entries.len(),
            index.toc.len(),
            index.footer.element_count
        );
    }
}

// --- Footer validation ---

#[test]
fn archive_cdn_footer_integrity() {
    for (name, data) in &fixture_files() {
        let index = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Footer hash should validate
        assert!(
            index.footer.is_valid(),
            "{name}: footer hash should be valid"
        );

        // Format should validate
        assert!(
            index.footer.validate_format().is_ok(),
            "{name}: footer format should be valid"
        );

        // File size should match
        assert!(
            index.footer.validate_file_size(data.len() as u64).is_ok(),
            "{name}: file size should match footer prediction"
        );
    }
}

// --- Footer field values ---

#[test]
fn archive_cdn_footer_fields() {
    for (name, data) in &fixture_files() {
        let index = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));
        let footer = &index.footer;

        // All CDN indices use version 1
        assert_eq!(footer.version, 1, "{name}: version");
        // 4KB pages
        assert_eq!(footer.page_size_kb, 4, "{name}: page_size_kb");
        // 4-byte offsets (regular archives, not archive-groups)
        assert_eq!(footer.offset_bytes, 4, "{name}: offset_bytes");
        // 4-byte sizes
        assert_eq!(footer.size_bytes, 4, "{name}: size_bytes");
        // Full 16-byte keys on CDN
        assert_eq!(footer.ekey_length, 16, "{name}: ekey_length");
        // 8-byte footer hash
        assert_eq!(footer.footer_hash_bytes, 8, "{name}: footer_hash_bytes");
    }
}

// --- Element count represents entries, not chunks ---

#[test]
fn archive_cdn_element_count_is_entries() {
    for (name, data) in &fixture_files() {
        let index = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // element_count should match the number of parsed entries
        assert_eq!(
            index.footer.element_count as usize,
            index.entries.len(),
            "{name}: element_count should equal number of entries"
        );
    }
}

// --- Entries are sorted ---

#[test]
fn archive_cdn_entries_sorted() {
    for (name, data) in &fixture_files() {
        let index = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        for window in index.entries.windows(2) {
            assert!(
                window[0].encoding_key <= window[1].encoding_key,
                "{name}: entries should be sorted by encoding key"
            );
        }
    }
}

// --- TOC consistency ---

#[test]
fn archive_cdn_toc_consistency() {
    for (name, data) in &fixture_files() {
        let index = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let record_size = index.footer.ekey_length as usize
            + index.footer.size_bytes as usize
            + index.footer.offset_bytes as usize;
        let records_per_page = (index.footer.page_size_kb as usize * 1024) / record_size;
        let expected_chunks = index.entries.len().div_ceil(records_per_page);

        assert_eq!(
            index.toc.len(),
            expected_chunks,
            "{name}: TOC length should match expected chunk count"
        );

        // Each TOC key should be the last key in its chunk
        for (i, toc_key) in index.toc.iter().enumerate() {
            let chunk_end = ((i + 1) * records_per_page).min(index.entries.len());
            let last_entry = &index.entries[chunk_end - 1];
            assert_eq!(
                toc_key, &last_entry.encoding_key,
                "{name}: TOC key {i} should match last entry in chunk"
            );
        }
    }
}

// --- Round-trip: parse -> build -> parse ---

#[test]
fn archive_cdn_round_trip() {
    for (name, data) in &fixture_files() {
        let original = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Rebuild from parsed data
        let builder = ArchiveIndexBuilder::from_archive_index(&original);
        let mut output = Cursor::new(Vec::new());
        let _rebuilt = builder
            .build(&mut output)
            .unwrap_or_else(|e| panic!("Build failed for {name}: {e}"));

        // Re-parse the rebuilt data
        let rebuilt_data = output.into_inner();
        let reparsed = ArchiveIndex::parse(&mut Cursor::new(&rebuilt_data))
            .unwrap_or_else(|e| panic!("Re-parse failed for {name}: {e}"));

        // Verify entry count matches
        assert_eq!(
            original.entries.len(),
            reparsed.entries.len(),
            "{name}: entry count should match after round-trip"
        );

        // Verify all entries match
        for (i, (orig, repr)) in original
            .entries
            .iter()
            .zip(reparsed.entries.iter())
            .enumerate()
        {
            assert_eq!(
                orig.encoding_key, repr.encoding_key,
                "{name}: entry {i} key mismatch"
            );
            assert_eq!(orig.size, repr.size, "{name}: entry {i} size mismatch");
            assert_eq!(
                orig.offset, repr.offset,
                "{name}: entry {i} offset mismatch"
            );
        }

        // Verify footer fields match
        assert_eq!(
            original.footer.element_count, reparsed.footer.element_count,
            "{name}: element_count mismatch"
        );
    }
}

// --- Binary search works on CDN data ---

#[test]
fn archive_cdn_binary_search() {
    for (name, data) in &fixture_files() {
        let index = ArchiveIndex::parse(&mut Cursor::new(data))
            .unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Search for the first, middle, and last entries
        let test_positions = [0, index.entries.len() / 2, index.entries.len() - 1];
        for &pos in &test_positions {
            let key = &index.entries[pos].encoding_key;
            let found = index.find_entry(key);
            assert!(
                found.is_some(),
                "{name}: should find entry at position {pos}"
            );
            assert_eq!(
                found.unwrap().encoding_key,
                *key,
                "{name}: found entry key should match at position {pos}"
            );
        }

        // Search for a non-existent key
        let fake_key = vec![0xFF; index.footer.ekey_length as usize];
        // This might or might not find something depending on data
        // Just verify it doesn't panic
        let _ = index.find_entry(&fake_key);
    }
}

// --- Known entry counts from CDN ---

#[test]
fn archive_cdn_known_entry_counts() {
    let dir = fixtures_dir();

    // WoW Classic Era large archive
    let data = std::fs::read(dir.join("0017a402f556fbece46c38dc431a2c9b.index")).unwrap();
    let index = ArchiveIndex::parse(&mut Cursor::new(&data)).unwrap();
    assert_eq!(
        index.entries.len(),
        7060,
        "WoW CE large archive entry count"
    );

    // WoW Classic Era small archive
    let data = std::fs::read(dir.join("00b79cc0eebdd26437c7e92e57ac7f5c.index")).unwrap();
    let index = ArchiveIndex::parse(&mut Cursor::new(&data)).unwrap();
    assert_eq!(
        index.entries.len(),
        2062,
        "WoW CE small archive entry count"
    );

    // StarCraft 2 archive
    let data = std::fs::read(dir.join("s2_00872b40344ef1a3dac4aff09588603c.index")).unwrap();
    let index = ArchiveIndex::parse(&mut Cursor::new(&data)).unwrap();
    assert_eq!(index.entries.len(), 1555, "SC2 archive entry count");
}

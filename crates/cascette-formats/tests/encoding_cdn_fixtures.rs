#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for encoding file parsing using real CDN data
//!
//! Tests parse truncated encoding files downloaded from Blizzard CDN
//! for WoW Classic Era and WoW Classic. Files contain the first 2 CKey
//! and 2 EKey pages with patched headers.

use cascette_formats::encoding::{EncodingFile, EncodingHeader};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/encoding")
        .leak()
}

/// Load all fixture encoding files
fn fixture_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = fixtures_dir();
    let files = [
        ("wow_classic_era", "wow_classic_era_truncated.bin"),
        ("wow_classic", "wow_classic_truncated.bin"),
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
fn encoding_cdn_parse_all() {
    let fixtures = fixture_files();
    assert!(!fixtures.is_empty(), "Should have fixture files");

    for (name, data) in &fixtures {
        let result = EncodingFile::parse(data);
        assert!(
            result.is_ok(),
            "Failed to parse {name}: {}",
            result.unwrap_err()
        );
        let enc = result.unwrap();
        println!(
            "{name}: {} ckey entries across {} pages, {} ekey entries across {} pages, {} especs",
            enc.ckey_count(),
            enc.ckey_pages.len(),
            enc.ekey_count(),
            enc.ekey_pages.len(),
            enc.espec_table.entries.len(),
        );
    }
}

#[test]
fn encoding_cdn_header_fields() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let h = &enc.header;
        assert_eq!(h.magic, *b"EN", "{name}: magic");
        assert_eq!(h.version, 1, "{name}: version");
        assert_eq!(h.ckey_hash_size, 16, "{name}: ckey_hash_size");
        assert_eq!(h.ekey_hash_size, 16, "{name}: ekey_hash_size");
        assert_eq!(h.ckey_page_size_kb, 4, "{name}: ckey_page_size_kb");
        assert_eq!(h.ekey_page_size_kb, 4, "{name}: ekey_page_size_kb");
        assert_eq!(h.flags, 0, "{name}: flags must be 0");

        // Truncated fixtures have 2 pages each
        assert_eq!(h.ckey_page_count, 2, "{name}: ckey_page_count");
        assert_eq!(h.ekey_page_count, 2, "{name}: ekey_page_count");

        // Verify page size helpers
        assert_eq!(h.ckey_page_size(), 4096, "{name}: ckey_page_size bytes");
        assert_eq!(h.ekey_page_size(), 4096, "{name}: ekey_page_size bytes");
    }
}

#[test]
fn encoding_cdn_page_size_shift_encoding() {
    // Verify the page size encoding: raw u16 * 1024 == (hi << 8 | lo) << 10
    // This test validates the TODO item about page size doc comments
    let h = EncodingHeader::new();
    assert_eq!(h.ckey_page_size_kb, 4);
    assert_eq!(h.ckey_page_size(), 4 * 1024);
    assert_eq!(h.ckey_page_size(), (4u16 as usize) << 10);

    // Test with a different KB value
    let mut h2 = h;
    h2.ckey_page_size_kb = 8;
    assert_eq!(h2.ckey_page_size(), 8192);
    assert_eq!(h2.ckey_page_size(), (8u16 as usize) << 10);
}

#[test]
fn encoding_cdn_espec_table() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // ESpec table should have entries
        assert!(
            !enc.espec_table.entries.is_empty(),
            "{name}: espec table should not be empty"
        );

        // All entries should be valid UTF-8 and non-empty
        for (i, spec) in enc.espec_table.entries.iter().enumerate() {
            assert!(!spec.is_empty(), "{name}: espec[{i}] should not be empty");
            // ESpec strings follow a pattern like "b:{256K*=z}" or "n:{*=z}"
            assert!(
                spec.is_ascii(),
                "{name}: espec[{i}] should be ASCII: {spec}"
            );
        }

        println!(
            "{name}: {} espec entries, first: {:?}",
            enc.espec_table.entries.len(),
            enc.espec_table.entries.first()
        );
    }
}

#[test]
fn encoding_cdn_ckey_page_entries() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert_eq!(enc.ckey_pages.len(), 2, "{name}: should have 2 ckey pages");

        for (pi, page) in enc.ckey_pages.iter().enumerate() {
            assert!(
                !page.entries.is_empty(),
                "{name}: ckey page {pi} should have entries"
            );

            for (ei, entry) in page.entries.iter().enumerate() {
                // key_count must be >= 1 (zero is padding)
                assert!(
                    entry.key_count >= 1,
                    "{name}: ckey page {pi} entry {ei} key_count should be >= 1"
                );
                // encoding_keys length must match key_count
                assert_eq!(
                    entry.encoding_keys.len(),
                    entry.key_count as usize,
                    "{name}: ckey page {pi} entry {ei} encoding_keys length mismatch"
                );
                // file_size should fit in 40 bits
                assert!(
                    entry.file_size < (1u64 << 40),
                    "{name}: ckey page {pi} entry {ei} file_size exceeds 40 bits"
                );
            }
        }

        // First page should have entries sorted by content key
        // (Agent.exe requires this for binary search)
        let first_page = &enc.ckey_pages[0];
        for window in first_page.entries.windows(2) {
            assert!(
                window[0].content_key.as_bytes() <= window[1].content_key.as_bytes(),
                "{name}: ckey entries should be sorted within page"
            );
        }
    }
}

#[test]
fn encoding_cdn_ekey_page_entries() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        assert_eq!(enc.ekey_pages.len(), 2, "{name}: should have 2 ekey pages");

        for (pi, page) in enc.ekey_pages.iter().enumerate() {
            assert!(
                !page.entries.is_empty(),
                "{name}: ekey page {pi} should have entries"
            );

            for (ei, entry) in page.entries.iter().enumerate() {
                // espec_index should be a valid index into the table
                assert!(
                    (entry.espec_index as usize) < enc.espec_table.entries.len(),
                    "{name}: ekey page {pi} entry {ei} espec_index {} out of range (max {})",
                    entry.espec_index,
                    enc.espec_table.entries.len()
                );
                // file_size should fit in 40 bits
                assert!(
                    entry.file_size < (1u64 << 40),
                    "{name}: ekey page {pi} entry {ei} file_size exceeds 40 bits"
                );
            }
        }

        // EKey entries should be sorted within each page
        let first_page = &enc.ekey_pages[0];
        for window in first_page.entries.windows(2) {
            assert!(
                window[0].encoding_key.as_bytes() <= window[1].encoding_key.as_bytes(),
                "{name}: ekey entries should be sorted within page"
            );
        }
    }
}

#[test]
fn encoding_cdn_page_index_sorted() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // CKey index entries should be sorted by first_key
        for window in enc.ckey_index.windows(2) {
            assert!(
                window[0].first_key <= window[1].first_key,
                "{name}: ckey index should be sorted"
            );
        }

        // EKey index entries should be sorted by first_key
        for window in enc.ekey_index.windows(2) {
            assert!(
                window[0].first_key <= window[1].first_key,
                "{name}: ekey index should be sorted"
            );
        }
    }
}

#[test]
fn encoding_cdn_page_checksums_valid() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // CKey page checksums
        for (i, (index_entry, page)) in enc.ckey_index.iter().zip(&enc.ckey_pages).enumerate() {
            assert!(
                index_entry.verify(&page.original_data),
                "{name}: ckey page {i} checksum mismatch"
            );
        }

        // EKey page checksums
        for (i, (index_entry, page)) in enc.ekey_index.iter().zip(&enc.ekey_pages).enumerate() {
            assert!(
                index_entry.verify(&page.original_data),
                "{name}: ekey page {i} checksum mismatch"
            );
        }
    }
}

#[test]
fn encoding_cdn_round_trip() {
    for (name, data) in &fixture_files() {
        let original =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let rebuilt_data = original
            .build()
            .unwrap_or_else(|e| panic!("Build failed for {name}: {e}"));

        // Re-parse the rebuilt data
        let reparsed = EncodingFile::parse(&rebuilt_data)
            .unwrap_or_else(|e| panic!("Re-parse failed for {name}: {e}"));

        // Verify structural equality
        assert_eq!(
            original.header.version, reparsed.header.version,
            "{name}: version mismatch"
        );
        assert_eq!(
            original.header.ckey_page_count, reparsed.header.ckey_page_count,
            "{name}: ckey_page_count mismatch"
        );
        assert_eq!(
            original.header.ekey_page_count, reparsed.header.ekey_page_count,
            "{name}: ekey_page_count mismatch"
        );
        assert_eq!(
            original.ckey_count(),
            reparsed.ckey_count(),
            "{name}: ckey entry count mismatch"
        );
        assert_eq!(
            original.ekey_count(),
            reparsed.ekey_count(),
            "{name}: ekey entry count mismatch"
        );
        assert_eq!(
            original.espec_table.entries.len(),
            reparsed.espec_table.entries.len(),
            "{name}: espec entry count mismatch"
        );
    }
}

#[test]
fn encoding_cdn_round_trip_byte_exact() {
    // The encoding file build() uses original_data for pages, so the
    // output should be byte-identical to the input.
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let rebuilt = enc
            .build()
            .unwrap_or_else(|e| panic!("Build failed for {name}: {e}"));

        assert_eq!(
            data.len(),
            rebuilt.len(),
            "{name}: rebuilt size mismatch (original={}, rebuilt={})",
            data.len(),
            rebuilt.len()
        );
        assert_eq!(
            data,
            &rebuilt[..],
            "{name}: rebuilt data should be byte-identical"
        );
    }
}

#[test]
fn encoding_cdn_ckey_lookup() {
    // Verify that content key lookups work on real data
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Take the first entry from the first CKey page and look it up
        let first_entry = &enc.ckey_pages[0].entries[0];
        let result = enc.find_encoding(&first_entry.content_key);
        assert!(
            result.is_some(),
            "{name}: should find encoding for first ckey entry"
        );
        assert_eq!(
            result.unwrap(),
            first_entry.encoding_keys[0],
            "{name}: encoding key should match"
        );
    }
}

#[test]
fn encoding_cdn_ekey_lookup() {
    // Verify that encoding key lookups work on real data
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Take the first entry from the first EKey page and look it up
        let first_entry = &enc.ekey_pages[0].entries[0];
        let result = enc.find_espec(&first_entry.encoding_key);
        assert!(
            result.is_some(),
            "{name}: should find espec for first ekey entry"
        );

        // The espec should match what we'd get from the table directly
        let expected = enc.espec_table.get(first_entry.espec_index).unwrap();
        assert_eq!(
            result.unwrap(),
            expected,
            "{name}: espec should match table entry"
        );
    }
}

#[test]
fn encoding_cdn_batch_lookup() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        // Collect content keys from first page
        let ckeys: Vec<_> = enc.ckey_pages[0]
            .entries
            .iter()
            .take(10)
            .map(|e| e.content_key)
            .collect();

        let results = enc.batch_find_encodings(&ckeys);
        assert_eq!(results.len(), ckeys.len(), "{name}: result count mismatch");

        // All keys from the file should be found
        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_some(),
                "{name}: batch lookup should find ckey[{i}]"
            );
        }
    }
}

#[test]
fn encoding_cdn_data_size_calculation() {
    for (name, data) in &fixture_files() {
        let enc =
            EncodingFile::parse(data).unwrap_or_else(|e| panic!("Parse failed for {name}: {e}"));

        let calculated_size = enc.header.data_size();
        let trailing_len = enc.trailing_espec.as_ref().map_or(0, String::len);
        let expected_size = data.len() - trailing_len;

        assert_eq!(
            calculated_size, expected_size,
            "{name}: data_size() should match actual data (excluding trailing espec)"
        );
    }
}

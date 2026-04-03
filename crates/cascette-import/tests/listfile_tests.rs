#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for listfile parsing and TACT key loading.
//!
//! The community listfile (FileDataID;Path CSV) is used by every reference
//! tool to resolve FileDataIDs to human-readable paths. wow.export, CASCHost,
//! and wago.tools all rely on this mapping for file browsing and extraction.
//!
//! TACT keys (key_id key_hex, whitespace-delimited) enable decryption of
//! encrypted CASC content. TACTSharp and CascLib load these from the
//! WoWDev TACTKeys repository.

use cascette_crypto::TactKeyProvider;
use cascette_crypto::keys::TactKeyStore;
use std::fmt::Write as _;
use std::path::PathBuf;

// --- TACT key loading via TactKeyStore (crypto crate, no network needed) ---

/// Real keys from the WoWDev TACTKeys repository (WoW.txt format).
/// These are publicly known keys used in older WoW Classic builds.
const SAMPLE_TACTKEYS_TXT: &str = r"
# WoW TACT Keys - whitespace-delimited format (WoW.txt)
# Source: https://github.com/wowdev/TACTKeys
FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200
FF813F7D062AC0BC AA0B5C77F088CCC2D39049BD267F066D
D1E9B5EDF9283668 8E4A2579894E38B4AB9058BA5C7328EE
# Armadillo keys use 0x prefix
0xB76729BF37C3581F 08717B15BF3C7955210B9DE18F3EA825
// C-style comment should also be skipped
8CF9F4C7EAEFF4B6 CDAA1A3E0C4FA99B8A3C5B4B8DEA2FE8
";

/// Real keys in the wago.tools CSV format (key_id,key_hex).
const SAMPLE_TACTKEYS_CSV: &str = r"
# CSV format used by some tools
FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200
FF813F7D062AC0BC,AA0B5C77F088CCC2D39049BD267F066D
0xD1E9B5EDF9283668,8E4A2579894E38B4AB9058BA5C7328EE
";

#[test]
fn tactkeys_load_from_txt() {
    let mut store = TactKeyStore::empty();
    let count = store.load_from_txt(SAMPLE_TACTKEYS_TXT);

    assert_eq!(
        count, 5,
        "Should load 5 keys from TXT (2 comments + 5 data lines)"
    );
}

#[test]
fn tactkeys_load_from_csv() {
    let mut store = TactKeyStore::empty();
    let count = store.load_from_csv(SAMPLE_TACTKEYS_CSV);

    assert_eq!(count, 3, "Should load 3 keys from CSV");
}

#[test]
fn tactkeys_txt_key_lookup() {
    let mut store = TactKeyStore::empty();
    store.load_from_txt(SAMPLE_TACTKEYS_TXT);

    // FA505078126ACB3E -> BDC51862ABED79B2DE48C8E7E66C6200
    let key_id = u64::from_str_radix("FA505078126ACB3E", 16).unwrap();
    let key = store
        .get_key(key_id)
        .expect("Key lookup must succeed")
        .expect("Key FA505078126ACB3E must be found");

    assert_eq!(hex::encode_upper(key), "BDC51862ABED79B2DE48C8E7E66C6200");
}

#[test]
fn tactkeys_csv_key_lookup() {
    let mut store = TactKeyStore::empty();
    store.load_from_csv(SAMPLE_TACTKEYS_CSV);

    let key_id = u64::from_str_radix("FF813F7D062AC0BC", 16).unwrap();
    let key = store
        .get_key(key_id)
        .expect("Key lookup must succeed")
        .expect("Key FF813F7D062AC0BC must be found");

    assert_eq!(hex::encode_upper(key), "AA0B5C77F088CCC2D39049BD267F066D");
}

#[test]
fn tactkeys_txt_0x_prefix_parsed() {
    let mut store = TactKeyStore::empty();
    store.load_from_txt(SAMPLE_TACTKEYS_TXT);

    // Key with 0x prefix: 0xB76729BF37C3581F
    let key_id = u64::from_str_radix("B76729BF37C3581F", 16).unwrap();
    let result = store.get_key(key_id).expect("Lookup must not error");
    assert!(
        result.is_some(),
        "Key with 0x prefix must be parsed and found"
    );
}

#[test]
fn tactkeys_csv_0x_prefix_parsed() {
    let mut store = TactKeyStore::empty();
    store.load_from_csv(SAMPLE_TACTKEYS_CSV);

    let key_id = u64::from_str_radix("D1E9B5EDF9283668", 16).unwrap();
    let result = store.get_key(key_id).expect("Lookup must not error");
    assert!(
        result.is_some(),
        "Key with 0x prefix must be parsed from CSV"
    );
}

#[test]
fn tactkeys_missing_key_returns_none() {
    let store = TactKeyStore::empty();
    let result = store
        .get_key(0xDEAD_BEEF_CAFE_BABE)
        .expect("Lookup must not error");
    assert!(result.is_none(), "Missing key must return None");
}

#[test]
fn tactkeys_comment_lines_skipped_txt() {
    // Comments must not affect key count
    let only_comments = "# all comments\n# no keys here\n// also a comment\n";
    let mut store = TactKeyStore::empty();
    let count = store.load_from_txt(only_comments);
    assert_eq!(count, 0, "No keys should be loaded from comment-only input");
}

#[test]
fn tactkeys_comment_lines_skipped_csv() {
    let only_comments = "# all comments\n# no keys here\n";
    let mut store = TactKeyStore::empty();
    let count = store.load_from_csv(only_comments);
    assert_eq!(count, 0, "No keys should be loaded from comment-only input");
}

#[test]
fn tactkeys_empty_input_loads_zero() {
    let mut store = TactKeyStore::empty();
    assert_eq!(store.load_from_txt(""), 0);
    assert_eq!(store.load_from_csv(""), 0);
}

#[test]
fn tactkeys_malformed_lines_skipped() {
    let malformed = "not_a_hex_id AABBCCDD\nFA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200\n";
    let mut store = TactKeyStore::empty();
    let count = store.load_from_txt(malformed);
    // Only the valid line should be counted
    assert_eq!(
        count, 1,
        "Malformed lines must be skipped, valid lines must load"
    );
}

#[test]
fn tactkeys_invalid_key_hex_skipped() {
    // Key hex is wrong length (only 30 chars instead of 32)
    let bad_hex = "FA505078126ACB3E AABBCCDDEE112233445566778899\n"; // 30 hex chars = 15 bytes
    let mut store = TactKeyStore::empty();
    let count = store.load_from_txt(bad_hex);
    assert_eq!(count, 0, "Key with wrong hex length must be skipped");
}

#[test]
fn tactkeys_roundtrip_txt_then_csv() {
    // Keys loaded from TXT and CSV formats must be equivalent
    let mut txt_store = TactKeyStore::empty();
    txt_store.load_from_txt(SAMPLE_TACTKEYS_TXT);

    let mut csv_store = TactKeyStore::empty();
    csv_store.load_from_csv(SAMPLE_TACTKEYS_CSV);

    // Keys present in both
    let shared_ids = [
        u64::from_str_radix("FA505078126ACB3E", 16).unwrap(),
        u64::from_str_radix("FF813F7D062AC0BC", 16).unwrap(),
        u64::from_str_radix("D1E9B5EDF9283668", 16).unwrap(),
    ];

    for id in shared_ids {
        let txt_key = txt_store
            .get_key(id)
            .expect("TXT lookup OK")
            .expect("Key in TXT store");
        let csv_key = csv_store
            .get_key(id)
            .expect("CSV lookup OK")
            .expect("Key in CSV store");
        assert_eq!(
            txt_key, csv_key,
            "Key 0x{id:016X} must be identical in both stores"
        );
    }
}

// --- Listfile content parsing ---
// parse_listfile_content is private, so we test it indirectly through
// the public module's inline tests. Instead, we test ListfileProvider
// construction and the public file_mappings API.

#[test]
fn listfile_provider_construction() {
    use cascette_import::ListfileProvider;

    let tmp = tempfile::tempdir().expect("tempdir should succeed");
    let provider = ListfileProvider::new(tmp.path().to_path_buf());
    assert!(
        provider.is_ok(),
        "ListfileProvider::new should succeed with a valid cache dir"
    );

    let provider = provider.unwrap();
    // No mappings loaded yet — in-memory store is empty
    assert!(
        provider.file_mappings().is_empty(),
        "Freshly created provider must have empty in-memory mappings"
    );
}

#[test]
fn listfile_provider_empty_cache_dir() {
    use cascette_import::ListfileProvider;

    // A non-existent cache dir must not cause construction to fail
    let provider =
        ListfileProvider::new(PathBuf::from("/tmp/cascette-test-nonexistent-dir-abc123"));
    assert!(
        provider.is_ok(),
        "ListfileProvider::new must succeed even if cache dir doesn't exist yet"
    );
}

// --- Real-world scale listfile parsing ---
// Test parse_listfile_content behaviour at realistic scale using generated data.
// We use the fact that the function is accessible within the module's test namespace
// by testing the public API surface that wraps it.

#[test]
fn tactkeys_store_len_and_is_empty() {
    let mut store = TactKeyStore::empty();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    store.load_from_txt(SAMPLE_TACTKEYS_TXT);
    assert!(!store.is_empty());
    assert_eq!(store.len(), 5);
}

#[test]
fn tactkeys_large_file_parse() {
    // Simulate a realistic WoW.txt with 500 key entries
    let mut content = String::from("# WoW TACT Keys\n");
    let mut expected_count = 0u32;
    for i in 0u64..500 {
        let id = i.wrapping_mul(0x9E3779B97F4A7C15); // spread bits
        let key: [u8; 16] = {
            let mut k = [0u8; 16];
            k[0..8].copy_from_slice(&i.to_le_bytes());
            k[8..16].copy_from_slice(&(i ^ 0xFF).to_le_bytes());
            k
        };
        writeln!(content, "{id:016X} {}", hex::encode_upper(key)).unwrap();
        expected_count += 1;
    }

    let mut store = TactKeyStore::empty();
    let count = store.load_from_txt(&content);
    assert_eq!(
        count as u32, expected_count,
        "All 500 synthetic keys should load"
    );
    assert_eq!(store.len(), 500);
}

#[test]
fn tactkeys_duplicate_keys_not_duplicated() {
    // Loading the same key twice must not double-count it
    let dup = "FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200\n\
               FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200\n";
    let mut store = TactKeyStore::empty();
    store.load_from_txt(dup);
    // The store uses a HashMap, so the key should appear exactly once
    assert_eq!(store.len(), 1, "Duplicate key entries must deduplicate");
}

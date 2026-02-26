//! Integration tests for KeyringConfig using real CDN data
//!
//! These tests parse keyring config files fetched from Blizzard CDN to verify
//! that the parser handles real-world data correctly.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use cascette_formats::config::KeyringConfig;

/// Load a test fixture file from the test_fixtures/config directory
fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/test_fixtures/config/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read fixture {path}: {e}"))
}

// --- WoW Retail keyring (1 entry) ---

#[test]
fn wow_keyring_parses() {
    let data = load_fixture("wow_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    assert_eq!(config.len(), 1);
}

#[test]
fn wow_keyring_validates() {
    let data = load_fixture("wow_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    config.validate().unwrap();
}

#[test]
fn wow_keyring_entry_values() {
    let data = load_fixture("wow_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let entry = &config.entries()[0];
    assert_eq!(entry.key_id, "4eb4869f95f23b53");
    assert_eq!(entry.key_value, "c9316739348dcc033aa8112f9a3acf5d");
}

#[test]
fn wow_keyring_lookup_by_hex() {
    let data = load_fixture("wow_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    assert_eq!(
        config.get_key("4eb4869f95f23b53"),
        Some("c9316739348dcc033aa8112f9a3acf5d")
    );
}

#[test]
fn wow_keyring_lookup_by_u64() {
    let data = load_fixture("wow_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    assert_eq!(
        config.get_key_by_id(0x4eb4869f95f23b53),
        Some("c9316739348dcc033aa8112f9a3acf5d")
    );
}

#[test]
fn wow_keyring_round_trip() {
    let data = load_fixture("wow_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let rebuilt = config.build();
    let reparsed = KeyringConfig::parse(&rebuilt[..]).unwrap();
    assert_eq!(reparsed.len(), config.len());
    assert_eq!(reparsed.entries()[0].key_id, config.entries()[0].key_id);
    assert_eq!(
        reparsed.entries()[0].key_value,
        config.entries()[0].key_value
    );
}

// --- Overwatch 2 keyring (63 entries, largest observed) ---

#[test]
fn overwatch_keyring_parses() {
    let data = load_fixture("overwatch_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    assert_eq!(config.len(), 63);
}

#[test]
fn overwatch_keyring_validates() {
    let data = load_fixture("overwatch_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    config.validate().unwrap();
}

#[test]
fn overwatch_keyring_first_entry() {
    let data = load_fixture("overwatch_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let entry = &config.entries()[0];
    assert_eq!(entry.key_id, "1b3e4e1ecfb25877");
    assert_eq!(entry.key_value, "3de60d37c664723595f27c5cdbf08bfa");
}

#[test]
fn overwatch_keyring_last_entry() {
    let data = load_fixture("overwatch_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let entry = &config.entries()[62];
    assert_eq!(entry.key_id, "1aaca19f8ee10f4f");
    assert_eq!(entry.key_value, "89381c748f6531bbfcd97753d06cc3cd");
}

#[test]
fn overwatch_keyring_lookup_mid_entry() {
    let data = load_fixture("overwatch_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    // Entry from the middle of the file
    assert_eq!(
        config.get_key("534974e4f814d9e6"),
        Some("c8477c289dce66d9136507a33aa33301")
    );
}

#[test]
fn overwatch_keyring_round_trip() {
    let data = load_fixture("overwatch_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let rebuilt = config.build();
    let reparsed = KeyringConfig::parse(&rebuilt[..]).unwrap();
    assert_eq!(reparsed.len(), config.len());
    for (orig, rebuilt) in config.entries().iter().zip(reparsed.entries().iter()) {
        assert_eq!(orig.key_id, rebuilt.key_id);
        assert_eq!(orig.key_value, rebuilt.key_value);
    }
}

// --- Call of Duty (Odin) keyring (1 entry) ---

#[test]
fn odin_keyring_parses() {
    let data = load_fixture("odin_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    assert_eq!(config.len(), 1);
}

#[test]
fn odin_keyring_validates() {
    let data = load_fixture("odin_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    config.validate().unwrap();
}

#[test]
fn odin_keyring_entry_values() {
    let data = load_fixture("odin_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let entry = &config.entries()[0];
    assert_eq!(entry.key_id, "01281b858d1e75a4");
    assert_eq!(entry.key_value, "26aceda706eb9ad5cea68d1431e623d7");
}

#[test]
fn odin_keyring_round_trip() {
    let data = load_fixture("odin_keyring_config.txt");
    let config = KeyringConfig::parse(&data[..]).unwrap();
    let rebuilt = config.build();
    let reparsed = KeyringConfig::parse(&rebuilt[..]).unwrap();
    assert_eq!(reparsed.len(), config.len());
    assert_eq!(reparsed.entries()[0].key_id, config.entries()[0].key_id);
    assert_eq!(
        reparsed.entries()[0].key_value,
        config.entries()[0].key_value
    );
}

#![allow(clippy::expect_used, clippy::panic)]

//! TACT key management
//!
//! Demonstrates the TACT encryption key store used by CASC for content
//! decryption, including hardcoded keys, key loading from text formats,
//! and the UnifiedKeyStore trait abstraction.
//!
//! ```text
//! cargo run -p cascette-crypto --example tact_keyring
//! ```

use cascette_crypto::{TactKey, TactKeyProvider, TactKeyStore, UnifiedKeyStore};

fn main() {
    hardcoded_keys();
    key_lookup();
    empty_store_operations();
    load_from_csv();
    load_from_txt();
    unified_key_store();
}

fn hardcoded_keys() {
    println!("=== Hardcoded Key Store ===");

    let store = TactKeyStore::new();
    println!(
        "TactKeyStore::new() contains {} hardcoded keys",
        store.len()
    );

    // List a few key IDs
    let mut ids: Vec<u64> = store.iter().map(|k| k.id).collect();
    ids.sort_unstable();
    println!("Key IDs:");
    for id in &ids {
        let key = store.get(*id).expect("key should exist in store");
        println!("  0x{id:016X} -> {}", hex::encode_upper(key));
    }
    println!();
}

fn key_lookup() {
    println!("=== Key Lookup ===");

    let store = TactKeyStore::new();

    // Look up the Battle for Azeroth global encryption key
    let bfa_key_id: u64 = 0xFA50_5078_126A_CB3E;
    match store.get(bfa_key_id) {
        Some(key) => {
            println!("BfA key 0x{bfa_key_id:016X}: {}", hex::encode_upper(key));
        }
        None => {
            panic!("BfA key should be present in hardcoded store");
        }
    }

    // Missing key returns None
    let missing_id: u64 = 0x0000_0000_0000_0001;
    assert!(store.get(missing_id).is_none());
    println!("Missing key 0x{missing_id:016X}: None");
    println!();
}

fn empty_store_operations() {
    println!("=== Empty Store and Manual Key Addition ===");

    let mut store = TactKeyStore::empty();
    assert_eq!(store.len(), 0);
    println!("TactKeyStore::empty() has {} keys", store.len());

    // Add a key from raw bytes
    let key = TactKey::new(
        0x1234_5678_ABCD_EF00,
        [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ],
    );
    store.add(key);
    println!("After add: {} key(s)", store.len());

    // Add a key from hex
    let key_from_hex = TactKey::from_hex(0xAAAA_BBBB_CCCC_DDDD, "DEADBEEFCAFEBABE1122334455667788")
        .expect("hex key parsing should succeed");
    store.add(key_from_hex);
    println!("After second add: {} key(s)", store.len());

    // Retrieve and verify
    let retrieved = store.get(0x1234_5678_ABCD_EF00).expect("key should exist");
    println!(
        "Retrieved 0x1234567890ABCDEF: {}",
        hex::encode_upper(retrieved)
    );

    // Display format
    println!("TactKey display: {key_from_hex}");
    println!();
}

fn load_from_csv() {
    println!("=== Load Keys from CSV ===");

    // The CSV format uses comma-separated key_id and key_hex.
    // Lines starting with # are comments. Key IDs can be hex (0x prefix or
    // 16-char hex string) or decimal.
    let csv_content = "\
# TACT encryption keys (CSV format)
# Format: key_id,key_hex
FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200
0xFF813F7D062AC0BC,AA0B5C77F088CCC2D39049BD267F066D
D1E9B5EDF9283668,8E4A2579894E38B4AB9058BA5C7328EE
";

    let mut store = TactKeyStore::empty();
    let count = store.load_from_csv(csv_content);
    println!("Loaded {count} keys from CSV");
    println!("Store now has {} keys", store.len());

    // Verify a loaded key
    let bfa = store
        .get(0xFA50_5078_126A_CB3E)
        .expect("BfA key should be loaded");
    println!("BfA key from CSV: {}", hex::encode_upper(bfa));
    println!();
}

fn load_from_txt() {
    println!("=== Load Keys from TXT ===");

    // The TXT format uses whitespace-separated key_id and key_hex.
    // Lines starting with # or // are comments.
    let txt_content = "\
# TACT encryption keys (space-separated format)
// This format is simpler for manual editing
FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200
0xB76729641141CB34 9849D1AA7B1FD09819C5C66283A326EC
";

    let mut store = TactKeyStore::empty();
    let count = store.load_from_txt(txt_content);
    println!("Loaded {count} keys from TXT");
    println!("Store now has {} keys", store.len());

    // Verify
    let sl = store
        .get(0xB767_2964_1141_CB34)
        .expect("Shadowlands key should be loaded");
    println!("Shadowlands key from TXT: {}", hex::encode_upper(sl));
    println!();
}

fn unified_key_store() {
    println!("=== UnifiedKeyStore (TactKeyProvider trait) ===");

    // UnifiedKeyStore wraps any TactKeyProvider implementation, providing
    // a uniform interface regardless of backend (in-memory, database, keyring).
    let backend = TactKeyStore::new();
    let initial_count = backend.len();
    let mut unified = UnifiedKeyStore::new(backend);

    // Access through TactKeyProvider trait methods
    let count = unified.key_count().expect("key_count should succeed");
    println!("Key count via TactKeyProvider: {count}");
    assert_eq!(count, initial_count);

    // Look up through trait
    let bfa_key_id: u64 = 0xFA50_5078_126A_CB3E;
    let key = unified
        .get_key(bfa_key_id)
        .expect("get_key should succeed")
        .expect("BfA key should exist");
    println!("get_key(0x{bfa_key_id:016X}): {}", hex::encode_upper(key));

    // Check existence
    let exists = unified
        .contains_key(bfa_key_id)
        .expect("contains_key should succeed");
    println!("contains_key(0x{bfa_key_id:016X}): {exists}");

    // Add a key through trait
    let new_key = TactKey::new(0xDEAD_BEEF_CAFE_BABE, [0xFF; 16]);
    unified.add_key(new_key).expect("add_key should succeed");
    println!(
        "After add_key: {} keys",
        unified.key_count().expect("key_count should succeed")
    );

    // List all key IDs
    let mut ids = unified.list_key_ids().expect("list_key_ids should succeed");
    ids.sort_unstable();
    println!("Total key IDs listed: {}", ids.len());

    // Access underlying backend
    let backend_ref = unified.backend();
    println!("Backend len() matches: {}", backend_ref.len() == ids.len());

    // Remove through trait
    let removed = unified
        .remove_key(0xDEAD_BEEF_CAFE_BABE)
        .expect("remove_key should succeed");
    assert!(removed.is_some());
    println!(
        "After remove_key: {} keys",
        unified.key_count().expect("key_count should succeed")
    );
}

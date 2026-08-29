//! Demonstrates Armadillo key parsing and ChainedKeyProvider usage.
//!
//! Run with: `cargo run --example armadillo_keys -p cascette-crypto`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::uninlined_format_args
)]

use cascette_crypto::armadillo::{parse_ak_file, parse_hex_key, write_ak_file};
use cascette_crypto::{ChainedKeyProvider, TactKey, TactKeyProvider, TactKeyStore};

fn main() {
    // --- Hex key parsing ---
    println!("=== Hex Key Parsing ===");
    let hex_str = "0123456789abcdef0123456789abcdef";
    let key = parse_hex_key(hex_str).expect("valid hex key");
    println!("Parsed hex key: {}", hex::encode(key));

    // --- .ak file construction and parsing ---
    println!("\n=== .ak File Round-Trip ===");
    let original_key = [
        0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD,
        0xEF,
    ];
    let ak_data = write_ak_file(&original_key);
    println!(
        "Serialized .ak file ({} bytes): {}",
        ak_data.len(),
        hex::encode(ak_data)
    );

    let parsed_key = parse_ak_file(&ak_data).expect("valid .ak file");
    assert_eq!(original_key, parsed_key);
    println!("Round-trip verified: keys match");

    // --- ChainedKeyProvider ---
    println!("\n=== ChainedKeyProvider ===");
    let mut primary = TactKeyStore::empty();
    primary.add(TactKey::new(0x1111, [0xAA; 16]));

    let mut secondary = TactKeyStore::empty();
    secondary.add(TactKey::new(0x2222, [0xBB; 16]));
    secondary.add(TactKey::new(0x1111, [0xCC; 16])); // same ID, different value

    let mut chain = ChainedKeyProvider::new(Box::new(primary));
    chain.push(Box::new(secondary));

    // Primary wins for shared key
    let key = chain.get_key(0x1111).unwrap();
    println!(
        "Key 0x1111 (in both): {} (from primary)",
        hex::encode(key.unwrap())
    );

    // Falls through to secondary
    let key = chain.get_key(0x2222).unwrap();
    println!("Key 0x2222 (secondary only): {}", hex::encode(key.unwrap()));

    // Missing from all
    let key = chain.get_key(0x9999).unwrap();
    println!("Key 0x9999 (missing): {:?}", key);

    println!("\nTotal keys across chain: {}", chain.key_count().unwrap());
}

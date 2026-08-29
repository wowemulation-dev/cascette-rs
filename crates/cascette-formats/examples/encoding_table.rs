#![allow(clippy::expect_used, clippy::panic)]
//! Encoding file operations example
//!
//! Demonstrates building an encoding file with CKey and EKey entries,
//! serializing it, parsing it back, and performing lookups.
//!
//! ```text
//! cargo run -p cascette-formats --example encoding_table
//! ```

use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::encoding::{CKeyEntryData, EKeyEntryData, EncodingBuilder, EncodingFile};

fn main() {
    println!("=== Building Encoding File ===");

    let mut builder = EncodingBuilder::new();

    // Add 20 entries with different ESpec types
    let entries: Vec<(ContentKey, EncodingKey, &str)> = (0u8..20)
        .map(|i| {
            let ckey = ContentKey::from_bytes({
                let mut b = [0u8; 16];
                b[0] = i;
                b[15] = i.wrapping_mul(7);
                b
            });
            let ekey = EncodingKey::from_bytes({
                let mut b = [0u8; 16];
                b[0] = i.wrapping_add(100);
                b[15] = i.wrapping_mul(13);
                b
            });
            let espec = match i % 3 {
                0 => "z",
                1 => "n",
                _ => "b:{256K*=z}",
            };
            (ckey, ekey, espec)
        })
        .collect();

    for (i, (ckey, ekey, espec)) in entries.iter().enumerate() {
        builder.add_ckey_entry(CKeyEntryData {
            content_key: *ckey,
            file_size: 1024 * (i as u64 + 1),
            encoding_keys: vec![*ekey],
        });
        builder.add_ekey_entry(EKeyEntryData {
            encoding_key: *ekey,
            espec: espec.to_string(),
            file_size: 512 * (i as u64 + 1),
        });
    }

    println!("CKey entries added: {}", builder.ckey_count());
    println!("EKey entries added: {}", builder.ekey_count());

    // Build the encoding file
    let encoding = builder.build().expect("encoding build should succeed");

    println!();
    println!("=== Encoding File Structure ===");
    println!(
        "Magic:            {:?}",
        std::str::from_utf8(&encoding.header.magic)
    );
    println!("Version:          {}", encoding.header.version);
    println!("CKey hash size:   {} bytes", encoding.header.ckey_hash_size);
    println!("EKey hash size:   {} bytes", encoding.header.ekey_hash_size);
    println!("CKey page size:   {} KB", encoding.header.ckey_page_size_kb);
    println!("EKey page size:   {} KB", encoding.header.ekey_page_size_kb);
    println!("CKey pages:       {}", encoding.header.ckey_page_count);
    println!("EKey pages:       {}", encoding.header.ekey_page_count);
    println!(
        "ESpec block size: {} bytes",
        encoding.header.espec_block_size
    );
    println!("Total CKey entries: {}", encoding.ckey_count());
    println!("Total EKey entries: {}", encoding.ekey_count());

    println!();
    println!("=== ESpec Table ===");
    for (i, espec) in encoding.espec_table.entries.iter().enumerate() {
        println!("  [{i}] {espec}");
    }

    println!();
    println!("=== Serialization and Round-Trip ===");

    let serialized = encoding.build().expect("serialization should succeed");
    println!("Serialized size:  {} bytes", serialized.len());

    let parsed = EncodingFile::parse(&serialized).expect("parse should succeed");
    println!("Parsed CKey count: {}", parsed.ckey_count());
    println!("Parsed EKey count: {}", parsed.ekey_count());
    println!("ESpec entries:    {}", parsed.espec_table.entries.len());

    println!();
    println!("=== Content Key Lookups ===");

    // Look up specific content keys
    for i in [0usize, 5, 10, 15, 19] {
        let (ckey, expected_ekey, _) = &entries[i];
        let found = parsed.find_encoding(ckey);
        match found {
            Some(ekey) => {
                let matches = ekey == *expected_ekey;
                println!("  CKey[{i}] -> EKey found, matches expected: {matches}");
            }
            None => {
                println!("  CKey[{i}] -> not found");
            }
        }
    }

    // Look up a key that does not exist
    let missing = ContentKey::from_bytes([0xFF; 16]);
    let not_found = parsed.find_encoding(&missing);
    println!("  Missing key -> found: {}", not_found.is_some());

    println!();
    println!("=== ESpec Lookups ===");

    for i in [0usize, 1, 2] {
        let (_, ekey, expected_espec) = &entries[i];
        let found = parsed.find_espec(ekey);
        match found {
            Some(espec) => {
                println!("  EKey[{i}] -> espec=\"{espec}\" (expected: \"{expected_espec}\")");
            }
            None => {
                println!("  EKey[{i}] -> espec not found");
            }
        }
    }

    println!();
    println!("=== Batch Lookups ===");

    let batch_ckeys: Vec<ContentKey> = entries.iter().take(5).map(|(ck, _, _)| *ck).collect();
    let batch_results = parsed.batch_find_encodings(&batch_ckeys);
    println!("Batch lookup of {} keys:", batch_ckeys.len());
    for (i, result) in batch_results.iter().enumerate() {
        println!("  [{i}] found: {}", result.is_some());
    }

    println!();
    println!("=== Trailing ESpec Generation ===");

    let trailing = EncodingBuilder::generate_trailing_espec(&encoding);
    println!("Trailing ESpec:   {trailing}");

    println!();
    println!("=== Modify and Rebuild ===");

    // Create a builder from the existing encoding file, add an entry, rebuild
    let mut modified_builder = EncodingBuilder::from_encoding_file(&parsed);
    println!(
        "Existing entries: CKey={} EKey={}",
        modified_builder.ckey_count(),
        modified_builder.ekey_count()
    );

    let new_ckey = ContentKey::from_bytes([0xAA; 16]);
    let new_ekey = EncodingKey::from_bytes([0xBB; 16]);
    modified_builder.add_ckey_entry(CKeyEntryData {
        content_key: new_ckey,
        file_size: 99999,
        encoding_keys: vec![new_ekey],
    });
    modified_builder.add_ekey_entry(EKeyEntryData {
        encoding_key: new_ekey,
        espec: "z".to_string(),
        file_size: 88888,
    });

    let modified = modified_builder
        .build()
        .expect("modified build should succeed");
    println!(
        "After adding:     CKey={} EKey={}",
        modified.ckey_count(),
        modified.ekey_count()
    );

    let found_new = modified.find_encoding(&new_ckey);
    println!("New entry found:  {}", found_new.is_some());
}

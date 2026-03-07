#![allow(clippy::expect_used, clippy::panic)]

//! CASC cryptographic primitives
//!
//! Demonstrates MD5 content/encoding keys, Jenkins96 hashing for root file
//! name lookups, and IDX bucket calculation from truncated encoding keys.
//!
//! ```text
//! cargo run -p cascette-crypto --example crypto_primitives
//! ```

use cascette_crypto::{
    jenkins::{Jenkins96, hashlittle, hashlittle2},
    md5::{ContentKey, EncodingKey, FileDataId},
};

fn main() {
    content_key_demo();
    encoding_key_demo();
    jenkins96_demo();
    hashlittle_demo();
    hashlittle2_demo();
    idx_bucket_demo();
    file_data_id_demo();
}

fn content_key_demo() {
    println!("=== ContentKey (MD5 of file content) ===");

    let file_content = b"# Blizzard_AuctionHouseUI\n## Interface: 100200\n";
    let ckey = ContentKey::from_data(file_content);
    println!("Content key: {}", ckey.to_hex());
    println!("Raw bytes:   {:02x?}", ckey.as_bytes());

    // Round-trip through hex
    let restored =
        ContentKey::from_hex(&ckey.to_hex()).expect("round-trip hex parsing should succeed");
    assert_eq!(ckey, restored);
    println!("Hex round-trip: verified");

    // Construct from known bytes
    let known =
        ContentKey::from_bytes([0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    println!("From bytes:  {}", known.to_hex());
    println!();
}

fn encoding_key_demo() {
    println!("=== EncodingKey (MD5 of encoded blob) ===");

    let encoded_blob = b"BLTE\x00\x00\x00\x00<sample encoded content>";
    let ekey = EncodingKey::from_data(encoded_blob);
    println!("Encoding key: {}", ekey.to_hex());

    // first_9() is used for IDX lookups -- archives store truncated 9-byte EKeys
    let first_9 = ekey.first_9();
    println!("First 9 bytes (IDX lookup): {}", hex::encode(first_9));
    println!("Full 16 bytes:              {}", ekey.to_hex());
    println!();
}

fn jenkins96_demo() {
    println!("=== Jenkins96 (root file name hashing) ===");

    // CASC root files map normalized file paths to content keys using Jenkins96.
    // Paths are uppercased and use backslash separators before hashing.
    let paths = [
        "INTERFACE\\ADDONS\\BLIZZARD_AUCTIONHOUSEUI\\BLIZZARD_AUCTIONHOUSEUI.TOC",
        "WORLD\\MAPS\\AZEROTH\\AZEROTH.WDT",
        "DBFilesClient\\Map.db2",
    ];

    for path in &paths {
        // CASC normalizes to uppercase before hashing
        let normalized = path.to_uppercase();
        let hash = Jenkins96::hash(normalized.as_bytes());
        println!(
            "  {:<64} -> hash64=0x{:016x}  hash32=0x{:08x}",
            path, hash.hash64, hash.hash32
        );
    }
    println!();
}

fn hashlittle_demo() {
    println!("=== hashlittle (single 32-bit hash) ===");

    // Known test vectors from lookup3.c reference implementation
    let empty_hash = hashlittle(b"", 0);
    println!("hashlittle(\"\", 0)                            = 0x{empty_hash:08x}");
    assert_eq!(
        empty_hash, 0xdead_beef,
        "empty string with initval=0 should return 0xdeadbeef"
    );

    let fssa_0 = hashlittle(b"Four score and seven years ago", 0);
    println!("hashlittle(\"Four score and seven years ago\", 0) = 0x{fssa_0:08x}");
    assert_eq!(fssa_0, 0x1777_0551);

    let fssa_1 = hashlittle(b"Four score and seven years ago", 1);
    println!("hashlittle(\"Four score and seven years ago\", 1) = 0x{fssa_1:08x}");
    assert_eq!(fssa_1, 0xcd62_8161);

    // Different initval produces different hash
    assert_ne!(fssa_0, fssa_1);
    println!("Different initval produces different hash: verified");
    println!();
}

fn hashlittle2_demo() {
    println!("=== hashlittle2 (dual 32-bit hash) ===");

    let mut pc = 0u32;
    let mut pb = 0u32;
    hashlittle2(b"Four score and seven years ago", &mut pc, &mut pb);
    println!("hashlittle2(\"Four score and seven years ago\", pc=0, pb=0):");
    println!("  pc = 0x{pc:08x}");
    println!("  pb = 0x{pb:08x}");
    assert_eq!(pc, 0x1777_0551);
    assert_eq!(pb, 0xce72_26e6);

    // pc from hashlittle2 matches hashlittle output
    let single = hashlittle(b"Four score and seven years ago", 0);
    assert_eq!(pc, single, "pc from hashlittle2 should match hashlittle");
    println!("  pc matches hashlittle(): verified");

    // With non-zero initial values
    let mut pc2 = 1u32;
    let mut pb2 = 0u32;
    hashlittle2(b"Four score and seven years ago", &mut pc2, &mut pb2);
    println!("hashlittle2(\"Four score and seven years ago\", pc=1, pb=0):");
    println!("  pc = 0x{pc2:08x}");
    println!("  pb = 0x{pb2:08x}");
    assert_eq!(pc2, 0xcd62_8161);
    assert_eq!(pb2, 0x6cbe_a4b3);
    println!();
}

fn idx_bucket_demo() {
    println!("=== IDX Bucket Calculation ===");

    // The CASC local index splits files across 16 .idx buckets.
    // Bucket = (XOR-fold all 9 bytes of truncated EKey) mod 16.
    let ekey = EncodingKey::from_data(b"sample encoded content for bucket demo");
    let first_9 = ekey.first_9();

    let xor_fold = first_9.iter().fold(0u8, |acc, &b| acc ^ b);
    let bucket = xor_fold % 16;

    println!("EKey:         {}", ekey.to_hex());
    println!("First 9:      {}", hex::encode(first_9));
    println!("XOR fold:     0x{xor_fold:02x} ({xor_fold})");
    println!("Bucket index: {bucket} (of 0..15)");
    println!();
}

fn file_data_id_demo() {
    println!("=== FileDataId ===");

    let fdid = FileDataId::new(4_279_401);
    println!("FileDataId::new(4279401) = {fdid}");
    println!("  .get() = {}", fdid.get());

    // From u32 conversion
    let from_u32: FileDataId = 1_000_000u32.into();
    println!("FileDataId::from(1000000) = {from_u32}");

    // Into u32 conversion
    let raw: u32 = fdid.into();
    println!("Into<u32>: {raw}");

    // Ordering
    let a = FileDataId::new(100);
    let b = FileDataId::new(200);
    println!("FileDataId(100) < FileDataId(200): {}", a < b);
}

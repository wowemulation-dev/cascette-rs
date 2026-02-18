#![allow(clippy::expect_used, clippy::panic)]

//! Dump IDX index entries from a local WoW installation.
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run --example dump_index \
//!       -p cascette-client-storage --features local-install

mod common;

use cascette_client_storage::index::IndexManager;

#[tokio::main]
async fn main() {
    let data = common::data_path();
    println!("Loading indices from: {}", data.display());

    let mut mgr = IndexManager::new(&data);
    mgr.load_all().await.expect("failed to load indices");

    let stats = mgr.stats();
    println!(
        "Loaded {} index files, {} total entries\n",
        stats.index_count, stats.total_entries
    );

    // Per-bucket statistics and sample entries
    let buckets = mgr.loaded_buckets();
    for &bucket in &buckets {
        let count = mgr.bucket_entry_count(bucket);
        println!("Bucket 0x{bucket:02x}: {count} entries");

        // Show first 5 entries from this bucket
        let entries: Vec<_> = mgr
            .iter_entries()
            .filter(|(b, _)| *b == bucket)
            .take(5)
            .collect();

        for (_, entry) in &entries {
            println!(
                "    EKey={} archive={:>4} offset={:>10} size={:>8}",
                common::hex_str(&entry.key),
                entry.archive_id(),
                entry.archive_offset(),
                entry.size,
            );
        }
        if count > 5 {
            println!("    ... and {} more", count - 5);
        }
        println!();
    }

    // Round-trip verification
    println!("=== Round-Trip Verification ===");
    let mut pass = 0u32;
    let mut fail = 0u32;

    for (_, entry) in mgr.iter_entries() {
        let packed = entry.to_packed(9, 30, 32);
        match cascette_client_storage::IndexEntry::from_packed(&packed, 9, 30, 32) {
            Ok(rt) => {
                if rt.key == entry.key
                    && rt.archive_id() == entry.archive_id()
                    && rt.archive_offset() == entry.archive_offset()
                    && rt.size == entry.size
                {
                    pass += 1;
                } else {
                    fail += 1;
                    if fail <= 5 {
                        println!(
                            "  MISMATCH: EKey={} original=({},{},{}) roundtrip=({},{},{})",
                            common::hex_str(&entry.key),
                            entry.archive_id(),
                            entry.archive_offset(),
                            entry.size,
                            rt.archive_id(),
                            rt.archive_offset(),
                            rt.size,
                        );
                    }
                }
            }
            Err(e) => {
                fail += 1;
                if fail <= 5 {
                    println!(
                        "  PARSE ERROR: EKey={} error={e}",
                        common::hex_str(&entry.key),
                    );
                }
            }
        }
    }

    let total = pass + fail;
    if fail == 0 {
        println!("  PASS: {total}/{total} entries round-tripped");
    } else {
        println!("  FAIL: {fail}/{total} entries did not round-trip");
    }
}

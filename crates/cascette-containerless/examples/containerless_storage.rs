#![allow(clippy::expect_used, clippy::panic)]
//! Basic containerless file storage example.
//!
//! Demonstrates opening a containerless storage instance, writing and reading
//! files by encoding key, querying file entries, listing files, computing
//! storage statistics, and running integrity verification.
//!
//! ```text
//! cargo run -p cascette-containerless --example containerless_storage
//! ```

use cascette_containerless::{ContainerlessConfig, ContainerlessStorage, FileEntry};

/// Create a FileEntry whose ekey is the MD5 hash of `data`.
///
/// This mirrors how CASC works: the encoding key is the MD5 of the
/// BLTE-encoded payload, so writing `data` under this entry will
/// pass verification.
fn sample_entry(index: u32, ckey_byte: u8, data: &[u8]) -> FileEntry {
    let hash = md5::compute(data);
    FileEntry {
        index,
        ekey: hash.0,
        ckey: [ckey_byte; 16],
        encoded_size: data.len() as u64,
        decoded_size: data.len() as u64,
        path: Some(format!("data/file_{index}.bin")),
        flags: 0,
    }
}

#[tokio::main]
async fn main() {
    // -- 1. Create a temporary directory for storage --
    let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
    println!("=== Setup ===");
    println!("Storage root: {}", temp_dir.path().display());

    // -- 2. Build configuration --
    let config = ContainerlessConfig::new(temp_dir.path().to_path_buf());

    // -- 3. Open storage --
    let storage = ContainerlessStorage::open(config)
        .await
        .expect("failed to open storage");
    println!("Storage opened (plaintext database)");
    println!();

    // -- 4. Create file entries and write data --
    println!("=== Write Files ===");

    let data_a = b"Hello from containerless storage!";
    let entry_a = sample_entry(1, 0xAA, data_a);
    storage
        .write(&entry_a, data_a)
        .await
        .expect("failed to write file A");
    println!(
        "Wrote file A: ekey={}, size={}",
        hex::encode(entry_a.ekey),
        data_a.len()
    );

    let data_b = b"Second file with different content for testing.";
    let entry_b = sample_entry(2, 0xBB, data_b);
    storage
        .write(&entry_b, data_b)
        .await
        .expect("failed to write file B");
    println!(
        "Wrote file B: ekey={}, size={}",
        hex::encode(entry_b.ekey),
        data_b.len()
    );

    let data_c = b"A third loose file stored on disk.";
    let entry_c = sample_entry(3, 0xCC, data_c);
    storage
        .write(&entry_c, data_c)
        .await
        .expect("failed to write file C");
    println!(
        "Wrote file C: ekey={}, size={}",
        hex::encode(entry_c.ekey),
        data_c.len()
    );
    println!();

    // -- 5. Read back by encoding key --
    println!("=== Read by Encoding Key ===");
    let loaded = storage
        .read_by_ekey(&entry_a.ekey)
        .await
        .expect("failed to read file A");
    println!(
        "Read {} bytes for ekey {}",
        loaded.len(),
        hex::encode(entry_a.ekey)
    );
    assert_eq!(loaded, data_a, "round-trip mismatch for file A");
    println!("Content matches original data.");
    println!();

    // -- 6. Query a file entry from the database --
    println!("=== Query File Entry ===");
    let queried = storage
        .query_file(&entry_b.ekey)
        .await
        .expect("failed to query file B")
        .expect("file B not found in database");
    println!("File B entry:");
    println!("  index:        {}", queried.index);
    println!("  ekey:         {}", hex::encode(queried.ekey));
    println!("  ckey:         {}", hex::encode(queried.ckey));
    println!("  encoded_size: {}", queried.encoded_size);
    println!("  decoded_size: {}", queried.decoded_size);
    println!(
        "  path:         {}",
        queried.path.as_deref().unwrap_or("(none)")
    );
    println!("  flags:        {}", queried.flags);
    println!();

    // -- 7. Check residency --
    println!("=== Residency Check ===");
    println!("File A resident: {}", storage.is_resident(&entry_a.ekey));
    println!("File B resident: {}", storage.is_resident(&entry_b.ekey));
    let missing_key = [0xFF; 16];
    println!(
        "Unknown key resident: {}",
        storage.is_resident(&missing_key)
    );
    println!();

    // -- 8. List all files --
    println!("=== List Files ===");
    let files = storage.list_files().await.expect("failed to list files");
    println!("Total entries: {}", files.len());
    for f in &files {
        println!(
            "  [{}] ekey={} size={}",
            f.index,
            hex::encode(f.ekey),
            f.encoded_size
        );
    }
    println!();

    // -- 9. Storage statistics --
    println!("=== Storage Stats ===");
    let stats = storage.stats().await.expect("failed to compute stats");
    println!("Total files:         {}", stats.total_files);
    println!("Resident files:      {}", stats.resident_files);
    println!("Total size (bytes):  {}", stats.total_size_bytes);
    println!("Resident size:       {}", stats.resident_size_bytes);
    println!();

    // -- 10. Integrity verification --
    println!("=== Verify Integrity ===");
    let report = storage.verify().await.expect("verification failed");
    println!("Checked: {}", report.total);
    println!("Valid:   {}", report.valid);
    println!("Invalid: {}", report.invalid);
    println!("Missing: {}", report.missing);
    if report.invalid_keys.is_empty() {
        println!("All files passed verification.");
    } else {
        println!("Failed keys: {:?}", report.invalid_keys);
    }
}

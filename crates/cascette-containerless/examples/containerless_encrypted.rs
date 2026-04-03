#![allow(clippy::expect_used, clippy::panic)]
//! Encrypted containerless database example.
//!
//! Demonstrates creating a containerless storage instance with Salsa20
//! database encryption, writing files, flushing the encrypted database
//! to disk, and reopening it from the encrypted blob.
//!
//! ```text
//! cargo run -p cascette-containerless --example containerless_encrypted
//! ```

use cascette_containerless::{ContainerlessConfig, ContainerlessStorage, FileDatabase, FileEntry};

/// Encryption key (16 bytes) used for this example.
/// In production this comes from the build config's `build-file-db` field.
const EXAMPLE_KEY: [u8; 16] = [
    0x4A, 0x7B, 0x1C, 0x3D, 0x5E, 0x6F, 0x80, 0x91, 0xA2, 0xB3, 0xC4, 0xD5, 0xE6, 0xF7, 0x08, 0x19,
];

/// Create a FileEntry whose ekey is the MD5 hash of `data`.
fn sample_entry(index: u32, ckey_byte: u8, data: &[u8]) -> FileEntry {
    let hash = md5::compute(data);
    FileEntry {
        index,
        ekey: hash.0,
        ckey: [ckey_byte; 16],
        encoded_size: data.len() as u64,
        decoded_size: data.len() as u64,
        path: Some(format!("data/encrypted_{index}.bin")),
        flags: 0,
    }
}

#[tokio::main]
async fn main() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
    let db_path = temp_dir.path().join(".product.db");

    println!("=== Setup ===");
    println!("Storage root: {}", temp_dir.path().display());
    println!("Database path: {}", db_path.display());
    println!("Encryption key: {}", hex::encode(EXAMPLE_KEY));
    println!();

    // -- 1. Create encrypted storage --
    println!("=== Open Encrypted Storage ===");
    let config = ContainerlessConfig::new(temp_dir.path().to_path_buf()).with_db_key(EXAMPLE_KEY);
    let storage = ContainerlessStorage::open(config)
        .await
        .expect("failed to open encrypted storage");
    println!("Storage opened with Salsa20-encrypted database.");
    println!();

    // -- 2. Write files --
    println!("=== Write Files ===");
    let files_data: &[(&[u8], u8)] = &[
        (b"Encrypted payload alpha", 0x10),
        (b"Encrypted payload beta -- slightly longer content", 0x20),
        (
            b"Encrypted payload gamma with yet more bytes to store",
            0x30,
        ),
    ];

    let mut entries = Vec::new();
    for (i, (data, ckey_byte)) in files_data.iter().enumerate() {
        let entry = sample_entry((i + 1) as u32, *ckey_byte, data);
        storage
            .write(&entry, data)
            .await
            .expect("failed to write file");
        println!(
            "Wrote file {}: ekey={}, size={}",
            i + 1,
            hex::encode(entry.ekey),
            data.len()
        );
        entries.push(entry);
    }
    println!();

    // -- 3. Flush encrypted database to disk --
    println!("=== Flush Encrypted Database ===");
    storage.flush().await.expect("failed to flush database");
    println!("Database flushed to {}", db_path.display());

    let db_bytes = tokio::fs::read(&db_path)
        .await
        .expect("failed to read database file");
    println!("Encrypted database size: {} bytes", db_bytes.len());

    // Show that the file does not start with the SQLite header.
    let is_plaintext_sqlite = db_bytes.len() >= 16 && &db_bytes[..16] == b"SQLite format 3\0";
    println!("Starts with SQLite header: {is_plaintext_sqlite}");
    println!(
        "First 16 bytes (hex): {}",
        hex::encode(&db_bytes[..db_bytes.len().min(16)])
    );
    println!();

    // -- 4. Verify files before closing --
    println!("=== Verify Before Close ===");
    let report = storage.verify().await.expect("verification failed");
    println!(
        "Checked: {}, Valid: {}, Invalid: {}, Missing: {}",
        report.total, report.valid, report.invalid, report.missing
    );
    println!();

    // Drop the first storage instance.
    drop(storage);

    // -- 5. Reopen from encrypted database --
    println!("=== Reopen From Encrypted Database ===");
    let config2 = ContainerlessConfig::new(temp_dir.path().to_path_buf()).with_db_key(EXAMPLE_KEY);
    let storage2 = ContainerlessStorage::open(config2)
        .await
        .expect("failed to reopen encrypted storage");

    let stats = storage2.stats().await.expect("failed to get stats");
    println!("Reopened storage:");
    println!("  Total files:    {}", stats.total_files);
    println!("  Resident files: {}", stats.resident_files);
    println!("  Total size:     {} bytes", stats.total_size_bytes);

    // Verify data survived the round-trip.
    for (i, entry) in entries.iter().enumerate() {
        let loaded = storage2
            .read_by_ekey(&entry.ekey)
            .await
            .expect("failed to read file after reopen");
        assert_eq!(loaded, files_data[i].0, "data mismatch for file {}", i + 1);
    }
    println!("All files read back correctly after reopen.");
    println!();

    // -- 6. Direct FileDatabase operations --
    println!("=== Direct FileDatabase Operations ===");
    let iv = cascette_containerless::db::crypto::iv_from_key(&EXAMPLE_KEY);
    let db = FileDatabase::open_encrypted(&db_bytes, &EXAMPLE_KEY, &iv)
        .await
        .expect("failed to open database directly");

    let count = db.file_count().await.expect("failed to count files");
    println!("File count from direct DB access: {count}");

    let all = db.all_files().await.expect("failed to list files");
    for f in &all {
        println!(
            "  [{}] ekey={} encoded={} decoded={}",
            f.index,
            hex::encode(f.ekey),
            f.encoded_size,
            f.decoded_size
        );
    }
    println!();

    // -- 7. Export and re-import encrypted --
    println!("=== Export and Re-import ===");
    let exported = db
        .export_encrypted(&EXAMPLE_KEY, &iv)
        .await
        .expect("failed to export encrypted");
    println!("Exported encrypted blob: {} bytes", exported.len());

    let db2 = FileDatabase::open_encrypted(&exported, &EXAMPLE_KEY, &iv)
        .await
        .expect("failed to re-import encrypted database");
    let count2 = db2.file_count().await.expect("failed to count files");
    println!("File count after re-import: {count2}");
    assert_eq!(count, count2, "file count mismatch after re-import");
    println!("Round-trip export/import succeeded.");
}

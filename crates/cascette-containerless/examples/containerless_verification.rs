#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Containerless storage verification: initialization and round-trip.
//!
//! Exercises `cascette-containerless` with an in-memory turso database to
//! verify schema creation, file storage, and retrieval.
//!
//! Usage:
//!   cargo run -p cascette-containerless --example containerless_verification

use cascette_containerless::{ContainerlessConfig, ContainerlessStorage, FileEntry};
use cascette_crypto::{ContentKey, EncodingKey};

#[tokio::main]
async fn main() {
    // E2: Initialize containerless storage
    println!("=== E2: Containerless storage initialization ===");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ContainerlessConfig::new(dir.path().to_path_buf());

    let storage = ContainerlessStorage::open(config)
        .await
        .expect("containerless storage should initialize");

    let stats = storage.stats().await.expect("stats");
    assert_eq!(stats.total_files, 0, "new storage should have zero files");
    assert_eq!(
        stats.resident_files, 0,
        "new storage should have zero resident files"
    );

    println!("  schema created, 0 files");

    // E3: Store and retrieve a blob
    println!("\n=== E3: Containerless store and retrieve ===");
    let test_data = b"Hello from cascette verification examples!";
    let ekey = EncodingKey::from_data(test_data);
    let ckey = ContentKey::from_data(test_data);

    let entry = FileEntry {
        index: 1,
        ekey: *ekey.as_bytes(),
        ckey: *ckey.as_bytes(),
        encoded_size: test_data.len() as u64,
        decoded_size: test_data.len() as u64,
        path: None,
        flags: 0,
    };

    storage
        .write(&entry, test_data)
        .await
        .expect("write should succeed");

    assert!(
        storage.is_resident(ekey.as_bytes()),
        "written file should be resident"
    );

    let retrieved = storage
        .read_by_ekey(ekey.as_bytes())
        .await
        .expect("read should succeed");

    assert_eq!(
        retrieved, test_data,
        "retrieved data should match written data"
    );

    let stats = storage.stats().await.expect("stats");
    assert_eq!(stats.total_files, 1, "should have one file");
    assert_eq!(stats.resident_files, 1, "should have one resident file");

    println!(
        "  wrote {} bytes, read {} bytes",
        test_data.len(),
        retrieved.len(),
    );

    println!("\nAll containerless checks passed.");
}

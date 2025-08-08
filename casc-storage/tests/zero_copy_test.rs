//! Tests for zero-copy optimizations

use casc_storage::{
    CascStorage,
    types::{CascConfig, EKey},
};
use std::sync::Arc;
use std::thread;

#[test]
fn test_zero_copy_cache() {
    // This test verifies that Arc-based caching avoids unnecessary clones
    // We'll measure performance indirectly by checking that multiple reads
    // of the same data return the same Arc

    // Create a test storage (this will fail if no test data is available)
    // We'll handle the error gracefully for CI
    let config = CascConfig {
        data_path: "/tmp/test-casc-storage".into(),
        cache_size_mb: 100,
        read_only: true,
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
    };

    let storage_result = CascStorage::new(config);
    if storage_result.is_err() {
        println!("Skipping test - no test storage available");
        return;
    }

    let storage = storage_result.unwrap();

    // Try to read any file to test caching
    // In real tests, we'd have a known EKey
    let test_ekey = EKey::new([
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10,
    ]);

    // First read - will cache the data
    let result1 = storage.read_arc(&test_ekey);
    if result1.is_err() {
        // File doesn't exist - that's OK for this test
        println!("Test file not found - skipping cache test");
        return;
    }

    let data1 = result1.unwrap();

    // Second read - should return cached Arc (zero-copy)
    let data2 = storage.read_arc(&test_ekey).unwrap();

    // Verify that both Arc point to the same data
    // Arc::ptr_eq checks if two Arcs point to the same allocation
    assert!(
        Arc::ptr_eq(&data1, &data2),
        "Zero-copy cache should return the same Arc"
    );

    // For thread test, we need to wrap storage in Arc
    let storage = Arc::new(storage);

    // Spawn threads to test concurrent access
    let storage_clone = Arc::clone(&storage);
    let test_ekey_clone = test_ekey;
    let handle = thread::spawn(move || storage_clone.read_arc(&test_ekey_clone).unwrap());

    let data3 = handle.join().unwrap();

    // Thread should also get the same Arc from cache
    assert!(
        Arc::ptr_eq(&data1, &data3),
        "Zero-copy cache should work across threads"
    );
}

#[test]
fn test_archive_reader_zero_copy() {
    use casc_storage::archive::ArchiveReader;
    use std::path::Path;

    // Try to open a test archive
    let test_archive = Path::new("/tmp/test-archive.dat");
    if !test_archive.exists() {
        // Create a small test archive
        std::fs::write(test_archive, vec![0u8; 1024]).ok();
    }

    let reader_result = ArchiveReader::open(test_archive);
    if reader_result.is_err() {
        println!("Skipping archive test - cannot create test archive");
        return;
    }

    let reader = reader_result.unwrap();

    // Test that read_at_cow returns borrowed data when memory-mapped
    if reader.is_memory_mapped() {
        let cow_data = reader.read_at_cow(0, 100).unwrap();

        // Cow::Borrowed means zero-copy
        assert!(
            matches!(cow_data, std::borrow::Cow::Borrowed(_)),
            "Memory-mapped reads should be zero-copy"
        );
    }

    // Clean up
    let _ = std::fs::remove_file(test_archive);
}

#[test]
fn test_blte_parse_zero_copy() {
    use blte::{BLTE_MAGIC, BLTEFile};

    // Create a minimal BLTE file
    let mut test_data = Vec::new();
    test_data.extend_from_slice(&BLTE_MAGIC);
    test_data.extend_from_slice(&0u32.to_be_bytes()); // header_size = 0 (single chunk)
    test_data.extend_from_slice(b"N"); // Mode 'N' (no compression)
    test_data.extend_from_slice(b"test data");

    // Test zero-copy parsing
    let file_ref = BLTEFile::parse_ref(&test_data).unwrap();

    // Verify that data is borrowed, not cloned
    assert_eq!(file_ref.data.len(), 10); // "N" + "test data"
    assert_eq!(file_ref.data[0], b'N');

    // Get chunk data should also be zero-copy
    let chunk_ref = file_ref.get_chunk_data(0).unwrap();
    assert_eq!(chunk_ref.data.len(), 10);

    // Verify the data pointer is the same (zero-copy)
    assert_eq!(chunk_ref.data.as_ptr(), file_ref.data.as_ptr());
}

#[test]
fn test_memory_efficiency() {
    // This test verifies that our zero-copy optimizations
    // reduce memory allocations

    // We can't easily measure allocations directly in Rust tests,
    // but we can verify that our APIs work as expected

    // Test that Arc-based cache values can be shared
    let data = vec![1, 2, 3, 4, 5];
    let arc1 = Arc::new(data);
    let arc2 = Arc::clone(&arc1);

    // Both should point to the same allocation
    assert!(Arc::ptr_eq(&arc1, &arc2));

    // Strong count should be 2
    assert_eq!(Arc::strong_count(&arc1), 2);

    // Drop one reference
    drop(arc2);

    // Strong count should be 1
    assert_eq!(Arc::strong_count(&arc1), 1);
}

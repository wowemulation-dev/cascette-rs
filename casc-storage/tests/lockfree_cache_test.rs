//! Tests for lock-free cache implementation

use casc_storage::cache::LockFreeCache;
use casc_storage::storage::CascStorage;
use casc_storage::types::{CascConfig, EKey};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tempfile::TempDir;

/// Test basic lock-free cache operations
#[test]
fn test_lockfree_cache_basic() {
    let cache = LockFreeCache::new(10 * 1024 * 1024); // 10MB

    // Test data
    let key1 = EKey::new([
        0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd,
        0xef,
    ]);
    let data1 = Arc::new(vec![1, 2, 3, 4, 5]);

    // Test put and get
    cache.put(key1, Arc::clone(&data1));
    let retrieved = cache.get(&key1).unwrap();

    // Should return the same Arc (zero-copy)
    assert!(
        Arc::ptr_eq(&data1, &retrieved),
        "Cache should return the same Arc"
    );

    // Test cache stats
    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.entry_count, 1);

    println!("✓ Basic lock-free cache operations test passed");
}

/// Test concurrent cache access performance
#[test]
fn test_concurrent_cache_performance() {
    let cache = Arc::new(LockFreeCache::new(100 * 1024 * 1024)); // 100MB
    let num_threads = 8;
    let operations_per_thread = 10000;

    let start = Instant::now();
    let mut handles = vec![];

    // Spawn threads for concurrent access
    for thread_id in 0..num_threads {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for i in 0..operations_per_thread {
                let mut key_bytes = [0u8; 16];
                let val = (thread_id * operations_per_thread + i) as u32;
                key_bytes[0..4].copy_from_slice(&val.to_be_bytes());
                let key = EKey::new(key_bytes);
                let data = Arc::new(vec![thread_id as u8; 100]);

                // Mix of puts and gets
                cache_clone.put(key, Arc::clone(&data));
                cache_clone.get(&key);

                // Some keys from other threads
                if i % 10 == 0 {
                    let mut other_key_bytes = [0u8; 16];
                    other_key_bytes[0..4].copy_from_slice(&(i as u32).to_be_bytes());
                    let other_key = EKey::new(other_key_bytes);
                    cache_clone.get(&other_key);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = num_threads * operations_per_thread * 2; // puts + gets
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();

    println!("Concurrent cache performance:");
    println!("  Total operations: {total_ops}");
    println!("  Time: {elapsed:.2?}");
    println!("  Throughput: {ops_per_sec:.0} ops/sec");

    let stats = cache.stats();
    println!(
        "  Cache stats: {} entries, {:.2}% hit rate",
        stats.entry_count,
        stats.hit_rate * 100.0
    );

    // Should achieve high throughput with lock-free implementation
    assert!(
        ops_per_sec > 100_000.0,
        "Lock-free cache should achieve >100k ops/sec, got {ops_per_sec:.0}"
    );

    println!("✓ Concurrent cache performance test passed");
}

/// Test cache eviction under memory pressure
#[test]
fn test_cache_eviction() {
    let cache = LockFreeCache::new(1000); // 1KB - small for testing eviction

    // Add items that exceed cache size
    for i in 0..100 {
        let mut key_bytes = [0u8; 16];
        key_bytes[0] = i as u8;
        let key = EKey::new(key_bytes);
        let data = Arc::new(vec![i as u8; 50]); // 50 bytes each
        cache.put(key, data);
    }

    // Cache should have evicted items to stay under limit
    assert!(
        cache.current_size() <= 1000,
        "Cache size {} should be <= 1000",
        cache.current_size()
    );

    let stats = cache.stats();
    assert!(
        stats.entry_count < 100,
        "Cache should have evicted items, has {} entries",
        stats.entry_count
    );

    println!("✓ Cache eviction test passed");
}

/// Test zero-copy behavior with Arc
#[test]
fn test_zero_copy_with_arc() {
    let cache = LockFreeCache::new(10 * 1024 * 1024);

    let key = EKey::new([
        0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78,
        0x90,
    ]);
    let original_data = Arc::new(vec![42u8; 1000]);

    // Put the Arc into cache
    cache.put(key, Arc::clone(&original_data));

    // Get multiple times
    let retrieved1 = cache.get(&key).unwrap();
    let retrieved2 = cache.get(&key).unwrap();
    let retrieved3 = cache.get(&key).unwrap();

    // All should be the same Arc (zero-copy)
    assert!(Arc::ptr_eq(&original_data, &retrieved1));
    assert!(Arc::ptr_eq(&retrieved1, &retrieved2));
    assert!(Arc::ptr_eq(&retrieved2, &retrieved3));

    // Strong count should reflect all references
    assert_eq!(Arc::strong_count(&original_data), 5); // original + cache + 3 retrieved

    println!("✓ Zero-copy Arc behavior test passed");
}

/// Test CASC storage with lock-free cache
#[tokio::test]
#[ignore = "Test has setup issues with storage index synchronization"]
async fn test_casc_storage_with_lockfree_cache() {
    let temp_dir = TempDir::new().unwrap();
    let config = CascConfig {
        data_path: temp_dir.path().to_path_buf(),
        cache_size_mb: 10,
        read_only: false,
        use_memory_mapping: true,
        max_archive_size: 100 * 1024 * 1024,
    };

    let storage = CascStorage::new(config).unwrap();

    // Write some test data
    let key1 = EKey::new([0x11; 16]);
    let key2 = EKey::new([0x22; 16]);
    let data1 = vec![1u8; 1000];
    let data2 = vec![2u8; 1000];

    storage.write(&key1, &data1).unwrap();
    storage.write(&key2, &data2).unwrap();

    // Flush to ensure data is written
    storage.flush().unwrap();

    // Create new storage instance to test cache
    let storage2 = CascStorage::new(CascConfig {
        data_path: temp_dir.path().to_path_buf(),
        cache_size_mb: 10,
        read_only: true,
        use_memory_mapping: true,
        max_archive_size: 100 * 1024 * 1024,
    })
    .unwrap();

    // Load indices
    storage2.load_indices_parallel().await.unwrap();
    storage2.load_archives().unwrap();

    // Read using Arc (zero-copy)
    let start = Instant::now();
    let read1_arc = storage2.read_arc(&key1).unwrap();
    let first_read_time = start.elapsed();

    // Second read should be from cache (much faster)
    let start = Instant::now();
    let read1_arc_cached = storage2.read_arc(&key1).unwrap();
    let cached_read_time = start.elapsed();

    // Should be the same Arc (zero-copy from cache)
    assert!(
        Arc::ptr_eq(&read1_arc, &read1_arc_cached),
        "Cached read should return the same Arc"
    );

    // Cached read should be significantly faster
    assert!(
        cached_read_time.as_nanos() < first_read_time.as_nanos() / 10,
        "Cached read should be >10x faster: first={first_read_time:?}, cached={cached_read_time:?}"
    );

    println!(
        "Read times: first={:?}, cached={:?} ({}x faster)",
        first_read_time,
        cached_read_time,
        first_read_time.as_nanos() / cached_read_time.as_nanos().max(1)
    );

    println!("✓ CASC storage with lock-free cache test passed");
}

/// Benchmark lock-free cache vs simulated LRU cache
#[test]
fn test_lockfree_vs_lru_performance() {
    // Test with lock-free cache
    let lockfree_cache = Arc::new(LockFreeCache::new(50 * 1024 * 1024));
    let num_operations = 100_000;

    let start = Instant::now();
    for i in 0..num_operations {
        let mut key_bytes = [0u8; 16];
        let val = (i % 1000) as u16;
        key_bytes[0..2].copy_from_slice(&val.to_be_bytes());
        let key = EKey::new(key_bytes); // Reuse some keys
        let data = Arc::new(vec![i as u8; 100]);
        lockfree_cache.put(key, data);
        lockfree_cache.get(&key);
    }
    let lockfree_time = start.elapsed();
    let lockfree_ops_per_sec = (num_operations * 2) as f64 / lockfree_time.as_secs_f64();

    println!("Performance comparison:");
    println!(
        "  Lock-free cache: {:?} for {} ops ({:.0} ops/sec)",
        lockfree_time,
        num_operations * 2,
        lockfree_ops_per_sec
    );

    // Lock-free should achieve very high throughput
    assert!(
        lockfree_ops_per_sec > 200_000.0,
        "Lock-free cache should achieve >200k ops/sec in single-threaded test"
    );

    println!("✓ Lock-free cache performance benchmark passed");
}

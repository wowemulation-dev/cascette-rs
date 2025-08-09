//! Test binary search index optimizations

use casc_storage::index::{CombinedIndex, SortedIndex};
use casc_storage::types::{ArchiveLocation, EKey};
use std::collections::HashMap;
use std::time::Instant;

/// Generate test data with realistic distribution
fn generate_test_data(count: usize) -> Vec<(EKey, ArchiveLocation)> {
    let mut data = Vec::with_capacity(count);

    for i in 0..count {
        let mut key_bytes = [0u8; 16];
        // Simulate real key distribution
        key_bytes[0] = (i % 256) as u8;
        key_bytes[1] = ((i / 256) % 256) as u8;
        key_bytes[2] = ((i / 65536) % 256) as u8;

        let key = EKey::new(key_bytes);
        let location = ArchiveLocation {
            archive_id: (i % 1000) as u16,
            offset: (i * 4096) as u64,
            size: 1024 + (i % 4096) as u32,
        };

        data.push((key, location));
    }

    data
}

#[test]
fn test_sorted_index_performance() {
    let test_data = generate_test_data(10000);

    // Build sorted index
    let mut sorted_index = SortedIndex::new();
    for (key, location) in &test_data {
        sorted_index.insert(*key, *location);
    }

    // Benchmark lookups
    let start = Instant::now();
    let mut found = 0;
    for (key, _) in &test_data[..1000] {
        if sorted_index.lookup(key).is_some() {
            found += 1;
        }
    }
    let elapsed = start.elapsed();

    assert_eq!(found, 1000);
    println!("Sorted index: 1000 lookups in {elapsed:?}");

    // Compare with HashMap for reference
    let mut hashmap = HashMap::new();
    for (key, location) in &test_data {
        hashmap.insert(*key, *location);
    }

    let start = Instant::now();
    let mut _found = 0;
    for (key, _) in &test_data[..1000] {
        if hashmap.contains_key(key) {
            _found += 1;
        }
    }
    let hashmap_elapsed = start.elapsed();

    println!("HashMap: 1000 lookups in {hashmap_elapsed:?}");

    // Binary search should be competitive with HashMap for this size
    assert!(elapsed.as_micros() < hashmap_elapsed.as_micros() * 10);
}

#[test]
fn test_combined_index_bucket_optimization() {
    let index = CombinedIndex::new();

    // Insert data across multiple buckets
    let test_data = generate_test_data(5000);
    for (key, location) in &test_data {
        index.insert(*key, *location);
    }

    // First lookup should build global index
    let start = Instant::now();
    let result = index.lookup(&test_data[0].0);
    let first_lookup = start.elapsed();
    assert!(result.is_some());

    // Second lookup of same key should be faster (cached bucket)
    let start = Instant::now();
    let result = index.lookup(&test_data[0].0);
    let second_lookup = start.elapsed();
    assert!(result.is_some());

    println!("First lookup: {first_lookup:?}");
    println!("Second lookup: {second_lookup:?}");

    // Second lookup should be faster due to global index
    assert!(second_lookup < first_lookup);

    // Test batch lookups
    let keys: Vec<_> = test_data.iter().take(100).map(|(k, _)| *k).collect();
    let start = Instant::now();
    let results = index.lookup_batch(&keys);
    let batch_elapsed = start.elapsed();

    assert_eq!(results.len(), 100);
    assert!(results.iter().all(|r| r.is_some()));

    println!("Batch lookup (100 keys): {batch_elapsed:?}");

    // Check statistics
    let stats = index.stats();
    println!("Index statistics:");
    println!("  Total entries: {}", stats.total_entries);
    println!("  Bucket count: {}", stats.bucket_count);
    println!("  Avg bucket size: {:.2}", stats.avg_bucket_size);
    println!("  Global index size: {}", stats.global_index_size);

    assert_eq!(stats.total_entries, 5000);
}

#[test]
fn test_range_queries() {
    let mut index = SortedIndex::new();

    // Insert sequential keys
    for i in 0..100u8 {
        let mut key_bytes = [0u8; 16];
        key_bytes[0] = i;
        let key = EKey::new(key_bytes);
        let location = ArchiveLocation {
            archive_id: i as u16,
            offset: (i as u64) * 100,
            size: 50,
        };
        index.insert(key, location);
    }

    // Test range query
    let mut start_key = [0u8; 16];
    start_key[0] = 10;
    let start = EKey::new(start_key);

    let mut end_key = [0u8; 16];
    end_key[0] = 20;
    let end = EKey::new(end_key);

    let range: Vec<_> = index.range(&start, &end).collect();
    assert_eq!(range.len(), 11); // 10 through 20 inclusive

    // Test lower bound
    let mut target_key = [0u8; 16];
    target_key[0] = 15;
    let target = EKey::new(target_key);

    let lower = index.lower_bound(&target);
    assert!(lower.is_some());
    assert_eq!(lower.unwrap().0.as_bytes()[0], 15);

    // Test upper bound
    let upper = index.upper_bound(&target);
    assert!(upper.is_some());
    assert_eq!(upper.unwrap().0.as_bytes()[0], 15);
}

#[test]
fn test_migration_from_hashmap() {
    // Create HashMap with test data
    let mut hashmap = HashMap::new();
    let test_data = generate_test_data(1000);
    for (key, location) in &test_data {
        hashmap.insert(*key, *location);
    }

    // Migrate to sorted index
    let sorted_index = SortedIndex::from_hashmap(hashmap.clone());

    // Verify all entries are present
    assert_eq!(sorted_index.len(), hashmap.len());

    for (key, expected_location) in &test_data {
        let actual_location = sorted_index.lookup(key);
        assert_eq!(actual_location, Some(expected_location));
    }

    // Verify entries are sorted
    let mut prev_key = None;
    for (key, _) in sorted_index.entries() {
        if let Some(prev) = prev_key {
            assert!(key > prev, "Keys should be in sorted order");
        }
        prev_key = Some(key);
    }
}

#[test]
fn test_concurrent_combined_index() {
    use std::sync::Arc;
    use std::thread;

    let index = Arc::new(CombinedIndex::new());
    let test_data = Arc::new(generate_test_data(10000));

    // Insert data concurrently
    let mut handles = vec![];
    for chunk in test_data.chunks(1000) {
        let index_clone = Arc::clone(&index);
        let chunk = chunk.to_vec();

        let handle = thread::spawn(move || {
            for (key, location) in chunk {
                index_clone.insert(key, location);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all data is present
    assert_eq!(index.total_entries(), 10000);

    // Concurrent lookups
    let mut handles = vec![];
    for _ in 0..10 {
        let index_clone = Arc::clone(&index);
        let test_data_clone = Arc::clone(&test_data);

        let handle = thread::spawn(move || {
            let mut found = 0;
            for (key, _) in test_data_clone.iter().take(100) {
                if index_clone.lookup(key).is_some() {
                    found += 1;
                }
            }
            found
        });
        handles.push(handle);
    }

    let mut total_found = 0;
    for handle in handles {
        total_found += handle.join().unwrap();
    }

    assert_eq!(total_found, 1000); // 10 threads × 100 lookups each
}

#[test]
fn test_binary_search_vs_linear() {
    let test_data = generate_test_data(50000);

    // Sorted index (binary search)
    let mut sorted_index = SortedIndex::new();
    for (key, location) in &test_data {
        sorted_index.insert(*key, *location);
    }

    // Simulate linear search with Vec
    let vec_data = test_data.clone();

    // Benchmark sorted index (binary search)
    let keys_to_find: Vec<_> = test_data.iter().step_by(50).map(|(k, _)| *k).collect();

    let start = Instant::now();
    let mut binary_found = 0;
    for key in &keys_to_find {
        if sorted_index.lookup(key).is_some() {
            binary_found += 1;
        }
    }
    let binary_time = start.elapsed();

    // Benchmark linear search
    let start = Instant::now();
    let mut linear_found = 0;
    for key in &keys_to_find {
        if vec_data.iter().any(|(k, _)| k == key) {
            linear_found += 1;
        }
    }
    let linear_time = start.elapsed();

    assert_eq!(
        binary_found, linear_found,
        "Both searches should find same number of keys"
    );

    println!(
        "Binary search: {binary_time:?} for {} lookups",
        keys_to_find.len()
    );
    println!(
        "Linear search: {linear_time:?} for {} lookups",
        keys_to_find.len()
    );

    // Binary search should be significantly faster
    let speedup = linear_time.as_nanos() as f64 / binary_time.as_nanos() as f64;
    println!("Speedup: {speedup:.2}x");

    assert!(
        speedup > 10.0,
        "Binary search should be >10x faster than linear for 50k entries"
    );
}

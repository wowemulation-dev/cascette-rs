//! Iterator functionality tests for keyring-based TACT key storage
//!
//! This module contains comprehensive tests for the `KeyringIterator` implementation.
//! Tests verify proper iteration behavior, edge cases, and performance characteristics.

#[cfg(test)]
#[cfg(feature = "keyring")]
mod iterator_tests {
    use crate::TactKey;
    use crate::keyring::KeyringTactKeyStore;
    use std::collections::{HashMap, HashSet};

    #[test]
    #[ignore = "Requires keyring access"]
    fn test_empty_store_iterator() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");
        let mut iter = store.iter();

        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None); // Multiple calls should be safe
    }

    #[test]
    #[ignore = "Requires keyring access"]
    fn test_single_key_iterator() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");
        let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);

        store.add(test_key).expect("Adding key should succeed");

        let mut iter = store.iter();
        let first_key = iter.next();
        assert!(first_key.is_some(), "Iterator should return the added key");

        let retrieved_key = first_key.expect("First key should exist");
        assert_eq!(retrieved_key.id, 0x1234_5678_90AB_CDEF);
        assert_eq!(retrieved_key.key, [0x42; 16]);

        // Should be no more keys
        assert_eq!(
            iter.next(),
            None,
            "Iterator should be exhausted after first key"
        );
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_multiple_keys_iterator() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        let test_keys = vec![
            TactKey::new(0x1111_1111_1111_1111, [0x11; 16]),
            TactKey::new(0x2222_2222_2222_2222, [0x22; 16]),
            TactKey::new(0x3333_3333_3333_3333, [0x33; 16]),
            TactKey::new(0x4444_4444_4444_4444, [0x44; 16]),
        ];

        // Add all test keys
        for key in &test_keys {
            store.add(*key).expect("Adding key should succeed");
        }

        // Collect all keys from iterator
        let retrieved_keys: Vec<TactKey> = store.iter().collect();

        // Should have same number of keys
        assert_eq!(retrieved_keys.len(), test_keys.len());

        // Convert to sets for comparison (order might not be guaranteed)
        let expected_ids: HashSet<u64> = test_keys.iter().map(|k| k.id).collect();
        let retrieved_ids: HashSet<u64> = retrieved_keys.iter().map(|k| k.id).collect();

        assert_eq!(expected_ids, retrieved_ids);

        // Verify key contents
        let expected_map: HashMap<u64, [u8; 16]> =
            test_keys.iter().map(|k| (k.id, k.key)).collect();
        for key in retrieved_keys {
            assert_eq!(
                expected_map.get(&key.id),
                Some(&key.key),
                "Key content should match for ID {:016X}",
                key.id
            );
        }
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_after_modifications() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Add initial keys
        let key1 = TactKey::new(0x1111_1111_1111_1111, [0x11; 16]);
        let key2 = TactKey::new(0x2222_2222_2222_2222, [0x22; 16]);
        store.add(key1).expect("Adding key1 should succeed");
        store.add(key2).expect("Adding key2 should succeed");

        // Verify initial state
        let initial_keys: HashSet<u64> = store.iter().map(|k| k.id).collect();
        assert_eq!(initial_keys.len(), 2);
        assert!(initial_keys.contains(&0x1111_1111_1111_1111));
        assert!(initial_keys.contains(&0x2222_2222_2222_2222));

        // Add another key
        let key3 = TactKey::new(0x3333_3333_3333_3333, [0x33; 16]);
        store.add(key3).expect("Adding key3 should succeed");

        // Iterator should reflect the change
        let after_add: HashSet<u64> = store.iter().map(|k| k.id).collect();
        assert_eq!(after_add.len(), 3);
        assert!(after_add.contains(&0x3333_3333_3333_3333));

        // Remove a key
        store
            .remove(0x2222_2222_2222_2222)
            .expect("Removing key2 should succeed");

        // Iterator should reflect the removal
        let after_remove: HashSet<u64> = store.iter().map(|k| k.id).collect();
        assert_eq!(after_remove.len(), 2);
        assert!(!after_remove.contains(&0x2222_2222_2222_2222));
        assert!(after_remove.contains(&0x1111_1111_1111_1111));
        assert!(after_remove.contains(&0x3333_3333_3333_3333));
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_consistency_with_len() {
        // Test consistency between len() and iterator count for various sizes
        let test_sizes = [0, 1, 5, 10];

        for &size in &test_sizes {
            // Clear store by creating a new one (or implement a clear method)
            let store = KeyringTactKeyStore::new().expect("Store recreation should succeed");

            // Add 'size' number of keys
            for i in 0..size {
                let key = TactKey::new(0x1000_0000_0000_0000 + i as u64, [i as u8; 16]);
                store.add(key).expect("Adding key should succeed");
            }

            let len = store.len();
            let iter_count = store.iter().count();

            assert_eq!(len, size);
            assert_eq!(iter_count, size);
            assert_eq!(len, iter_count);
        }
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_multiple_times() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        let test_keys = vec![
            TactKey::new(0x1111_1111_1111_1111, [0x11; 16]),
            TactKey::new(0x2222_2222_2222_2222, [0x22; 16]),
        ];

        for key in &test_keys {
            store.add(*key).expect("Adding key should succeed");
        }

        // Create multiple iterators and verify they all work
        let keys1: Vec<TactKey> = store.iter().collect();
        let keys2: Vec<TactKey> = store.iter().collect();
        let keys3: Vec<TactKey> = store.iter().collect();

        assert_eq!(keys1.len(), 2);
        assert_eq!(keys2.len(), 2);
        assert_eq!(keys3.len(), 2);

        // Convert to sets for comparison (order might vary)
        let set1: HashSet<u64> = keys1.iter().map(|k| k.id).collect();
        let set2: HashSet<u64> = keys2.iter().map(|k| k.id).collect();
        let set3: HashSet<u64> = keys3.iter().map(|k| k.id).collect();

        assert_eq!(set1, set2);
        assert_eq!(set2, set3);
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_partial_consumption() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Add several keys
        for i in 0..5 {
            let key = TactKey::new(0x1000_0000_0000_0000 + i, [i as u8; 16]);
            store.add(key).expect("Adding key should succeed");
        }

        let mut iter = store.iter();

        // Consume only part of the iterator
        let first_key = iter.next();
        assert!(first_key.is_some());

        let second_key = iter.next();
        assert!(second_key.is_some());

        // Drop the iterator without consuming all items
        drop(iter);

        // Create a new iterator - should still work
        assert_eq!(store.iter().count(), 5);
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_with_hardcoded_keys() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Load hardcoded keys
        let hardcoded_count = store
            .load_hardcoded_keys()
            .expect("Loading hardcoded keys should succeed");
        assert!(hardcoded_count > 0);

        // Iterator should include all hardcoded keys
        let all_keys: Vec<TactKey> = store.iter().collect();
        assert_eq!(all_keys.len(), hardcoded_count);

        // Verify some known hardcoded keys are present
        let key_ids: HashSet<u64> = all_keys.iter().map(|k| k.id).collect();

        // These are from the original hardcoded keys in the design
        let known_ids = [
            0xFA50_5078_126A_CB3E, // BFA
            0xFF81_3F7D_062A_C0BC, // BFA
            0xB767_2964_1141_CB34, // Shadowlands
            0x0EBE_36B5_010D_FD7F, // The War Within
            0xDEE3_A052_1EFF_6F03, // Classic
        ];

        for &known_id in &known_ids {
            assert!(
                key_ids.contains(&known_id),
                "Missing hardcoded key: {:016X}",
                known_id
            );
        }
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_error_handling() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Test iterator when keyring might be in an error state
        // This is difficult to test without mocking, but we can test basic resilience

        let mut iter = store.iter();

        // Iterator should handle internal errors gracefully
        // If keyring access fails, iterator might return None or handle the error internally
        let result = iter.next();

        // Either we get None (empty store) or Some(key) (if keyring is accessible)
        match result {
            None => {
                // Empty store or keyring error handled gracefully
                assert_eq!(store.len(), 0);
            }
            Some(key) => {
                // Iterator working normally
                assert!(key.id > 0, "Key ID should be valid and non-zero");
                assert_ne!(key.key, [0; 16], "Key data should not be empty");
            }
        }
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_performance_characteristics() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Add a moderate number of keys
        let key_count = 50;
        for i in 0..key_count {
            let key = TactKey::new(0x1000_0000_0000_0000 + i, [i as u8; 16]);
            store.add(key).expect("Adding key should succeed");
        }

        // Measure iterator creation and consumption time
        let start = std::time::Instant::now();
        let count = store.iter().count();
        let duration = start.elapsed();

        assert_eq!(count, key_count as usize);

        // Iterator should complete in reasonable time
        // This is a rough benchmark - actual time depends on system
        assert!(
            duration.as_millis() < 5000,
            "Iterator took too long: {:?}",
            duration
        );

        // Test multiple iterations
        let start = std::time::Instant::now();
        for _ in 0..10 {
            let _count = store.iter().count();
        }
        let multiple_duration = start.elapsed();

        // Multiple iterations should not be exponentially slower
        assert!(
            multiple_duration.as_millis() < 20000,
            "Multiple iterations took too long: {:?}",
            multiple_duration
        );
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_collect_vs_manual_iteration() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Add test keys
        let test_keys = [
            TactKey::new(0x1111_1111_1111_1111, [0x11; 16]),
            TactKey::new(0x2222_2222_2222_2222, [0x22; 16]),
            TactKey::new(0x3333_3333_3333_3333, [0x33; 16]),
        ];

        for key in &test_keys {
            store.add(*key).expect("Adding key should succeed");
        }

        // Collect using collect()
        let collected_keys: Vec<TactKey> = store.iter().collect();

        // Manual iteration
        let mut manual_keys = Vec::new();
        for key in store.iter() {
            manual_keys.push(key);
        }

        // Both methods should produce the same result
        assert_eq!(collected_keys.len(), manual_keys.len());

        let collected_ids: HashSet<u64> = collected_keys.iter().map(|k| k.id).collect();
        let manual_ids: HashSet<u64> = manual_keys.iter().map(|k| k.id).collect();

        assert_eq!(collected_ids, manual_ids);
    }

    #[test]
    #[ignore = "TDD - Implementation not complete"]
    fn test_iterator_functional_operations() {
        let store = KeyringTactKeyStore::new().expect("Store creation should succeed");

        // Add keys with different patterns
        let test_data = [
            (0x1000_0000_0000_0001, [0x01; 16]),
            (0x1000_0000_0000_0002, [0x02; 16]),
            (0x1000_0000_0000_0003, [0x03; 16]),
            (0x1000_0000_0000_0004, [0x04; 16]),
            (0x1000_0000_0000_0005, [0x05; 16]),
        ];

        for &(id, key_data) in &test_data {
            let key = TactKey::new(id, key_data);
            store.add(key).expect("Adding key should succeed");
        }

        // Test functional iterator operations
        assert_eq!(store.iter().count(), 5);

        // Filter keys
        let filtered_count = store.iter().filter(|k| k.id % 2 == 0).count();
        assert_eq!(filtered_count, 2); // Even IDs: 2, 4

        // Count operation
        let key_count = store.iter().count();
        assert_eq!(key_count, 5);

        // Find operation
        let found_key = store.iter().find(|k| k.id == 0x1000_0000_0000_0003);
        assert!(
            found_key.is_some(),
            "Should find key with ID 0x1000000000000003"
        );
        assert_eq!(found_key.expect("Found key should be Some").key, [0x03; 16]);

        // Count operation
        let count = store.iter().count();
        assert_eq!(count, 5);

        // Any/All operations
        let has_target_key = store.iter().any(|k| k.id == 0x1000_0000_0000_0001);
        assert!(has_target_key);

        let all_have_valid_ids = store.iter().all(|k| k.id >= 0x1000_0000_0000_0000);
        assert!(all_have_valid_ids);
    }
}

#[cfg(test)]
#[cfg(not(feature = "keyring"))]
mod stub_iterator_tests {
    use crate::keyring::KeyringTactKeyStore;

    #[test]
    fn test_stub_iterator_empty() {
        // Test that stub iterator behaves correctly when keyring is disabled
        let store = KeyringTactKeyStore::new();
        assert!(store.is_err());

        // If we could create a store, its iterator should be empty
        // This is handled by the stub implementation returning empty iterator
    }
}

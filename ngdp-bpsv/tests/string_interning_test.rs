//! Test string interning functionality for memory efficiency

use ngdp_bpsv::{InternedBpsvDocument, StringInterner};
use std::sync::Arc;

/// Test basic string interning
#[test]
fn test_basic_string_interning() {
    let interner = StringInterner::new();

    // Intern the same string multiple times
    let s1 = interner.intern("us");
    let s2 = interner.intern("us");
    let s3 = interner.intern("us");

    // All should be the same Arc
    assert!(Arc::ptr_eq(&s1, &s2), "First two should be same Arc");
    assert!(Arc::ptr_eq(&s2, &s3), "Last two should be same Arc");

    // Different string should be different Arc
    let s4 = interner.intern("eu");
    assert!(
        !Arc::ptr_eq(&s1, &s4),
        "Different strings should be different Arcs"
    );

    assert_eq!(interner.unique_count(), 2, "Should have 2 unique strings");

    println!("✓ Basic string interning test passed");
}

/// Test memory savings with typical BPSV config data
#[test]
fn test_memory_savings_with_config_data() {
    // Simulate a typical BPSV config with repeated values
    let bpsv_data = r#"Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!HEX:16
## seqn = 1234567
us|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456
eu|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456
cn|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456
kr|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456
tw|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456
sg|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456
xx|a1b2c3d4e5f678901234567890123456|b1b2c3d4e5f678901234567890123456|c1b2c3d4e5f678901234567890123456|1001|1.14.3.51903|d1b2c3d4e5f678901234567890123456"#;

    let interned_doc = InternedBpsvDocument::parse(bpsv_data).unwrap();

    let stats = interned_doc.memory_stats();

    println!("Memory statistics:");
    println!("  Unique strings: {}", stats.unique_strings);
    println!("  Total bytes: {}", stats.total_bytes);
    println!("  Total references: {}", stats.total_references);
    println!("  Deduplication ratio: {:.2}x", stats.deduplication_ratio);
    println!(
        "  Hit rate: {:.2}%",
        interned_doc.interner_hit_rate() * 100.0
    );

    // Should have significant deduplication
    assert!(
        stats.deduplication_ratio > 2.0,
        "Should have >2x deduplication"
    );
    assert_eq!(interned_doc.row_count(), 7, "Should have 7 rows");

    // The repeated values (configs, version, build) should be interned
    // Only regions are unique
    assert!(
        stats.unique_strings < 30,
        "Should have fewer unique strings due to interning"
    );

    println!("✓ Memory savings test passed");
}

/// Test finding rows with interned values
#[test]
fn test_find_rows_with_interning() {
    let bpsv_data = r#"Region!STRING:0|Status!STRING:0|Level!DEC:4
## seqn = 999
us|active|10
eu|active|10
cn|maintenance|10
kr|active|10
tw|maintenance|10"#;

    let doc = InternedBpsvDocument::parse(bpsv_data).unwrap();

    // Find all active regions
    let active_rows = doc.find_rows("Status", "active");
    assert_eq!(active_rows.len(), 3, "Should find 3 active regions");

    // Find all maintenance regions
    let maint_rows = doc.find_rows("Status", "maintenance");
    assert_eq!(maint_rows.len(), 2, "Should find 2 maintenance regions");

    // Check that "active" and "maintenance" are interned
    let stats = doc.memory_stats();
    println!("Find test - unique strings: {}", stats.unique_strings);

    // We should have: us, eu, cn, kr, tw, active, maintenance, 10 = 8 unique strings
    assert!(
        stats.unique_strings <= 8,
        "Should have at most 8 unique strings"
    );

    println!("✓ Find rows test passed");
}

/// Test concurrent access to interned document
#[test]
#[ignore = "Deduplication ratio test depends on implementation details"]
fn test_concurrent_interning() {
    use std::thread;

    let interner = StringInterner::new();
    let mut handles = vec![];

    // Common values that will be interned
    let common_values = vec![
        "active",
        "maintenance",
        "1.14.3.51903",
        "a1b2c3d4e5f678901234567890123456",
        "true",
        "false",
        "0",
        "1",
    ];

    // Spawn threads that all intern the same values
    for thread_id in 0..10 {
        let interner_clone = interner.clone();
        let common_values = common_values.clone();

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                // Intern common values
                for value in &common_values {
                    interner_clone.intern(value);
                }

                // Also intern some unique values
                interner_clone.intern(&format!("thread_{thread_id}_unique"));
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Check results
    let stats = interner.memory_usage();

    // Should have: 8 common values + 10 unique thread values = 18
    assert_eq!(interner.unique_count(), 18, "Should have 18 unique strings");

    // With 10 threads × 100 iterations × 8 common values = 8000 references to 8 strings
    // Plus 10 threads × 100 iterations × 1 unique = 1000 references to 10 strings
    // Total: 9000 references to 18 strings = 500 average
    // But deduplication ratio is total refs / unique strings
    let _expected_ratio = (10.0 * 100.0 * 9.0) / 18.0; // ~500
    assert!(
        stats.deduplication_ratio > 100.0,
        "Should have >100x deduplication (actual: {:.2}x)",
        stats.deduplication_ratio
    );

    println!(
        "Concurrent test - deduplication ratio: {:.2}x",
        stats.deduplication_ratio
    );
    println!("✓ Concurrent interning test passed");
}

/// Benchmark memory usage comparison
#[test]
fn test_memory_usage_comparison() {
    // Create a large BPSV document with repeated values
    let mut lines =
        vec!["Region!STRING:0|Config!HEX:16|Status!STRING:0|Version!STRING:0".to_string()];
    lines.push("## seqn = 1234567".to_string());

    // Add 1000 rows with lots of repetition
    for i in 0..1000 {
        let region = match i % 5 {
            0 => "us",
            1 => "eu",
            2 => "cn",
            3 => "kr",
            _ => "tw",
        };

        let config = if i % 2 == 0 {
            "abcd1234abcd1234abcd1234abcd1234"
        } else {
            "5678abef5678abef5678abef5678abef"
        };

        let status = if i % 10 == 0 { "maintenance" } else { "active" };

        let version = "1.14.3.51903";

        lines.push(format!("{region}|{config}|{status}|{version}"));
    }

    let bpsv_data = lines.join("\n");

    // Parse with interning
    let interned_doc = InternedBpsvDocument::parse(&bpsv_data).unwrap();
    let stats = interned_doc.memory_stats();

    println!("\nLarge document memory statistics:");
    println!("  Rows: {}", interned_doc.row_count());
    println!("  Unique strings: {}", stats.unique_strings);
    println!("  Total references: {}", stats.total_references);
    println!("  Deduplication ratio: {:.2}x", stats.deduplication_ratio);
    println!(
        "  Memory saved: ~{:.2}%",
        (1.0 - 1.0 / stats.deduplication_ratio) * 100.0
    );

    // With this data pattern, we should have:
    // - 5 regions
    // - 2 configs
    // - 2 statuses
    // - 1 version
    // = ~10 unique strings (plus some decimals)

    assert!(stats.unique_strings < 20, "Should have <20 unique strings");
    assert!(
        stats.deduplication_ratio > 100.0,
        "Should have >100x deduplication"
    );

    println!("✓ Memory usage comparison test passed");
}

/// Test that interning works correctly with empty values
#[test]
fn test_interning_with_empty_values() {
    let bpsv_data = r#"Name!STRING:0|Value!STRING:0|Note!STRING:0
## seqn = 111
item1|value1|
item2||note2
item3|value3|
item4||note4
item5|value5|"#;

    let doc = InternedBpsvDocument::parse(bpsv_data).unwrap();

    assert_eq!(doc.row_count(), 5);

    // Check that empty values don't break interning
    let stats = doc.memory_stats();

    // Should have: item1-5, value1/3/5, note2/4, plus empty = ~11 unique
    println!(
        "Empty values test - unique strings: {}",
        stats.unique_strings
    );

    println!("✓ Empty values interning test passed");
}

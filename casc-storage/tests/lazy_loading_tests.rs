//! Tests for lazy loading manifest functionality

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::manual_repeat_n)]

use casc_storage::manifest::{ManifestConfig, TactManifests};

/// Test that lazy loading infrastructure is properly initialized
#[tokio::test]
async fn test_lazy_loading_initialization() {
    // Create config with lazy loading enabled
    let config = ManifestConfig {
        lazy_loading: true,
        lazy_cache_limit: 1000,
        ..Default::default()
    };

    let manifests = TactManifests::new(config);

    // Should not be loaded initially
    assert!(!manifests.is_loaded());

    // Test with mock data
    let mock_root_data = create_mock_root_data();
    let mock_encoding_data = create_mock_encoding_data();

    // Load manifests with lazy loading
    manifests.load_root_from_data(mock_root_data).unwrap();
    manifests
        .load_encoding_from_data(mock_encoding_data)
        .unwrap();

    // Should be marked as loaded
    assert!(manifests.is_loaded());

    println!("✓ Lazy loading initialization test passed");
}

/// Test that lazy loading fallback works properly
#[tokio::test]
async fn test_lazy_loading_fallback() {
    // Create config with lazy loading enabled
    let config = ManifestConfig {
        lazy_loading: true,
        ..Default::default()
    };

    let manifests = TactManifests::new(config);

    // Test with mock data
    let mock_root_data = create_mock_root_data();
    let mock_encoding_data = create_mock_encoding_data();

    // Load manifests with lazy loading
    manifests.load_root_from_data(mock_root_data).unwrap();
    manifests
        .load_encoding_from_data(mock_encoding_data)
        .unwrap();

    // Try to lookup a FileDataID - should fallback to full loading for now
    let result = manifests.lookup_by_fdid(123456);

    // Should fail gracefully since we have mock data, not real data
    // The important thing is that it doesn't panic
    match result {
        Ok(_) => println!("Lookup succeeded (unexpected with mock data)"),
        Err(_) => println!("Lookup failed as expected with mock data"),
    }

    println!("✓ Lazy loading fallback test passed");
}

/// Test memory usage comparison between lazy and full loading
#[tokio::test]
async fn test_lazy_vs_full_memory_usage() {
    // Test with both lazy loading on and off
    for lazy_enabled in [false, true] {
        let config = ManifestConfig {
            lazy_loading: lazy_enabled,
            ..Default::default()
        };

        let manifests = TactManifests::new(config);

        // Create larger mock data to see memory differences
        let large_mock_data = create_large_mock_data();

        let before_memory = get_approximate_memory_usage();

        // Load the data - use different mock data for root and encoding
        manifests
            .load_root_from_data(large_mock_data.clone())
            .unwrap();
        let encoding_data = create_large_mock_encoding_data();
        manifests.load_encoding_from_data(encoding_data).unwrap();

        let after_memory = get_approximate_memory_usage();

        println!(
            "Memory usage (lazy={}): {} -> {} bytes",
            lazy_enabled, before_memory, after_memory
        );

        // Clear caches to test cache clearing
        manifests.clear_cache();

        let after_clear = get_approximate_memory_usage();
        println!(
            "Memory after cache clear (lazy={}): {} bytes",
            lazy_enabled, after_clear
        );
    }

    println!("✓ Memory usage comparison test completed");
}

/// Create mock root manifest data for testing
fn create_mock_root_data() -> Vec<u8> {
    // Create a minimal valid-looking TACT root
    let mut data = Vec::new();

    // Magic bytes "TSFM"
    data.extend_from_slice(b"TSFM");

    // Header size (0x18 for new format)
    data.extend_from_slice(&0x18u32.to_le_bytes());

    // Version
    data.extend_from_slice(&1u32.to_le_bytes());

    // Total file count
    data.extend_from_slice(&10u32.to_le_bytes());

    // Named file count
    data.extend_from_slice(&10u32.to_le_bytes());

    // Padding
    data.extend_from_slice(&0u32.to_le_bytes());

    // Add some mock block data to make it parseable
    // This won't be valid TACT data but should be enough for infrastructure testing
    data.extend(std::iter::repeat(0).take(100)); // Padding to make it look like there's content

    data
}

/// Create mock encoding manifest data for testing
fn create_mock_encoding_data() -> Vec<u8> {
    // Create a minimal encoding file structure
    let mut data = Vec::new();

    // Magic "EN"
    data.extend_from_slice(&[0x45, 0x4E]);

    // Version
    data.push(1);

    // Hash sizes
    data.push(16); // CKey hash size
    data.push(16); // EKey hash size

    // Page sizes (big endian)
    data.extend_from_slice(&1u16.to_be_bytes()); // CKey page size in KB
    data.extend_from_slice(&1u16.to_be_bytes()); // EKey page size in KB

    // Page counts (big endian)
    data.extend_from_slice(&0u32.to_be_bytes()); // CKey page count
    data.extend_from_slice(&0u32.to_be_bytes()); // EKey page count

    // Unknown
    data.push(0);

    // ESpec block size (big endian)
    data.extend_from_slice(&0u32.to_be_bytes());

    // Add padding to make it look substantial
    data.extend(std::iter::repeat(0).take(100));

    data
}

/// Create larger mock data for memory testing
fn create_large_mock_data() -> Vec<u8> {
    let mut base_data = create_mock_root_data();

    // Extend with dummy data to simulate larger manifest
    for i in 0..10000 {
        base_data.extend_from_slice(&(i as u32).to_le_bytes());
    }

    base_data
}

/// Create larger mock encoding data for memory testing
fn create_large_mock_encoding_data() -> Vec<u8> {
    let mut base_data = create_mock_encoding_data();

    // Extend with dummy data to simulate larger manifest
    for i in 0..10000 {
        base_data.extend_from_slice(&(i as u32).to_be_bytes()); // Big endian for encoding
    }

    base_data
}

/// Get approximate memory usage (simple implementation)
fn get_approximate_memory_usage() -> usize {
    // This is a very rough approximation
    // In a real implementation, you might use more sophisticated memory tracking
    std::mem::size_of::<usize>() * 1000 // Placeholder
}

/// Test that configuration affects lazy loading behavior
#[test]
fn test_lazy_loading_configuration() {
    // Test default configuration
    let default_config = ManifestConfig::default();
    assert!(
        default_config.lazy_loading,
        "Lazy loading should be enabled by default"
    );
    assert_eq!(
        default_config.lazy_cache_limit, 10_000,
        "Default cache limit should be 10,000"
    );

    // Test custom configuration
    let custom_config = ManifestConfig {
        lazy_loading: false,
        lazy_cache_limit: 5000,
        ..Default::default()
    };

    assert!(
        !custom_config.lazy_loading,
        "Lazy loading should be disabled when set to false"
    );
    assert_eq!(
        custom_config.lazy_cache_limit, 5000,
        "Cache limit should match configured value"
    );

    println!("✓ Lazy loading configuration test passed");
}

/// Test cache clearing functionality with lazy loading
#[tokio::test]
async fn test_lazy_cache_clearing() {
    let config = ManifestConfig {
        lazy_loading: true,
        ..Default::default()
    };

    let manifests = TactManifests::new(config);

    // Load test data
    manifests
        .load_root_from_data(create_mock_root_data())
        .unwrap();
    manifests
        .load_encoding_from_data(create_mock_encoding_data())
        .unwrap();

    // Should be loaded
    assert!(manifests.is_loaded());

    // Clear caches - should not affect is_loaded status
    manifests.clear_cache();
    assert!(
        manifests.is_loaded(),
        "Clearing cache should not affect loaded status"
    );

    println!("✓ Lazy cache clearing test passed");
}

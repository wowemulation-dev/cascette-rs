//! Integration test for progressive file loading

use casc_storage::types::CascConfig;
use casc_storage::{CascStorage, EKey, ProgressiveConfig, SizeHint};
use tempfile::TempDir;

#[tokio::test]
async fn test_progressive_loading_initialization() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();

    // Create config
    let config = CascConfig {
        data_path: data_path.clone(),
        cache_size_mb: 10,
        max_archive_size: 100 * 1024 * 1024,
        use_memory_mapping: false,
        read_only: false,
    };

    // Create storage
    let mut storage = CascStorage::new(config).unwrap();

    // Configure progressive loading
    let progressive_config = ProgressiveConfig {
        chunk_size: 256 * 1024,
        max_prefetch_chunks: 4,
        chunk_timeout: std::time::Duration::from_secs(30),
        use_predictive_prefetch: true,
        min_progressive_size: 1024 * 1024,
    };

    // Initialize progressive loading
    storage.init_progressive_loading(progressive_config);

    // Verify it's initialized
    assert!(storage.has_progressive_loading());
}

#[tokio::test]
async fn test_progressive_file_creation() {
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();

    // Create config
    let config = CascConfig {
        data_path: data_path.clone(),
        cache_size_mb: 10,
        max_archive_size: 100 * 1024 * 1024,
        use_memory_mapping: false,
        read_only: false,
    };

    // Create storage
    let mut storage = CascStorage::new(config).unwrap();

    // Configure progressive loading
    let progressive_config = ProgressiveConfig::default();
    storage.init_progressive_loading(progressive_config);

    // Create a test EKey
    let ekey = EKey::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);

    // Try to create a progressive file (it will succeed even without data)
    let result = storage.read_progressive(&ekey, SizeHint::Unknown).await;

    // The API should return Ok with a progressive file handle
    assert!(result.is_ok());

    // But reading from it should fail since we have no actual data
    if let Ok(progressive_file) = result {
        let read_result = progressive_file.read(0, 100).await;
        assert!(read_result.is_err());
    }
}

#[tokio::test]
async fn test_progressive_stats() {
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().to_path_buf();

    // Create config
    let config = CascConfig {
        data_path: data_path.clone(),
        cache_size_mb: 10,
        max_archive_size: 100 * 1024 * 1024,
        use_memory_mapping: false,
        read_only: false,
    };

    // Create storage
    let mut storage = CascStorage::new(config).unwrap();

    // Configure progressive loading
    let progressive_config = ProgressiveConfig::default();
    storage.init_progressive_loading(progressive_config);

    // Get stats (should be empty)
    let stats = storage.get_progressive_stats().await;
    assert_eq!(stats.len(), 0);

    // Cleanup
    storage.cleanup_progressive_files().await;
}

#[test]
fn test_size_hint_logic() {
    let config = ProgressiveConfig::default();

    // Test exact size hints
    assert!(SizeHint::Exact(2_000_000).should_use_progressive(&config));
    assert!(!SizeHint::Exact(500_000).should_use_progressive(&config));

    // Test estimated size hints
    assert!(
        SizeHint::Estimated {
            size: 2_000_000,
            confidence: 0.8
        }
        .should_use_progressive(&config)
    );

    assert!(
        !SizeHint::Estimated {
            size: 2_000_000,
            confidence: 0.3
        }
        .should_use_progressive(&config)
    );

    // Test minimum size hints
    assert!(SizeHint::Minimum(2_000_000).should_use_progressive(&config));
    assert!(!SizeHint::Minimum(500_000).should_use_progressive(&config));

    // Test unknown size hint
    assert!(!SizeHint::Unknown.should_use_progressive(&config));
}

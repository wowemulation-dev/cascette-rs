//! Tests for progressive file loading functionality

use casc_storage::*;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test CascStorage with sample data
async fn create_test_storage() -> (CascStorage, TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let data_path = temp_dir.path().to_path_buf();

    // Create directory structure
    let data_subdir = data_path.join("data");
    std::fs::create_dir_all(&data_subdir).expect("Failed to create data directory");

    let config = types::CascConfig {
        data_path,
        read_only: false,
        cache_size_mb: 128,
        max_archive_size: 16 * 1024 * 1024, // 16MB
        use_memory_mapping: true,
    };

    let storage = CascStorage::new(config).expect("Failed to create CASC storage");
    (storage, temp_dir)
}

/// Create sample data for testing
fn create_sample_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

#[tokio::test]
async fn test_progressive_config_defaults() {
    let config = ProgressiveConfig::default();
    
    assert_eq!(config.chunk_size, 256 * 1024); // 256KB
    assert_eq!(config.max_prefetch_chunks, 4);
    assert_eq!(config.min_progressive_size, 1024 * 1024); // 1MB
    assert!(config.use_predictive_prefetch);
}

#[tokio::test]
async fn test_size_hint_logic() {
    let config = ProgressiveConfig::default();
    
    // Exact size - should use progressive for large files
    let large_exact = SizeHint::Exact(2 * 1024 * 1024); // 2MB
    assert!(large_exact.should_use_progressive(&config));
    assert_eq!(large_exact.suggested_initial_size(), Some(2 * 1024 * 1024));
    
    // Small file - should not use progressive
    let small_exact = SizeHint::Exact(500 * 1024); // 500KB
    assert!(!small_exact.should_use_progressive(&config));
    
    // High confidence estimate
    let high_confidence = SizeHint::Estimated {
        size: 3 * 1024 * 1024,
        confidence: 0.9,
    };
    assert!(high_confidence.should_use_progressive(&config));
    assert_eq!(high_confidence.suggested_initial_size(), Some(3 * 1024 * 1024));
    
    // Low confidence estimate - should not use progressive
    let low_confidence = SizeHint::Estimated {
        size: 3 * 1024 * 1024,
        confidence: 0.3,
    };
    assert!(!low_confidence.should_use_progressive(&config));
    assert_eq!(low_confidence.suggested_initial_size(), None);
    
    // Minimum size
    let minimum = SizeHint::Minimum(2 * 1024 * 1024); // Needs to be > min_progressive_size
    assert!(minimum.should_use_progressive(&config));
    assert_eq!(minimum.suggested_initial_size(), Some(2 * 1024 * 1024));
    
    // Unknown size
    let unknown = SizeHint::Unknown;
    assert!(!unknown.should_use_progressive(&config));
    assert_eq!(unknown.suggested_initial_size(), None);
}

#[tokio::test]
async fn test_progressive_loading_initialization() {
    let (mut storage, _temp_dir) = create_test_storage().await;
    
    // Initially, progressive loading should not be available
    assert!(!storage.has_progressive_loading());
    
    // Initialize progressive loading
    let config = ProgressiveConfig::default();
    storage.init_progressive_loading(config);
    
    // Now it should be available
    assert!(storage.has_progressive_loading());
}

#[tokio::test]
async fn test_progressive_file_with_mock_data() {
    use casc_storage::progressive::{ChunkLoader, ProgressiveFile, SizeHint};
    use casc_storage::types::EKey;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    // Mock chunk loader for testing
    struct MockChunkLoader {
        total_size: usize,
        chunk_load_count: Arc<AtomicUsize>,
    }
    
    #[async_trait::async_trait]
    impl ChunkLoader for MockChunkLoader {
        async fn load_chunk(&self, _ekey: EKey, offset: u64, size: usize) -> Result<Vec<u8>> {
            self.chunk_load_count.fetch_add(1, Ordering::SeqCst);
            
            let start = offset as usize;
            let end = (start + size).min(self.total_size);
            
            if start >= self.total_size {
                return Ok(Vec::new());
            }
            
            // Generate deterministic data
            let data: Vec<u8> = (start..end).map(|i| (i % 256) as u8).collect();
            
            // Simulate loading delay
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            
            Ok(data)
        }
    }
    
    let total_size = 4096; // 4KB test file
    let ekey = EKey::new([1; 16]);
    let size_hint = SizeHint::Exact(total_size as u64);
    let config = ProgressiveConfig {
        chunk_size: 1024, // 1KB chunks
        max_prefetch_chunks: 2,
        ..ProgressiveConfig::default()
    };
    
    let chunk_load_count = Arc::new(AtomicUsize::new(0));
    let loader = Arc::new(MockChunkLoader {
        total_size,
        chunk_load_count: Arc::clone(&chunk_load_count),
    });
    
    let progressive_file = ProgressiveFile::new(
        ekey,
        size_hint,
        config,
        Arc::downgrade(&loader) as std::sync::Weak<dyn ChunkLoader + Send + Sync>,
    );
    
    // Read from beginning
    let data1 = progressive_file.read(0, 512).await.expect("Failed to read");
    assert_eq!(data1.len(), 512);
    assert_eq!(data1[0], 0);
    assert_eq!(data1[255], 255);
    assert_eq!(data1[511], 255); // (511 % 256) = 255
    
    // Should have loaded at least one chunk
    assert!(chunk_load_count.load(Ordering::SeqCst) > 0);
    let initial_loads = chunk_load_count.load(Ordering::SeqCst);
    
    // Read from same chunk - should be cached
    let data2 = progressive_file.read(100, 300).await.expect("Failed to read");
    assert_eq!(data2.len(), 300);
    assert_eq!(data2[0], (100 % 256) as u8); // Should start at offset 100
    
    // Should not have loaded additional chunks
    assert_eq!(chunk_load_count.load(Ordering::SeqCst), initial_loads);
    
    // Read across chunk boundary
    let data3 = progressive_file.read(800, 500).await.expect("Failed to read");
    assert_eq!(data3.len(), 500);
    assert_eq!(data3[0], (800 % 256) as u8); // Should start at offset 800
    
    // Should have loaded more chunks now
    assert!(chunk_load_count.load(Ordering::SeqCst) > initial_loads);
    
    // Check statistics
    let stats = progressive_file.get_stats().await;
    assert!(stats.chunks_loaded > 0);
    assert!(stats.bytes_loaded > 0);
    assert!(stats.cache_hits > 0 || stats.cache_misses > 0);
    
    println!("Progressive loading stats: {:#?}", stats);
}

#[tokio::test]
async fn test_progressive_loading_with_real_storage() {
    let (mut storage, _temp_dir) = create_test_storage().await;
    
    // Create and store some test data
    let test_data = create_sample_data(5 * 1024 * 1024); // 5MB test file
    let test_ekey = EKey::new([42; 16]);
    
    // Initialize progressive loading
    let config = ProgressiveConfig {
        chunk_size: 256 * 1024, // 256KB chunks
        max_prefetch_chunks: 3,
        min_progressive_size: 1024 * 1024, // 1MB minimum
        ..ProgressiveConfig::default()
    };
    storage.init_progressive_loading(config);
    
    // Write the test data to storage
    storage.write(&test_ekey, &test_data).expect("Failed to write test data");
    
    // Flush to ensure data is written
    storage.flush().expect("Failed to flush storage");
    
    // Load archives to make the data available
    storage.load_archives().expect("Failed to load archives");
    
    // Get size hint from storage
    let size_hint = storage.get_size_hint_for_ekey(&test_ekey);
    println!("Size hint for test file: {:?}", size_hint);
    
    // Create progressive file
    let progressive_file = storage
        .read_progressive(&test_ekey, SizeHint::Exact(test_data.len() as u64))
        .await
        .expect("Failed to create progressive file");
    
    // Read data progressively
    let chunk1 = progressive_file.read(0, 1024).await.expect("Failed to read chunk 1");
    assert_eq!(chunk1.len(), 1024);
    assert_eq!(chunk1, test_data[0..1024]);
    
    let chunk2 = progressive_file.read(100_000, 2048).await.expect("Failed to read chunk 2");
    assert_eq!(chunk2.len(), 2048);
    assert_eq!(chunk2, test_data[100_000..102_048]);
    
    // Check that we can read near the end of the file
    let end_offset = test_data.len() - 1024;
    let chunk3 = progressive_file.read(end_offset as u64, 1024).await.expect("Failed to read from end");
    assert_eq!(chunk3.len(), 1024);
    assert_eq!(chunk3, test_data[end_offset..]);
    
    // Verify statistics
    let stats = progressive_file.get_stats().await;
    println!("Real storage progressive loading stats: {:#?}", stats);
    assert!(stats.chunks_loaded > 0);
    assert!(stats.bytes_loaded > 0);
}

#[tokio::test] 
async fn test_size_hint_from_manifest() {
    let (mut storage, _temp_dir) = create_test_storage().await;
    
    // Initialize TACT manifests 
    let manifest_config = manifest::ManifestConfig::default();
    storage.init_tact_manifests(manifest_config);
    
    // Initialize progressive loading
    storage.init_progressive_loading(ProgressiveConfig::default());
    
    // Create mock manifest data
    // This would normally be loaded from actual TACT manifest files
    // For now, just test that the API works
    
    // Test reading by FileDataID (would fail since no manifests loaded, but tests the API)
    let result = storage.read_by_fdid_progressive(12345).await;
    assert!(result.is_err()); // Expected since no manifest data loaded
    
    // Test that TACT manifests are initialized
    assert!(!storage.tact_manifests_loaded()); // No actual data loaded yet
}

#[tokio::test]
async fn test_progressive_cleanup_and_stats() {
    let (mut storage, _temp_dir) = create_test_storage().await;
    
    // Initialize progressive loading
    storage.init_progressive_loading(ProgressiveConfig::default());
    
    // Create some test files
    let test_data1 = create_sample_data(2 * 1024 * 1024);
    let test_data2 = create_sample_data(3 * 1024 * 1024);
    let ekey1 = EKey::new([1; 16]);
    let ekey2 = EKey::new([2; 16]);
    
    storage.write(&ekey1, &test_data1).expect("Failed to write test data 1");
    storage.write(&ekey2, &test_data2).expect("Failed to write test data 2");
    storage.flush().expect("Failed to flush storage");
    storage.load_archives().expect("Failed to load archives");
    
    // Create progressive files
    let _file1 = storage
        .read_progressive(&ekey1, SizeHint::Exact(test_data1.len() as u64))
        .await
        .expect("Failed to create progressive file 1");
    
    let _file2 = storage
        .read_progressive(&ekey2, SizeHint::Exact(test_data2.len() as u64))
        .await
        .expect("Failed to create progressive file 2");
    
    // Trigger some reads to generate stats
    let _ = _file1.read(0, 1024).await;
    let _ = _file2.read(0, 1024).await;
    
    // Get global progressive stats
    let global_stats = storage.get_progressive_stats().await;
    println!("Global progressive stats: {:#?}", global_stats);
    assert_eq!(global_stats.len(), 2); // Should have stats for both files
    
    // Test cleanup (won't remove files since they're recently accessed)
    storage.cleanup_progressive_files().await;
    
    // Verify cleanup didn't remove active files
    let stats_after_cleanup = storage.get_progressive_stats().await;
    assert_eq!(stats_after_cleanup.len(), 2);
}

#[tokio::test]
async fn test_concurrent_progressive_reads() {
    let (mut storage, _temp_dir) = create_test_storage().await;
    
    // Initialize progressive loading with higher concurrency
    let config = ProgressiveConfig {
        chunk_size: 128 * 1024, // 128KB chunks
        max_prefetch_chunks: 4,
        ..ProgressiveConfig::default()
    };
    storage.init_progressive_loading(config);
    
    // Create a large test file
    let test_data = create_sample_data(8 * 1024 * 1024); // 8MB
    let test_ekey = EKey::new([99; 16]);
    storage.write(&test_ekey, &test_data).expect("Failed to write test data");
    storage.flush().expect("Failed to flush storage");
    storage.load_archives().expect("Failed to load archives");
    
    // Create progressive file
    let progressive_file = storage
        .read_progressive(&test_ekey, SizeHint::Exact(test_data.len() as u64))
        .await
        .expect("Failed to create progressive file");
    
    // Perform concurrent reads from different parts of the file
    let file1 = progressive_file.clone();
    let file2 = progressive_file.clone();
    let file3 = progressive_file.clone();
    
    let expected_data = test_data.clone();
    
    let (result1, result2, result3) = tokio::join!(
        async {
            let data = file1.read(0, 2048).await.expect("Concurrent read 1 failed");
            assert_eq!(data, expected_data[0..2048]);
            data.len()
        },
        async {
            let data = file2.read(1_000_000, 2048).await.expect("Concurrent read 2 failed");
            assert_eq!(data, expected_data[1_000_000..1_002_048]);
            data.len()
        },
        async {
            let data = file3.read(4_000_000, 2048).await.expect("Concurrent read 3 failed");
            assert_eq!(data, expected_data[4_000_000..4_002_048]);
            data.len()
        }
    );
    
    // All reads should have succeeded
    assert_eq!(result1, 2048);
    assert_eq!(result2, 2048);
    assert_eq!(result3, 2048);
    
    // Check final statistics
    let final_stats = progressive_file.get_stats().await;
    println!("Concurrent reads stats: {:#?}", final_stats);
    assert!(final_stats.chunks_loaded > 0);
    assert!(final_stats.bytes_loaded > 6144); // At least 3 * 2048 bytes
}

#[tokio::test]
async fn test_progressive_vs_traditional_performance() {
    let (mut storage, _temp_dir) = create_test_storage().await;
    
    // Initialize progressive loading
    storage.init_progressive_loading(ProgressiveConfig::default());
    
    // Create test data
    let test_data = create_sample_data(4 * 1024 * 1024); // 4MB
    let test_ekey = EKey::new([123; 16]);
    storage.write(&test_ekey, &test_data).expect("Failed to write test data");
    storage.flush().expect("Failed to flush storage");
    storage.load_archives().expect("Failed to load archives");
    
    // Time traditional read
    let start = std::time::Instant::now();
    let traditional_data = storage.read(&test_ekey).expect("Traditional read failed");
    let traditional_time = start.elapsed();
    
    // Time progressive read (full file)
    let progressive_file = storage
        .read_progressive(&test_ekey, SizeHint::Exact(test_data.len() as u64))
        .await
        .expect("Failed to create progressive file");
    
    let start = std::time::Instant::now();
    let progressive_data = progressive_file.read(0, test_data.len()).await.expect("Progressive read failed");
    let progressive_time = start.elapsed();
    
    // Verify data is identical
    assert_eq!(traditional_data.len(), progressive_data.len());
    assert_eq!(traditional_data, progressive_data);
    
    println!("Traditional read time: {:?}", traditional_time);
    println!("Progressive read time: {:?}", progressive_time);
    
    // Progressive reading should be reasonably competitive
    // (might be slower for full reads due to chunking overhead, but should be close)
    let ratio = progressive_time.as_nanos() as f64 / traditional_time.as_nanos() as f64;
    println!("Progressive/Traditional ratio: {:.2}x", ratio);
    
    // For full file reads, progressive should be within 3x of traditional
    // The main benefit is for partial reads and memory usage
    assert!(ratio < 3.0, "Progressive reading is too slow compared to traditional");
    
    // Test partial read performance (where progressive should shine)
    let start = std::time::Instant::now();
    let partial_data = progressive_file.read(1_000_000, 4096).await.expect("Partial read failed");
    let partial_time = start.elapsed();
    
    assert_eq!(partial_data.len(), 4096);
    assert_eq!(partial_data, test_data[1_000_000..1_004_096]);
    
    println!("Progressive partial read time: {:?}", partial_time);
    
    // Partial reads should be fast since they don't need to load the entire file
    assert!(partial_time < traditional_time / 2, "Partial progressive read should be much faster");
}
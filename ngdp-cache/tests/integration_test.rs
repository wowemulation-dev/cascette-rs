//! Integration tests for ngdp-cache

use ngdp_cache::{cdn::CdnCache, generic::GenericCache, ribbit::RibbitCache, tact::TactCache};
use std::time::Duration;

/// Test data for various scenarios
const TEST_CONFIG_DATA: &[u8] =
    b"root = 9e3dfbafb41949c8cb14e0bc0055d225 70c91468bb187cc2b3d045d476c6899f
encoding = e468c86f90cd051195a3c5f8b08d7bd7 12ad2799f3e1ee9a9b5620e43a0d2b75
install = 17adc9e821c34e06ba6f4568aab0c040 9a127c8076a2c1b24fa3a97b0f5346d8
download = f2c3b74f3c51db3a5c4e2d87c52a0c82 24e1cd9ec87419dd826e991fa141c6e0";

const TEST_ARCHIVE_DATA: &[u8] =
    b"BLTE\x00\x00\x00\x10\x00\x00\x00\x00\x00\x00\x00\x00This is test archive data";

const TEST_RIBBIT_RESPONSE: &[u8] =
    b"Region!STRING:0|BuildConfig!HEX:32|CDNConfig!HEX:32|BuildId!DEC:10|VersionsName!STRING:0
## seqn = 3016450
us|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|61491|11.1.7.61491";

#[tokio::test]
async fn test_cache_directory_creation() {
    // Test that cache directories are created properly
    let generic = GenericCache::new().await.unwrap();
    assert!(generic.base_dir().exists());

    let tact = TactCache::new().await.unwrap();
    assert!(tact.base_dir().exists());

    let cdn = CdnCache::new().await.unwrap();
    assert!(cdn.base_dir().exists());

    let ribbit = RibbitCache::new().await.unwrap();
    assert!(ribbit.base_dir().exists());
}

#[tokio::test]
async fn test_cross_cache_isolation() {
    // Test that different cache types don't interfere with each other
    let generic = GenericCache::new().await.unwrap();
    let tact = TactCache::new().await.unwrap();

    // Write to generic cache
    generic.write("test_key", b"generic data").await.unwrap();

    // Ensure it doesn't exist in TACT cache
    assert!(!tact.has_config("test_key").await);
    assert!(!tact.has_data("test_key").await);

    // Cleanup
    generic.delete("test_key").await.unwrap();
}

#[tokio::test]
async fn test_tact_cache_workflow() {
    let tact = TactCache::new().await.unwrap();

    // Simulate real TACT workflow
    let build_config_hash = "be2bb98dc28aee05bbee519393696cdb";
    let cdn_config_hash = "fac77b9ca52c84ac28ad83a7dbe1c829";
    let index_hash = "0052ea9a56fd7b3b6fe7d1d906e6cdef";

    // Write configs
    tact.write_config(build_config_hash, TEST_CONFIG_DATA)
        .await
        .unwrap();
    tact.write_config(
        cdn_config_hash,
        b"archives = 8a41b9e8bf2d85ad73e087c446c655fb",
    )
    .await
    .unwrap();

    // Write index
    tact.write_index(index_hash, b"binary index data")
        .await
        .unwrap();

    // Write data
    let data_hash = "1234567890abcdef1234567890abcdef";
    tact.write_data(data_hash, b"game data").await.unwrap();

    // Verify all exist
    assert!(tact.has_config(build_config_hash).await);
    assert!(tact.has_config(cdn_config_hash).await);
    assert!(tact.has_index(index_hash).await);
    assert!(tact.has_data(data_hash).await);

    // Read and verify
    let config_data = tact.read_config(build_config_hash).await.unwrap();
    assert_eq!(config_data, TEST_CONFIG_DATA);

    // Cleanup
    tokio::fs::remove_file(tact.config_path(build_config_hash))
        .await
        .ok();
    tokio::fs::remove_file(tact.config_path(cdn_config_hash))
        .await
        .ok();
    tokio::fs::remove_file(tact.index_path(index_hash))
        .await
        .ok();
    tokio::fs::remove_file(tact.data_path(data_hash)).await.ok();
}

#[tokio::test]
async fn test_cdn_cache_product_separation() {
    // Test that different products have separate caches
    let wow_cache = CdnCache::for_product("wow").await.unwrap();
    let d4_cache = CdnCache::for_product("d4").await.unwrap();

    let archive_hash = "deadbeef1234567890abcdef12345678";

    // Write to WoW cache
    wow_cache
        .write_archive(archive_hash, TEST_ARCHIVE_DATA)
        .await
        .unwrap();

    // Ensure it doesn't exist in D4 cache
    assert!(!d4_cache.has_archive(archive_hash).await);

    // But exists in WoW cache
    assert!(wow_cache.has_archive(archive_hash).await);

    // Cleanup
    tokio::fs::remove_file(wow_cache.archive_path(archive_hash))
        .await
        .ok();
}

#[tokio::test]
async fn test_cdn_cache_streaming() {
    let cdn = CdnCache::new().await.unwrap();
    let archive_hash = "streamtest1234567890abcdef123456";

    // Write large data
    let large_data = vec![0u8; 1024 * 1024]; // 1MB
    cdn.write_archive(archive_hash, &large_data).await.unwrap();

    // Test streaming read
    let mut file = cdn.open_archive(archive_hash).await.unwrap();
    let mut buffer = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut file, &mut buffer)
        .await
        .unwrap();

    assert_eq!(buffer.len(), large_data.len());

    // Cleanup
    tokio::fs::remove_file(cdn.archive_path(archive_hash))
        .await
        .ok();
}

#[tokio::test]
async fn test_ribbit_cache_expiration() {
    // Test cache with reasonable TTL for testing
    let cache = RibbitCache::with_ttl(Duration::from_secs(1)).await.unwrap();

    let region = "us";
    let product = "wow_test";
    let endpoint = "versions_test";

    // Write data
    cache
        .write(region, product, endpoint, TEST_RIBBIT_RESPONSE)
        .await
        .unwrap();

    // Check that files were created
    let cache_path = cache.cache_path(region, product, endpoint);
    let meta_path = cache.metadata_path(region, product, endpoint);
    assert!(cache_path.exists(), "Cache file should exist after write");
    assert!(meta_path.exists(), "Metadata file should exist after write");

    // Should be valid immediately
    assert!(
        cache.is_valid(region, product, endpoint).await,
        "Cache should be valid immediately after write"
    );

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Should be expired
    assert!(
        !cache.is_valid(region, product, endpoint).await,
        "Cache should be expired after TTL"
    );

    // Clear expired entries
    cache.clear_expired().await.unwrap();

    // Verify the files are actually gone
    assert!(
        !cache_path.exists(),
        "Cache file should be deleted after clear_expired"
    );
    assert!(
        !meta_path.exists(),
        "Metadata file should be deleted after clear_expired"
    );
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let _cache = GenericCache::new().await.unwrap();

    // Spawn multiple tasks writing to different keys
    let mut handles = vec![];

    for i in 0..10 {
        let cache_clone = GenericCache::new().await.unwrap();
        let handle = tokio::spawn(async move {
            let key = format!("concurrent_key_{}", i);
            let data = format!("data_{}", i);
            cache_clone.write(&key, data.as_bytes()).await.unwrap();

            // Read it back
            let read_data = cache_clone.read(&key).await.unwrap();
            assert_eq!(read_data, data.as_bytes());

            // Cleanup
            cache_clone.delete(&key).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_cache_corruption_detection() {
    let cache = GenericCache::new().await.unwrap();
    let key = "corruption_test";

    // Write valid data
    cache.write(key, b"valid data").await.unwrap();

    // Corrupt the file by writing invalid data directly
    let path = cache.get_path(key);
    tokio::fs::write(&path, b"").await.unwrap(); // Empty file

    // Try to read - should get empty data
    let data = cache.read(key).await.unwrap();
    assert_eq!(data.len(), 0);

    // Cleanup
    cache.delete(key).await.unwrap();
}

#[tokio::test]
async fn test_cache_key_validation() {
    let cache = GenericCache::new().await.unwrap();

    // Test various key formats
    let test_keys = vec![
        "simple_key",
        "key_with_numbers_123",
        "key-with-dashes",
        "key.with.dots",
        "UPPERCASE_KEY",
        "key/with/slashes", // This creates subdirectories
    ];

    for key in test_keys {
        let data = format!("data for {}", key).into_bytes();

        // Should handle all keys
        cache.write(key, &data).await.unwrap();
        let read_data = cache.read(key).await.unwrap();
        assert_eq!(read_data, data);

        // Cleanup
        cache.delete(key).await.unwrap();
    }
}

#[tokio::test]
async fn test_large_file_handling() {
    let cdn = CdnCache::new().await.unwrap();

    // Test with a file larger than typical buffer sizes
    let large_hash = "largefiletest567890abcdef1234567";
    let size = 10 * 1024 * 1024; // 10MB
    let large_data = vec![42u8; size];

    // Write large file
    cdn.write_archive(large_hash, &large_data).await.unwrap();

    // Verify size
    let reported_size = cdn.archive_size(large_hash).await.unwrap();
    assert_eq!(reported_size, size as u64);

    // Read it back
    let read_data = cdn.read_archive(large_hash).await.unwrap();
    assert_eq!(read_data.len(), size);
    assert_eq!(read_data[0], 42);
    assert_eq!(read_data[size - 1], 42);

    // Cleanup
    tokio::fs::remove_file(cdn.archive_path(large_hash))
        .await
        .ok();
}

#[tokio::test]
async fn test_cache_clear_operations() {
    let cache = GenericCache::with_subdirectory("clear_test").await.unwrap();

    // Write multiple entries
    for i in 0..5 {
        let key = format!("clear_key_{}", i);
        cache.write(&key, b"data").await.unwrap();
    }

    // Verify all exist
    for i in 0..5 {
        let key = format!("clear_key_{}", i);
        assert!(cache.exists(&key).await);
    }

    // Clear all
    cache.clear().await.unwrap();

    // Verify all are gone
    for i in 0..5 {
        let key = format!("clear_key_{}", i);
        assert!(!cache.exists(&key).await);
    }
}

#[tokio::test]
async fn test_nested_directory_creation() {
    let tact = TactCache::new().await.unwrap();

    // Use a hash that will create nested directories
    let deeply_nested_hash = "abcdef0123456789abcdef0123456789";

    // This should create all parent directories
    tact.write_data(deeply_nested_hash, b"nested data")
        .await
        .unwrap();

    // Verify it exists
    assert!(tact.has_data(deeply_nested_hash).await);

    // Read it back
    let data = tact.read_data(deeply_nested_hash).await.unwrap();
    assert_eq!(data, b"nested data");

    // Cleanup
    tokio::fs::remove_file(tact.data_path(deeply_nested_hash))
        .await
        .ok();
}

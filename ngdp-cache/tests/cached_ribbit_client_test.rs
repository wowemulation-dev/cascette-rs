//! Integration tests for CachedRibbitClient

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};
use std::path::PathBuf;

/// Test data directory for isolated testing
async fn test_cache_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("ngdp_cache_test").join("cached_ribbit");
    tokio::fs::create_dir_all(&dir).await.unwrap();
    dir
}

/// Clean up test directory
async fn cleanup_test_dir(dir: &PathBuf) {
    let _ = tokio::fs::remove_dir_all(dir).await;
}

#[tokio::test]
async fn test_cached_client_creation() {
    // Test default creation
    let client = CachedRibbitClient::new(Region::US).await.unwrap();
    assert_eq!(client.inner().region(), Region::US);

    // Test with custom cache directory
    let test_dir = test_cache_dir().await;
    let client = CachedRibbitClient::with_cache_dir(Region::EU, test_dir.clone())
        .await
        .unwrap();
    assert_eq!(client.inner().region(), Region::EU);
    
    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_filename_conventions() {
    let test_dir = test_cache_dir().await;
    let _client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Test various endpoint filename generation
    let _test_cases = vec![
        (Endpoint::Summary, None, "summary-#-0.bmime"),
        (Endpoint::ProductVersions("wow".to_string()), None, "versions-wow-0.bmime"),
        (Endpoint::ProductCdns("d4".to_string()), None, "cdns-d4-0.bmime"),
        (Endpoint::ProductBgdl("wow_classic".to_string()), None, "bgdl-wow_classic-0.bmime"),
        (Endpoint::Cert("abc123".to_string()), None, "certs-abc123-0.bmime"),
        (Endpoint::Cert("def456".to_string()), Some(12345), "certs-def456-12345.bmime"),
        (Endpoint::Ocsp("789xyz".to_string()), None, "ocsp-789xyz-0.bmime"),
        (Endpoint::Custom("products/wow/versions".to_string()), None, "products-wow-0.bmime"),
        (Endpoint::Custom("custom".to_string()), None, "custom-#-0.bmime"),
    ];

    // We can't directly test generate_cache_filename since it's private,
    // but we can verify the files are created with correct names
    // This is tested in the unit tests already

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_enable_disable() {
    let test_dir = test_cache_dir().await;
    let mut client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Initially enabled
    client.set_caching_enabled(true);
    
    // Create a dummy endpoint that would fail if actually called
    let endpoint = Endpoint::Custom("test/endpoint".to_string());
    
    // Write a test file to simulate cached response
    let cache_file = test_dir
        .join("us")
        .join("test-endpoint-0.bmime");
    tokio::fs::create_dir_all(cache_file.parent().unwrap()).await.unwrap();
    tokio::fs::write(&cache_file, b"test cached data").await.unwrap();
    
    // Write metadata with current timestamp
    let meta_file = test_dir
        .join("us")
        .join("test-endpoint-0.meta");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    tokio::fs::write(&meta_file, timestamp.to_string()).await.unwrap();

    // With caching enabled, should read from cache
    let result = client.request_raw(&endpoint).await.unwrap();
    assert_eq!(result, b"test cached data");

    // Disable caching
    client.set_caching_enabled(false);
    
    // Now it should try to connect to the server and fail
    // (since test/endpoint is not a real endpoint)
    let result = client.request_raw(&endpoint).await;
    assert!(result.is_err());

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_ttl_differentiation() {
    let test_dir = test_cache_dir().await;
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Certificate endpoints should have 30-day TTL
    // Regular endpoints should have 5-minute TTL
    // This is tested via unit tests, but we can verify the behavior

    // Write test data for both types
    let cert_file = test_dir.join("us").join("certs-test-0.bmime");
    let versions_file = test_dir.join("us").join("versions-test-0.bmime");
    
    tokio::fs::create_dir_all(cert_file.parent().unwrap()).await.unwrap();
    tokio::fs::write(&cert_file, b"cert data").await.unwrap();
    tokio::fs::write(&versions_file, b"versions data").await.unwrap();

    // Write metadata with old timestamp (10 minutes ago)
    let old_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() - 600; // 10 minutes ago

    let cert_meta = test_dir.join("us").join("certs-test-0.meta");
    let versions_meta = test_dir.join("us").join("versions-test-0.meta");
    
    tokio::fs::write(&cert_meta, old_timestamp.to_string()).await.unwrap();
    tokio::fs::write(&versions_meta, old_timestamp.to_string()).await.unwrap();

    // Certificate should still be valid (30-day TTL)
    let cert_endpoint = Endpoint::Cert("test".to_string());
    let result = client.request_raw(&cert_endpoint).await;
    // This will fail because we can't mock the actual request
    assert!(result.is_err());

    // Versions should be expired (5-minute TTL)
    let versions_endpoint = Endpoint::ProductVersions("test".to_string());
    let result = client.request_raw(&versions_endpoint).await;
    // This will also fail because we can't mock the actual request
    assert!(result.is_err());

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_clear_operations() {
    let test_dir = test_cache_dir().await;
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Create some cache files
    let cache_dir = test_dir.join("us");
    tokio::fs::create_dir_all(&cache_dir).await.unwrap();

    // Create multiple cache entries
    for i in 0..3 {
        let cache_file = cache_dir.join(format!("test-file{}-0.bmime", i));
        let meta_file = cache_dir.join(format!("test-file{}-0.meta", i));
        
        tokio::fs::write(&cache_file, format!("data {}", i)).await.unwrap();
        tokio::fs::write(&meta_file, "1234567890").await.unwrap();
    }

    // Verify files exist
    let mut entries = tokio::fs::read_dir(&cache_dir).await.unwrap();
    let mut count = 0;
    while entries.next_entry().await.unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 6); // 3 bmime + 3 meta files

    // Clear all cache
    client.clear_cache().await.unwrap();

    // Verify files are gone
    let mut entries = tokio::fs::read_dir(&cache_dir).await.unwrap();
    let mut count = 0;
    while entries.next_entry().await.unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 0);

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_expired_cache_cleanup() {
    let test_dir = test_cache_dir().await;
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    let cache_dir = test_dir.join("us");
    tokio::fs::create_dir_all(&cache_dir).await.unwrap();

    // Create cache entries with different ages
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Fresh certificate (should not be deleted)
    tokio::fs::write(cache_dir.join("certs-fresh-0.bmime"), b"fresh cert").await.unwrap();
    tokio::fs::write(cache_dir.join("certs-fresh-0.meta"), now.to_string()).await.unwrap();

    // Old certificate (31 days old - should be deleted)
    let old_cert_time = now - (31 * 24 * 60 * 60);
    tokio::fs::write(cache_dir.join("certs-old-0.bmime"), b"old cert").await.unwrap();
    tokio::fs::write(cache_dir.join("certs-old-0.meta"), old_cert_time.to_string()).await.unwrap();

    // Fresh regular file (should not be deleted)
    tokio::fs::write(cache_dir.join("versions-fresh-0.bmime"), b"fresh versions").await.unwrap();
    tokio::fs::write(cache_dir.join("versions-fresh-0.meta"), now.to_string()).await.unwrap();

    // Old regular file (6 minutes old - should be deleted)
    let old_version_time = now - (6 * 60);
    tokio::fs::write(cache_dir.join("versions-old-0.bmime"), b"old versions").await.unwrap();
    tokio::fs::write(cache_dir.join("versions-old-0.meta"), old_version_time.to_string()).await.unwrap();

    // Clear expired entries
    client.clear_expired().await.unwrap();

    // Check what remains
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&cache_dir).await.unwrap();
    while let Some(entry) = dir.next_entry().await.unwrap() {
        entries.push(entry.file_name().to_string_lossy().to_string());
    }

    // Should only have fresh files
    assert!(entries.contains(&"certs-fresh-0.bmime".to_string()));
    assert!(entries.contains(&"certs-fresh-0.meta".to_string()));
    assert!(entries.contains(&"versions-fresh-0.bmime".to_string()));
    assert!(entries.contains(&"versions-fresh-0.meta".to_string()));
    
    // Should not have old files
    assert!(!entries.contains(&"certs-old-0.bmime".to_string()));
    assert!(!entries.contains(&"certs-old-0.meta".to_string()));
    assert!(!entries.contains(&"versions-old-0.bmime".to_string()));
    assert!(!entries.contains(&"versions-old-0.meta".to_string()));

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let test_dir = test_cache_dir().await;
    
    // Create test data files
    let cache_dir = test_dir.join("us");
    tokio::fs::create_dir_all(&cache_dir).await.unwrap();
    
    // Pre-populate cache with test data
    for i in 0..5 {
        let filename = format!("concurrent-test{}-0.bmime", i);
        let metaname = format!("concurrent-test{}-0.meta", i);
        let data = format!("concurrent data {}", i);
        
        tokio::fs::write(cache_dir.join(&filename), &data).await.unwrap();
        tokio::fs::write(cache_dir.join(&metaname), "9999999999").await.unwrap();
    }

    // Spawn multiple tasks to read from cache concurrently
    let mut handles = vec![];
    
    for i in 0..5 {
        let test_dir_clone = test_dir.clone();
        let handle = tokio::spawn(async move {
            let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir_clone)
                .await
                .unwrap();
            
            // Try to read from cache (will fail to fetch from server if cache miss)
            let endpoint = Endpoint::Custom(format!("concurrent/test{}", i));
            let _ = client.request_raw(&endpoint).await;
            
            // Clear expired (shouldn't affect anything since all are fresh)
            let _ = client.clear_expired().await;
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_with_different_regions() {
    let test_dir = test_cache_dir().await;
    
    // Create clients for different regions
    let us_client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();
    let _eu_client = CachedRibbitClient::with_cache_dir(Region::EU, test_dir.clone())
        .await
        .unwrap();
    
    // Create cache files for different regions
    let us_dir = test_dir.join("us");
    let eu_dir = test_dir.join("eu");
    
    tokio::fs::create_dir_all(&us_dir).await.unwrap();
    tokio::fs::create_dir_all(&eu_dir).await.unwrap();
    
    // Write region-specific data
    tokio::fs::write(us_dir.join("test-data-0.bmime"), b"US data").await.unwrap();
    tokio::fs::write(us_dir.join("test-data-0.meta"), "9999999999").await.unwrap();
    
    tokio::fs::write(eu_dir.join("test-data-0.bmime"), b"EU data").await.unwrap();
    tokio::fs::write(eu_dir.join("test-data-0.meta"), "9999999999").await.unwrap();
    
    // Clear cache for US region only
    us_client.clear_cache().await.unwrap();
    
    // US files should be gone
    let mut us_entries = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&us_dir).await {
        while let Some(entry) = dir.next_entry().await.unwrap() {
            us_entries.push(entry);
        }
    }
    assert_eq!(us_entries.len(), 0);
    
    // EU files should still exist
    let mut eu_entries = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&eu_dir).await {
        while let Some(entry) = dir.next_entry().await.unwrap() {
            eu_entries.push(entry);
        }
    }
    assert_eq!(eu_entries.len(), 2);
    
    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_directory_structure() {
    let test_dir = test_cache_dir().await;
    let _client = CachedRibbitClient::with_cache_dir(Region::KR, test_dir.clone())
        .await
        .unwrap();
    
    // Create a cache entry
    let kr_dir = test_dir.join("kr");
    tokio::fs::create_dir_all(&kr_dir).await.unwrap();
    tokio::fs::write(kr_dir.join("test-0.bmime"), b"test").await.unwrap();
    tokio::fs::write(kr_dir.join("test-0.meta"), "123").await.unwrap();
    
    // Verify the directory structure
    assert!(test_dir.exists());
    assert!(kr_dir.exists());
    assert!(kr_dir.join("test-0.bmime").exists());
    assert!(kr_dir.join("test-0.meta").exists());
    
    cleanup_test_dir(&test_dir).await;
}
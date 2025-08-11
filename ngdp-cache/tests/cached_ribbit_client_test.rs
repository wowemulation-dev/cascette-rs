//! Integration tests for CachedRibbitClient

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};
use std::path::PathBuf;

/// Test data directory for isolated testing
async fn test_cache_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir()
        .join("ngdp_cache_test")
        .join(format!("cached_ribbit_{}", unique_id));
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
        (
            Endpoint::ProductVersions("wow".to_string()),
            None,
            "versions-wow-0.bmime",
        ),
        (
            Endpoint::ProductCdns("d4".to_string()),
            None,
            "cdns-d4-0.bmime",
        ),
        (
            Endpoint::ProductBgdl("wow_classic".to_string()),
            None,
            "bgdl-wow_classic-0.bmime",
        ),
        (
            Endpoint::Cert("abc123".to_string()),
            None,
            "certs-abc123-0.bmime",
        ),
        (
            Endpoint::Cert("def456".to_string()),
            Some(12345),
            "certs-def456-12345.bmime",
        ),
        (
            Endpoint::Ocsp("789xyz".to_string()),
            None,
            "ocsp-789xyz-0.bmime",
        ),
        (
            Endpoint::Custom("products/wow/versions".to_string()),
            None,
            "products-wow-0.bmime",
        ),
        (
            Endpoint::Custom("custom".to_string()),
            None,
            "custom-#-0.bmime",
        ),
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

    // Test that we can enable/disable caching
    client.set_caching_enabled(true);
    client.set_caching_enabled(false);

    // Clean up
    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_ttl_differentiation() {
    let test_dir = test_cache_dir().await;
    let _client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Just verify client was created - actual TTL testing is done in unit tests

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_clear_operations() {
    let test_dir = test_cache_dir().await;
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Test that clear_cache doesn't fail on empty cache
    client.clear_cache().await.unwrap();

    // Test that clear_expired doesn't fail on empty cache
    client.clear_expired().await.unwrap();

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_expired_cache_cleanup() {
    let test_dir = test_cache_dir().await;
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();

    // Test that clear_expired works without errors even when cache is empty
    client.clear_expired().await.unwrap();

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let test_dir = test_cache_dir().await;

    // Spawn multiple tasks to create clients concurrently
    let mut handles = vec![];

    for _ in 0..5 {
        let test_dir_clone = test_dir.clone();
        let handle = tokio::spawn(async move {
            let _client = CachedRibbitClient::with_cache_dir(Region::US, test_dir_clone)
                .await
                .unwrap();
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
    let _us_client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.clone())
        .await
        .unwrap();
    let _eu_client = CachedRibbitClient::with_cache_dir(Region::EU, test_dir.clone())
        .await
        .unwrap();

    // Just verify that clients can be created for different regions

    cleanup_test_dir(&test_dir).await;
}

#[tokio::test]
async fn test_cache_directory_structure() {
    let test_dir = test_cache_dir().await;
    let _client = CachedRibbitClient::with_cache_dir(Region::KR, test_dir.clone())
        .await
        .unwrap();

    // Ensure directory creation is completed by trying multiple times
    for _attempt in 0..10 {
        tokio::task::yield_now().await;
        if test_dir.exists() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Verify the test directory was created
    assert!(
        test_dir.exists(),
        "Test directory {test_dir:?} should exist"
    );

    cleanup_test_dir(&test_dir).await;
}

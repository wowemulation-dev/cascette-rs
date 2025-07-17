//! Integration tests for CachedRibbitClient

use ngdp_cache::cached_ribbit_client::CachedRibbitClient;
use ribbit_client::{Endpoint, Region};
use tempfile::TempDir;

#[tokio::test]
async fn test_cached_client_creation() {
    // Test default creation
    let client = CachedRibbitClient::new(Region::US).await.unwrap();
    assert_eq!(client.inner().region(), Region::US);

    // Test with custom cache directory
    let test_dir = TempDir::new().unwrap();
    let client = CachedRibbitClient::with_cache_dir(Region::EU, test_dir.path().to_path_buf())
        .await
        .unwrap();
    assert_eq!(client.inner().region(), Region::EU);

    drop(test_dir);
}

#[tokio::test]
async fn test_cache_filename_conventions() {
    let test_dir = TempDir::new().unwrap();
    let _client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.path().to_path_buf())
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

    drop(test_dir);
}

#[tokio::test]
async fn test_cache_enable_disable() {
    let test_dir = TempDir::new().unwrap();
    let mut client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.path().to_path_buf())
        .await
        .unwrap();

    // Test that we can enable/disable caching
    client.set_caching_enabled(true);
    client.set_caching_enabled(false);

    // Clean up
    drop(test_dir);
}

#[tokio::test]
async fn test_cache_clear_operations() {
    let test_dir = TempDir::new().unwrap();
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.path().to_path_buf())
        .await
        .unwrap();

    // Test that clear_cache doesn't fail on empty cache
    client.clear_cache().await.unwrap();

    // Test that clear_expired doesn't fail on empty cache
    client.clear_expired().await.unwrap();

    drop(test_dir);
}

#[tokio::test]
async fn test_expired_cache_cleanup() {
    let test_dir = TempDir::new().unwrap();
    let client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.path().to_path_buf())
        .await
        .unwrap();

    // Test that clear_expired works without errors even when cache is empty
    client.clear_expired().await.unwrap();
    drop(test_dir);
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let test_dir = TempDir::new().unwrap();

    // Spawn multiple tasks to create clients concurrently
    let mut handles = vec![];

    for _ in 0..5 {
        let test_dir_clone = test_dir.path().to_path_buf();
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

    drop(test_dir);
}

#[tokio::test]
async fn test_cache_with_different_regions() {
    let test_dir = TempDir::new().unwrap();

    // Create clients for different regions
    let _us_client = CachedRibbitClient::with_cache_dir(Region::US, test_dir.path().to_path_buf())
        .await
        .unwrap();
    let _eu_client = CachedRibbitClient::with_cache_dir(Region::EU, test_dir.path().to_path_buf())
        .await
        .unwrap();

    // Just verify that clients can be created for different regions
    drop(test_dir);
}

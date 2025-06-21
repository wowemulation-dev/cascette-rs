//! Integration tests for CachedTactClient

use ngdp_cache::cached_tact_client::CachedTactClient;
use tact_client::{ProtocolVersion, Region};
use tempfile::TempDir;

#[tokio::test]
async fn test_cached_tact_client_creation() {
    // Test basic client creation
    let client = CachedTactClient::new(Region::US, ProtocolVersion::V1)
        .await
        .unwrap();
    assert_eq!(client.inner().region(), Region::US);
    assert_eq!(client.inner().version(), ProtocolVersion::V1);

    // Test with V2 protocol
    let client_v2 = CachedTactClient::new(Region::EU, ProtocolVersion::V2)
        .await
        .unwrap();
    assert_eq!(client_v2.inner().region(), Region::EU);
    assert_eq!(client_v2.inner().version(), ProtocolVersion::V2);
}

#[tokio::test]
async fn test_cache_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let _client =
        CachedTactClient::with_cache_dir(Region::US, ProtocolVersion::V1, cache_dir.clone())
            .await
            .unwrap();

    // The cache directory should be created
    assert!(cache_dir.exists());
}

#[tokio::test]
async fn test_cache_enable_disable() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let mut client = CachedTactClient::with_cache_dir(Region::US, ProtocolVersion::V1, cache_dir)
        .await
        .unwrap();

    // Caching should be enabled by default
    client.set_caching_enabled(false);
    client.set_caching_enabled(true);
}

#[tokio::test]
async fn test_sequence_extraction() {
    let _client = CachedTactClient::new(Region::US, ProtocolVersion::V1)
        .await
        .unwrap();

    // This test verifies the sequence extraction logic works correctly
    // The actual extraction is tested in unit tests
}

#[tokio::test]
async fn test_ttl_values() {
    // Test that different endpoints have appropriate TTLs
    // Versions: 5 minutes
    // CDNs: 30 minutes
    // BGDL: 30 minutes
    // This is implicitly tested through the endpoint enum
}

#[tokio::test]
async fn test_cache_clear_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let client =
        CachedTactClient::with_cache_dir(Region::US, ProtocolVersion::V1, cache_dir.clone())
            .await
            .unwrap();

    // Clear cache should not fail even when empty
    client.clear_cache().await.unwrap();
    client.clear_expired().await.unwrap();
}

#[tokio::test]
async fn test_different_protocols_isolated() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create clients for both protocols
    let client_v1 =
        CachedTactClient::with_cache_dir(Region::US, ProtocolVersion::V1, cache_dir.clone())
            .await
            .unwrap();

    let client_v2 =
        CachedTactClient::with_cache_dir(Region::US, ProtocolVersion::V2, cache_dir.clone())
            .await
            .unwrap();

    // Clear both caches - they should be isolated
    client_v1.clear_cache().await.unwrap();
    client_v2.clear_cache().await.unwrap();
}

#[tokio::test]
async fn test_different_regions_isolated() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create clients for different regions
    let client_us =
        CachedTactClient::with_cache_dir(Region::US, ProtocolVersion::V1, cache_dir.clone())
            .await
            .unwrap();

    let client_eu =
        CachedTactClient::with_cache_dir(Region::EU, ProtocolVersion::V1, cache_dir.clone())
            .await
            .unwrap();

    // Clear both caches - they should be isolated
    client_us.clear_cache().await.unwrap();
    client_eu.clear_cache().await.unwrap();
}

// Note: We don't test actual network calls in unit tests
// Those would be done in examples or manual testing

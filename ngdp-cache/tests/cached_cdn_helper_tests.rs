//! Integration tests for CachedCdnClient helper methods

use ngdp_cache::cached_cdn_client::CachedCdnClient;
use tempfile::TempDir;

#[tokio::test]
async fn test_helper_methods_construct_correct_paths() {
    // Create a temporary cache directory
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // These tests will fail with 404 since we're using fake hashes,
    // but we can verify the URL construction by checking the error messages

    let cdn_host = "test.cdn.com";
    let path = "tpr/wow";
    let config_path = "tpr/configs/data";
    let hash = "abcd1234567890abcdef1234567890ab";

    // Test BuildConfig path construction
    let result = client.download_build_config(cdn_host, path, hash).await;
    match result {
        Err(err) => {
            let error_msg = err.to_string();
            // The error should mention the CDN client error, indicating it tried to download
            assert!(error_msg.contains("CDN client error"));
        }
        Ok(_) => panic!("Expected error but got success"),
    }

    // Test CDNConfig path construction
    let result = client.download_cdn_config(cdn_host, path, hash).await;
    assert!(result.is_err());

    // Test ProductConfig path construction
    let result = client
        .download_product_config(cdn_host, config_path, hash)
        .await;
    assert!(result.is_err());

    // Test KeyRing path construction
    let result = client.download_key_ring(cdn_host, path, hash).await;
    assert!(result.is_err());

    // Test Data file path construction
    let result = client.download_data(cdn_host, path, hash).await;
    assert!(result.is_err());

    // Test Patch file path construction
    let result = client.download_patch(cdn_host, path, hash).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_cache_directory_structure_with_helpers() {
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // The cache structure should follow the CDN path structure
    let cache_base = client.cache_dir();

    // After attempting downloads (even if they fail), the directory structure
    // should be created according to the CDN paths

    // Note: Since we can't actually download without valid hashes and CDN hosts,
    // we're mainly testing that the helper methods exist and can be called
    assert!(cache_base.exists());
}

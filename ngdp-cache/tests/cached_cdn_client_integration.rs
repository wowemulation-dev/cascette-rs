//! Integration tests for CachedCdnClient
//!
//! These tests verify the caching behavior with mock CDN responses.

use bytes::Bytes;
use ngdp_cache::cached_cdn_client::CachedCdnClient;
use ngdp_cache::cdn::CdnCache;
use ngdp_cdn::CdnClientTrait;
use std::io::Cursor;
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Create a mock CDN response
fn mock_cdn_response(content: &[u8]) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_bytes(content)
        .insert_header("content-type", "application/octet-stream")
}

#[tokio::test]
async fn test_cached_cdn_client_basic_caching() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // Test content and hash
    let test_hash = "abcdef1234567890abcdef1234567890";
    let test_content = b"Test CDN content data";

    // Set up mock to track request count
    let mock = Mock::given(method("GET"))
        .and(path_regex(
            r"^/tpr/wow/ab/cd/abcdef1234567890abcdef1234567890$",
        ))
        .respond_with(mock_cdn_response(test_content))
        .expect(1); // Should only be called once due to caching

    mock_server.register(mock).await;

    // Create client with temp cache directory
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // First download - should hit the mock server
    let mock_host = mock_server.uri().replace("http://", "");
    let response1 = client
        .download(&mock_host, "tpr/wow", test_hash, "")
        .await
        .unwrap();

    assert!(!response1.is_from_cache());
    assert_eq!(
        response1.bytes().await.unwrap(),
        Bytes::from(&test_content[..])
    );

    // Second download - should come from cache
    let response2 = client
        .download(&mock_host, "tpr/wow", test_hash, "")
        .await
        .unwrap();

    assert!(response2.is_from_cache());
    assert_eq!(
        response2.bytes().await.unwrap(),
        Bytes::from(&test_content[..])
    );

    // Verify mock was only called once
    mock_server.verify().await;
}

#[tokio::test]
async fn test_cached_cdn_client_content_types() {
    let mock_server = MockServer::start().await;
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Test different content types
    let test_cases = vec![
        (
            "config/data",
            "1234567890abcdef1234567890abcdef",
            &b"config data"[..],
        ),
        (
            "data",
            "abcdef1234567890abcdef1234567890",
            &b"game data"[..],
        ),
        (
            "patch",
            "fedcba0987654321fedcba0987654321",
            &b"patch data"[..],
        ),
    ];

    let mock_host = mock_server.uri().replace("http://", "");
    for (path, hash, content) in test_cases {
        // Set up mock for this content type
        let first_two = &hash[..2];
        let next_two = &hash[2..4];
        let mock_path = format!("^/{path}/{first_two}/{next_two}/{hash}$");

        Mock::given(method("GET"))
            .and(path_regex(&mock_path))
            .respond_with(mock_cdn_response(content))
            .mount(&mock_server)
            .await;

        // Download and verify
        let response = client.download(&mock_host, path, hash, "").await.unwrap();

        assert!(!response.is_from_cache());
        assert_eq!(response.bytes().await.unwrap(), Bytes::from(content));

        // Verify it's cached
        let response2 = client.download(&mock_host, path, hash, "").await.unwrap();

        assert!(response2.is_from_cache());
    }
}

#[tokio::test]
async fn test_cached_cdn_client_disable_caching() {
    let mock_server = MockServer::start().await;
    let temp_dir = TempDir::new().unwrap();
    let mut client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Disable caching
    client.set_caching_enabled(false);

    let test_hash = "d1234567890abcdef1234567890abcde";
    let test_content = b"Test content with caching disabled";

    // Mock should be called twice since caching is disabled
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/data/d1/23/d1234567890abcdef1234567890abcde$",
        ))
        .respond_with(mock_cdn_response(test_content))
        .expect(2)
        .mount(&mock_server)
        .await;

    // Both downloads should hit the server
    let mock_host = mock_server.uri().replace("http://", "");
    let response1 = client
        .download(&mock_host, "data", test_hash, "")
        .await
        .unwrap();

    assert!(!response1.is_from_cache());

    let response2 = client
        .download(&mock_host, "data", test_hash, "")
        .await
        .unwrap();

    assert!(!response2.is_from_cache());

    mock_server.verify().await;
}

#[tokio::test]
async fn test_cached_cdn_client_cache_stats() {
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Manually write some test files to cache
    let cache_dir = client.cache_dir();

    // Create test files
    let test_files = vec![
        ("config/ab/cd/abcd1234", &b"config content"[..]),
        ("data/12/34/12345678", &b"data content here"[..]),
        ("patch/ef/01/ef012345", &b"patch content"[..]),
    ];

    for (path, content) in &test_files {
        let file_path = cache_dir.join(path);
        tokio::fs::create_dir_all(file_path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(file_path, content).await.unwrap();
    }

    // Get cache stats
    let stats = client.cache_stats().await.unwrap();

    assert_eq!(stats.total_files, 3);
    assert_eq!(stats.config_files, 1);
    assert_eq!(stats.data_files, 1);
    assert_eq!(stats.patch_files, 1);

    // Verify human-readable formatting
    assert!(stats.total_size_human().contains("B"));
}

#[tokio::test]
async fn test_cached_cdn_client_clear_cache() {
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Write a test file
    let cache_dir = client.cache_dir();
    let test_file = cache_dir.join("data/te/st/test1234");
    tokio::fs::create_dir_all(test_file.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&test_file, b"test").await.unwrap();

    assert!(test_file.exists());

    // Clear cache
    client.clear_cache().await.unwrap();

    // Verify cache directory is gone
    assert!(!cache_dir.exists());
}

#[tokio::test]
async fn test_cached_cdn_client_streaming() {
    let mock_server = MockServer::start().await;
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let test_hash = "5678901234567890abcdef1234567890";
    let test_content = b"Streaming test content that is larger than usual";

    Mock::given(method("GET"))
        .and(path_regex(
            r"^/data/56/78/5678901234567890abcdef1234567890$",
        ))
        .respond_with(mock_cdn_response(test_content))
        .mount(&mock_server)
        .await;

    // Download with streaming
    let mock_host = mock_server.uri().replace("http://", "");
    let mut stream = client
        .download_stream(&mock_host, "data", test_hash, "")
        .await
        .unwrap();

    // Read from stream
    let mut buffer = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut *stream, &mut buffer)
        .await
        .unwrap();

    assert_eq!(buffer, test_content);

    // Second download should use cached file for streaming
    let mut stream2 = client
        .download_stream(&mock_host, "data", test_hash, "")
        .await
        .unwrap();

    let mut buffer2 = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut *stream2, &mut buffer2)
        .await
        .unwrap();

    assert_eq!(buffer2, test_content);
}

#[tokio::test]
async fn test_cached_cdn_client_size_check() {
    let temp_dir = TempDir::new().unwrap();
    let client = CachedCdnClient::with_cache_dir(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Manually write a test file using the cache structure
    let test_hash = "size1234567890abcdef1234567890ab";
    let test_content = b"Content with known size";

    // Create a CdnCache to write the file in the correct location
    let mut cache = CdnCache::with_base_dir(client.cache_dir().to_path_buf())
        .await
        .unwrap();
    cache.set_cdn_path(Some("data".to_string()));
    cache
        .write_buffer("data", test_hash, "", Cursor::new(test_content))
        .await
        .unwrap();

    // Check cached size
    let size = client.cached_size("data", test_hash, "").await.unwrap();
    assert_eq!(size, Some(test_content.len() as u64));

    // Non-existent file should return None
    let no_size = client
        .cached_size("data", "nonexistent123456", "")
        .await
        .unwrap();
    assert_eq!(no_size, None);
}

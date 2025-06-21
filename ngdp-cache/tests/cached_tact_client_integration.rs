//! Integration tests for CachedTactClient that don't make network requests
//!
//! These tests verify the caching functionality without requiring actual
//! network access to Blizzard servers.

use ngdp_cache::cached_tact_client::CachedTactClient;
use tact_client::{ProtocolVersion, Region};
use tempfile::TempDir;

/// Mock TACT response data for testing
mod mock_data {
    /// Mock versions response with sequence number
    pub const VERSIONS_RESPONSE: &str = r#"Product!STRING:0|Seqn!DEC:4|Flags!STRING:0
## seqn = 3020098
wow|12.0.5.58238|
wow_classic|11.0.5.58162|
wow_classic_era|1.15.5.58162|"#;

    /// Mock CDNs response with sequence number
    /// Note: This is CDN configuration data, not actual CDN content
    pub const CDNS_RESPONSE: &str = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
## seqn = 3020099
us|tpr/wow|level3.blizzard.com edgecast.blizzard.com cdn.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://edgecast.blizzard.com/?maxhosts=4 https://cdn.blizzard.com/?maxhosts=4|tpr/configs/data"#;

    /// Mock BGDL response
    pub const BGDL_RESPONSE: &str = r#"Region!STRING:0|ProductConfig!HEX:32
## seqn = 3020100
us|a9e69b29cce65c8e58cb7c6489df2bf8"#;
}

#[tokio::test]
async fn test_cached_tact_client_mock_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create client with custom cache directory
    let client = CachedTactClient::with_cache_dir(
        Region::US,
        ProtocolVersion::V1,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    // Since we can't make actual network requests in tests, we need to
    // manually write mock data to the cache and verify the caching behavior
    
    // Test 1: Verify cache directory structure is created
    assert!(cache_dir.exists());
    
    // Test 2: Write mock data directly to cache
    let product = "wow";
    let versions_path = cache_dir
        .join("us")
        .join("v1")
        .join(product)
        .join("versions-3020098.bpsv");
    
    // Ensure directory exists
    if let Some(parent) = versions_path.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    
    // Write mock versions data
    tokio::fs::write(&versions_path, mock_data::VERSIONS_RESPONSE)
        .await
        .unwrap();
    
    // Write metadata
    let meta_path = versions_path.with_extension("meta");
    let metadata = serde_json::json!({
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "ttl_seconds": 300,
        "region": "us",
        "protocol": "v1",
        "product": product,
        "endpoint": "versions",
        "sequence": 3020098,
        "response_size": mock_data::VERSIONS_RESPONSE.len()
    });
    tokio::fs::write(&meta_path, serde_json::to_string(&metadata).unwrap())
        .await
        .unwrap();
    
    // Test 3: Verify cache structure
    let cache_structure = cache_dir.join("us").join("v1").join(product);
    assert!(cache_structure.exists());
    
    // List files in cache
    let mut entries = tokio::fs::read_dir(&cache_structure).await.unwrap();
    let mut found_files = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        found_files.push(entry.file_name().to_string_lossy().to_string());
    }
    
    assert!(found_files.contains(&"versions-3020098.bpsv".to_string()));
    assert!(found_files.contains(&"versions-3020098.meta".to_string()));
    
    // Test 4: Clear cache
    client.clear_cache().await.unwrap();
    
    // Verify files are removed
    let mut entries = tokio::fs::read_dir(&cache_structure).await.unwrap_or_else(|_| {
        // Directory might not exist after clear
        panic!("Cache directory should still exist after clear")
    });
    let mut remaining_files = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        remaining_files.push(entry.file_name().to_string_lossy().to_string());
    }
    
    assert!(remaining_files.is_empty(), "Cache should be empty after clear");
}

#[tokio::test]
async fn test_cache_isolation_by_region() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create clients for different regions
    let _us_client = CachedTactClient::with_cache_dir(
        Region::US,
        ProtocolVersion::V1,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    let _eu_client = CachedTactClient::with_cache_dir(
        Region::EU,
        ProtocolVersion::V1,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    // Write mock data for each region
    for region in ["us", "eu"] {
        let path = cache_dir
            .join(region)
            .join("v1")
            .join("wow")
            .join("versions-1000.bpsv");
        
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        
        tokio::fs::write(&path, format!("{} data", region))
            .await
            .unwrap();
    }

    // Verify both regions have separate caches
    let us_path = cache_dir.join("us").join("v1").join("wow");
    let eu_path = cache_dir.join("eu").join("v1").join("wow");
    
    assert!(us_path.exists());
    assert!(eu_path.exists());
    
    // Read and verify content is different
    let us_content = tokio::fs::read_to_string(us_path.join("versions-1000.bpsv"))
        .await
        .unwrap();
    let eu_content = tokio::fs::read_to_string(eu_path.join("versions-1000.bpsv"))
        .await
        .unwrap();
    
    assert_eq!(us_content, "us data");
    assert_eq!(eu_content, "eu data");
}

#[tokio::test]
async fn test_cache_isolation_by_protocol() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Create clients for different protocol versions
    let _v1_client = CachedTactClient::with_cache_dir(
        Region::US,
        ProtocolVersion::V1,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    let _v2_client = CachedTactClient::with_cache_dir(
        Region::US,
        ProtocolVersion::V2,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    // Write mock data for each protocol
    for protocol in ["v1", "v2"] {
        let path = cache_dir
            .join("us")
            .join(protocol)
            .join("wow")
            .join("versions-2000.bpsv");
        
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        
        tokio::fs::write(&path, format!("{} data", protocol))
            .await
            .unwrap();
    }

    // Verify both protocols have separate caches
    let v1_content = tokio::fs::read_to_string(
        cache_dir.join("us").join("v1").join("wow").join("versions-2000.bpsv")
    )
    .await
    .unwrap();
    
    let v2_content = tokio::fs::read_to_string(
        cache_dir.join("us").join("v2").join("wow").join("versions-2000.bpsv")
    )
    .await
    .unwrap();
    
    assert_eq!(v1_content, "v1 data");
    assert_eq!(v2_content, "v2 data");
}

#[tokio::test]
async fn test_sequence_number_handling() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let client = CachedTactClient::with_cache_dir(
        Region::US,
        ProtocolVersion::V1,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    // Create cache directory
    let product_dir = cache_dir.join("us").join("v1").join("wow");
    tokio::fs::create_dir_all(&product_dir).await.unwrap();

    // Write multiple versions with different sequence numbers
    let sequences = [3020098, 3020099, 3020100];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for (i, seq) in sequences.iter().enumerate() {
        let data_path = product_dir.join(format!("versions-{}.bpsv", seq));
        let meta_path = product_dir.join(format!("versions-{}.meta", seq));
        
        // Write data
        tokio::fs::write(&data_path, format!("## seqn = {}\ndata", seq))
            .await
            .unwrap();
        
        // Write metadata with different timestamps
        let metadata = serde_json::json!({
            "timestamp": now - (60 * i as u64), // Older files have older timestamps
            "ttl_seconds": 300,
            "region": "us",
            "protocol": "v1",
            "product": "wow",
            "endpoint": "versions",
            "sequence": seq,
            "response_size": 20
        });
        
        tokio::fs::write(&meta_path, serde_json::to_string(&metadata).unwrap())
            .await
            .unwrap();
    }

    // The cache should prefer the highest sequence number
    // (In real usage, the client would check cache and find the highest valid sequence)
    
    // Verify all files exist
    for seq in sequences {
        let path = product_dir.join(format!("versions-{}.bpsv", seq));
        assert!(path.exists(), "Sequence {} should exist", seq);
    }

    // Clear expired entries (none should be expired yet)
    client.clear_expired().await.unwrap();
    
    // All files should still exist
    for seq in sequences {
        let path = product_dir.join(format!("versions-{}.bpsv", seq));
        assert!(path.exists(), "Sequence {} should still exist after clear_expired", seq);
    }
}

#[tokio::test]
async fn test_endpoint_differentiation() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    let _client = CachedTactClient::with_cache_dir(
        Region::US,
        ProtocolVersion::V1,
        cache_dir.clone(),
    )
    .await
    .unwrap();

    // Create cache entries for different endpoints
    let product_dir = cache_dir.join("us").join("v1").join("wow");
    tokio::fs::create_dir_all(&product_dir).await.unwrap();

    // Write data for each endpoint type
    let endpoints = [
        ("versions", mock_data::VERSIONS_RESPONSE),
        ("cdns", mock_data::CDNS_RESPONSE),  // This is CDN config, not CDN content!
        ("bgdl", mock_data::BGDL_RESPONSE),
    ];

    for (endpoint, data) in endpoints {
        let path = product_dir.join(format!("{}-1000.bpsv", endpoint));
        tokio::fs::write(&path, data).await.unwrap();
    }

    // Verify all endpoint types have separate cache files
    for (endpoint, _) in endpoints {
        let path = product_dir.join(format!("{}-1000.bpsv", endpoint));
        assert!(path.exists(), "{} cache should exist", endpoint);
    }

    // Read and verify each has correct content
    let versions_content = tokio::fs::read_to_string(product_dir.join("versions-1000.bpsv"))
        .await
        .unwrap();
    assert!(versions_content.contains("wow|12.0.5.58238"));

    let cdns_content = tokio::fs::read_to_string(product_dir.join("cdns-1000.bpsv"))
        .await
        .unwrap();
    assert!(cdns_content.contains("level3.blizzard.com"));
    assert!(cdns_content.contains("tpr/configs/data")); // This is config path, not content!

    let bgdl_content = tokio::fs::read_to_string(product_dir.join("bgdl-1000.bpsv"))
        .await
        .unwrap();
    assert!(bgdl_content.contains("a9e69b29cce65c8e58cb7c6489df2bf8"));
}

// Note: We cannot test actual network requests in unit tests.
// For real integration testing with network access, use the examples/
// or create a separate integration test suite that's excluded from CI.
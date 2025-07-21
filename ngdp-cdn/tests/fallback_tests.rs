//! Integration tests for CDN fallback functionality

use ngdp_cdn::{
    CdnClient, CdnClientBuilder, CdnClientBuilderTrait as _, CdnClientTrait as _,
    CdnClientWithFallback, CdnClientWithFallbackBuilder, FallbackCdnClientTrait as _,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

/// Test that primary CDN is tried first
#[tokio::test]
async fn test_primary_cdn_success() {
    let primary_server = MockServer::start().await;
    let backup_server = MockServer::start().await;

    // Set up successful response on primary
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test content"))
        .expect(1)
        .mount(&primary_server)
        .await;

    // Backup should not be called
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&backup_server)
        .await;

    let client = CdnClientWithFallbackBuilder::<CdnClient>::new()
        .add_primary_cdn(primary_server.uri().strip_prefix("http://").unwrap())
        .use_default_backups(false)
        .build()
        .await
        .unwrap();

    client.add_primary_cdn(backup_server.uri().strip_prefix("http://").unwrap());

    let response = client
        .download("tpr/wow", "1234567890abcdef", "")
        .await
        .unwrap();
    let content = response.bytes().await.unwrap();
    assert_eq!(&content[..], b"test content");
}

/// Test fallback to second CDN when primary fails
#[tokio::test]
async fn test_fallback_on_primary_failure() {
    let primary_server = MockServer::start().await;
    let backup_server = MockServer::start().await;

    // Primary returns 500 error
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&primary_server)
        .await;

    // Backup returns success
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"backup content"))
        .mount(&backup_server)
        .await;

    let client = CdnClientWithFallback::<CdnClient>::builder()
        .add_primary_cdn(primary_server.uri().strip_prefix("http://").unwrap())
        .add_primary_cdn(backup_server.uri().strip_prefix("http://").unwrap())
        .use_default_backups(false)
        .configure_base_client(|builder: CdnClientBuilder| {
            builder.max_retries(0) // Disable retries on individual CDN
        })
        .build()
        .await
        .unwrap();

    let response = client
        .download("tpr/wow", "1234567890abcdef", "")
        .await
        .unwrap();
    let content = response.bytes().await.unwrap();
    assert_eq!(&content[..], b"backup content");
}

/// Test that all CDNs are tried in order
#[tokio::test]
async fn test_all_cdns_tried_in_order() {
    let server1 = MockServer::start().await;
    let server2 = MockServer::start().await;
    let server3 = MockServer::start().await;

    // All servers fail except the last one
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server2)
        .await;

    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"success"))
        .expect(1)
        .mount(&server3)
        .await;

    let client = CdnClientWithFallback::<CdnClient>::builder()
        .add_primary_cdn(server1.uri().strip_prefix("http://").unwrap())
        .add_primary_cdn(server2.uri().strip_prefix("http://").unwrap())
        .add_primary_cdn(server3.uri().strip_prefix("http://").unwrap())
        .use_default_backups(false)
        .configure_base_client(|builder: CdnClientBuilder| builder.max_retries(0))
        .build()
        .await
        .unwrap();

    let response = client
        .download("tpr/wow", "1234567890abcdef", "")
        .await
        .unwrap();
    let content = response.bytes().await.unwrap();
    assert_eq!(&content[..], b"success");
}

/// Test that error is returned when all CDNs fail
#[tokio::test]
async fn test_all_cdns_fail() {
    let server1 = MockServer::start().await;
    let server2 = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server2)
        .await;

    let client = CdnClientWithFallback::<CdnClient>::builder()
        .add_primary_cdn(server1.uri().strip_prefix("http://").unwrap())
        .add_primary_cdn(server2.uri().strip_prefix("http://").unwrap())
        .use_default_backups(false)
        .configure_base_client(|builder: CdnClientBuilder| builder.max_retries(0))
        .build()
        .await
        .unwrap();

    let result = client.download("tpr/wow", "1234567890abcdef", "").await;
    assert!(result.is_err());
}

/// Test default backup CDNs
#[tokio::test]
async fn test_default_backup_cdns() {
    let client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();
    let hosts = client.get_all_cdn_hosts();

    assert!(hosts.contains(&"cdn.arctium.tools".to_string()));
    assert!(hosts.contains(&"tact.mirror.reliquaryhq.com".to_string()));
}

/// Test disabling default backup CDNs
#[tokio::test]
async fn test_disable_default_backups() {
    let mut client = CdnClientWithFallback::<CdnClient>::new().await.unwrap();
    client.set_use_default_backups(false);

    let hosts = client.get_all_cdn_hosts();
    assert!(hosts.is_empty());
}

/// Test adding and removing CDNs
#[tokio::test]
async fn test_cdn_management() {
    let client = CdnClientWithFallback::<CdnClient>::builder()
        .use_default_backups(false)
        .build()
        .await
        .unwrap();

    // Add CDNs
    client.add_primary_cdn("cdn1.example.com");
    client.add_primary_cdn("cdn2.example.com");

    let hosts = client.get_all_cdn_hosts();
    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0], "cdn1.example.com");
    assert_eq!(hosts[1], "cdn2.example.com");

    // Clear and set new CDNs
    client.set_primary_cdns(vec!["cdn3.example.com", "cdn4.example.com"]);

    let hosts = client.get_all_cdn_hosts();
    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0], "cdn3.example.com");
    assert_eq!(hosts[1], "cdn4.example.com");

    // Clear all
    client.clear_cdns();
    let hosts = client.get_all_cdn_hosts();
    assert!(hosts.is_empty());
}

/// Test that duplicate CDNs are not added
#[tokio::test]
async fn test_no_duplicate_cdns() {
    let client = CdnClientWithFallback::<CdnClient>::builder()
        .use_default_backups(false)
        .build()
        .await
        .unwrap();

    client.add_primary_cdn("cdn1.example.com");
    client.add_primary_cdn("cdn1.example.com");
    client.add_primary_cdn("cdn2.example.com");

    let hosts = client.get_all_cdn_hosts();
    assert_eq!(hosts.len(), 2);
}

/// Test different types of downloads with fallback
#[tokio::test]
async fn test_different_download_types() {
    let primary_server = MockServer::start().await;
    let backup_server = MockServer::start().await;

    // Primary fails
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&primary_server)
        .await;

    // Backup succeeds for different paths
    Mock::given(method("GET"))
        .and(path("/tpr/wow/config/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"config"))
        .mount(&backup_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tpr/wow/data/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .mount(&backup_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tpr/wow/patch/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"patch"))
        .mount(&backup_server)
        .await;

    let client = CdnClientWithFallback::<CdnClient>::builder()
        .add_primary_cdn(primary_server.uri().strip_prefix("http://").unwrap())
        .add_primary_cdn(backup_server.uri().strip_prefix("http://").unwrap())
        .use_default_backups(false)
        .configure_base_client(|builder: CdnClientBuilder| builder.max_retries(0))
        .build()
        .await
        .unwrap();

    // Test build config
    let response = client
        .download_build_config("tpr/wow", "1234567890abcdef")
        .await
        .unwrap();
    assert_eq!(&response.bytes().await.unwrap()[..], b"config");

    // Test data
    let response = client
        .download_data("tpr/wow", "1234567890abcdef")
        .await
        .unwrap();
    assert_eq!(&response.bytes().await.unwrap()[..], b"data");

    // Test patch
    let response = client
        .download_patch("tpr/wow", "1234567890abcdef")
        .await
        .unwrap();
    assert_eq!(&response.bytes().await.unwrap()[..], b"patch");
}

/// Test streaming download with fallback
#[tokio::test]
async fn test_streaming_download_fallback() {
    let primary_server = MockServer::start().await;
    let backup_server = MockServer::start().await;

    // Primary fails
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&primary_server)
        .await;

    // Backup succeeds
    Mock::given(method("GET"))
        .and(path("/tpr/wow/12/34/1234567890abcdef"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"streamed content"))
        .mount(&backup_server)
        .await;

    let client = CdnClientWithFallback::<CdnClient>::builder()
        .add_primary_cdn(primary_server.uri().strip_prefix("http://").unwrap())
        .add_primary_cdn(backup_server.uri().strip_prefix("http://").unwrap())
        .use_default_backups(false)
        .configure_base_client(|builder| builder.max_retries(0))
        .build()
        .await
        .unwrap();

    todo!();
    // let mut buffer = Vec::new();
    // let bytes_written = client
    //     .download_streaming("tpr/wow", "1234567890abcdef", "", &mut buffer)
    //     .await
    //     .unwrap();

    // assert_eq!(bytes_written, 16);
    // assert_eq!(buffer, b"streamed content");
}

//! Integration tests for HTTP version endpoint.
//!
//! These tests start a real HTTP server and make actual requests.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use axum::http::StatusCode;
use cascette_ribbit::{AppState, ServerConfig};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::NamedTempFile;

/// Create test database file.
fn create_test_db() -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temporary test database file");
    let json = r#"[{
        "id": 1,
        "product": "wow",
        "version": "1.14.2.42597",
        "build": "42597",
        "build_config": "0123456789abcdef0123456789abcdef",
        "cdn_config": "fedcba9876543210fedcba9876543210",
        "keyring": null,
        "product_config": null,
        "build_time": "2024-01-01T00:00:00+00:00",
        "encoding_ekey": "aaaabbbbccccddddeeeeffffaaaaffff",
        "root_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
        "install_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
        "download_ekey": "ddddeeeeffffaaaabbbbccccddddeeee"
    }, {
        "id": 2,
        "product": "wowt",
        "version": "11.0.7.58187",
        "build": "58187",
        "build_config": "1234567890abcdef1234567890abcdef",
        "cdn_config": "edcba9876543210fedcba9876543210f",
        "keyring": null,
        "product_config": null,
        "build_time": "2024-06-01T00:00:00+00:00",
        "encoding_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
        "root_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
        "install_ekey": "ddddeeeeffffaaaabbbbccccddddeeee",
        "download_ekey": "eeeeffffaaaabbbbccccddddeeeeaaaa"
    }]"#;
    file.write_all(json.as_bytes())
        .expect("Failed to write test JSON data to temporary file");
    file
}

/// Start test HTTP server on random port.
async fn start_test_server() -> (SocketAddr, Arc<AppState>) {
    // Install ring crypto provider for reqwest (idempotent)
    let _ = rustls::crypto::ring::default_provider().install_default();

    let db_file = create_test_db();
    let config = ServerConfig {
        http_bind: "127.0.0.1:0"
            .parse()
            .expect("Failed to parse HTTP bind address"),
        tcp_bind: "127.0.0.1:0"
            .parse()
            .expect("Failed to parse TCP bind address"),
        builds: db_file.path().to_path_buf(),
        cdn_hosts: "cdn.test.com".to_string(),
        cdn_path: "test/path".to_string(),
        tls_cert: None,
        tls_key: None,
    };

    let state = Arc::new(AppState::new(&config).expect("Failed to initialize AppState"));
    let app = cascette_ribbit::http::create_router(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind HTTP listener");
    let addr = listener
        .local_addr()
        .expect("Failed to get listener address");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("HTTP server failed to run");
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (addr, state)
}

#[tokio::test]
async fn test_http_versions_endpoint_success() {
    let (addr, _state) = start_test_server().await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/wow/versions"))
        .send()
        .await
        .expect("Failed to send GET request to test server");

    assert_eq!(response.status(), StatusCode::OK);

    // Check Content-Type header
    let content_type = response
        .headers()
        .get("content-type")
        .expect("Response should have content-type header")
        .to_str()
        .expect("Content-Type header should be valid UTF-8");
    assert!(content_type.contains("text/plain"));
    assert!(content_type.contains("charset=utf-8"));

    let body = response
        .text()
        .await
        .expect("Failed to read response body as text");

    // Verify BPSV format
    let lines: Vec<&str> = body.lines().collect();
    assert!(!lines.is_empty(), "Response should not be empty");

    // First line should be header
    let header = lines[0];
    assert!(header.contains("Region!STRING:0"));
    assert!(header.contains("BuildConfig!HEX:16"));
    assert!(header.contains("CDNConfig!HEX:16"));
    assert!(header.contains("KeyRing!HEX:16"));
    assert!(header.contains("BuildId!DEC:4"));
    assert!(header.contains("VersionsName!STRING:0"));

    // Should have 7 data rows (one per region)
    let data_line_count = lines
        .iter()
        .filter(|l| !l.is_empty() && !l.contains("Region!STRING") && !l.contains("seqn"))
        .count();
    assert_eq!(
        data_line_count, 7,
        "Should have 7 regions (us, eu, cn, kr, tw, sg, xx)"
    );

    // Verify regions are present
    assert!(body.contains("us|"));
    assert!(body.contains("eu|"));
    assert!(body.contains("cn|"));
    assert!(body.contains("kr|"));
    assert!(body.contains("tw|"));
    assert!(body.contains("sg|"));
    assert!(body.contains("xx|"));

    // Verify build data
    assert!(body.contains("0123456789abcdef0123456789abcdef")); // build_config
    assert!(body.contains("fedcba9876543210fedcba9876543210")); // cdn_config
    assert!(body.contains("42597")); // build id
    assert!(body.contains("1.14.2.42597")); // version

    // Last line should be sequence number
    let last_line = lines
        .last()
        .expect("Response should have at least one line for sequence number");
    assert!(last_line.starts_with("## seqn = "));
}

#[tokio::test]
async fn test_http_versions_endpoint_not_found() {
    let (addr, _state) = start_test_server().await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/nonexistent/versions"))
        .send()
        .await
        .expect("Failed to send GET request for non-existent product");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_versions_multiple_products() {
    let (addr, _state) = start_test_server().await;

    let client = reqwest::Client::new();

    // Test wow product
    let response1 = client
        .get(format!("http://{addr}/wow/versions"))
        .send()
        .await
        .expect("Failed to query wow product");
    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = response1
        .text()
        .await
        .expect("Failed to read wow response body");
    assert!(body1.contains("1.14.2.42597"));

    // Test wowt product
    let response2 = client
        .get(format!("http://{addr}/wowt/versions"))
        .send()
        .await
        .expect("Failed to query wowt product");
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = response2
        .text()
        .await
        .expect("Failed to read wowt response body");
    assert!(body2.contains("11.0.7.58187"));
}

#[tokio::test]
async fn test_http_cdns_endpoint() {
    let (addr, _state) = start_test_server().await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/wow/cdns"))
        .send()
        .await
        .expect("Failed to query CDNs endpoint");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .text()
        .await
        .expect("Failed to read CDNs response body");

    // Verify BPSV format
    let lines: Vec<&str> = body.lines().collect();
    assert!(!lines.is_empty());

    // First line should be CDN header
    let header = lines[0];
    assert!(header.contains("Name!STRING:0"));
    assert!(header.contains("Path!STRING:0"));
    assert!(header.contains("Hosts!STRING:0"));

    // Should have 5 CDN rows
    let data_line_count = lines
        .iter()
        .filter(|l| !l.is_empty() && !l.contains("Name!STRING") && !l.contains("seqn"))
        .count();
    assert_eq!(data_line_count, 5);

    // Verify CDN configuration
    assert!(body.contains("cdn.test.com"));
    assert!(body.contains("test/path"));
}

#[tokio::test]
async fn test_http_bgdl_endpoint() {
    let (addr, _state) = start_test_server().await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/wow/bgdl"))
        .send()
        .await
        .expect("Failed to query BGDL endpoint");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .text()
        .await
        .expect("Failed to read BGDL response body");

    // BGDL should have same format as versions
    assert!(body.contains("Region!STRING:0"));
    assert!(body.contains("BuildConfig!HEX:16"));

    // Should have 7 regions
    assert!(body.contains("us|"));
    assert!(body.contains("eu|"));
    assert!(body.contains("cn|"));
    assert!(body.contains("kr|"));
    assert!(body.contains("tw|"));
    assert!(body.contains("sg|"));
    assert!(body.contains("xx|"));
}

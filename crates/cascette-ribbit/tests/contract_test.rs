//! Contract tests verifying compatibility with cascette-protocol client.
//!
//! These tests start a real Ribbit server and verify that the cascette-protocol
//! client can successfully query it and parse responses.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use cascette_protocol::{ClientConfig, RibbitTactClient};
use cascette_ribbit::{AppState, ServerConfig};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;

/// Create test database with WoW builds.
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

/// Start test server on random ports.
async fn start_test_server() -> (SocketAddr, SocketAddr, Arc<AppState>) {
    // Install ring crypto provider for reqwest/rustls (idempotent)
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

    // Start HTTP server
    let http_state = state.clone();
    let http_app = cascette_ribbit::http::create_router(http_state);
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind HTTP listener");
    let http_addr = http_listener
        .local_addr()
        .expect("Failed to get HTTP listener address");

    tokio::spawn(async move {
        axum::serve(http_listener, http_app)
            .await
            .expect("HTTP server failed to run");
    });

    // Start TCP server
    let tcp_state = state.clone();
    let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind TCP listener");
    let tcp_addr = tcp_listener
        .local_addr()
        .expect("Failed to get TCP listener address");

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = tcp_listener.accept().await {
            let state = tcp_state.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

                let mut reader = BufReader::new(&mut socket);
                let mut command = String::new();
                if let Ok(Ok(_)) =
                    tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut command))
                        .await
                {
                    let command = command.trim();
                    if let Ok(response) =
                        cascette_ribbit::tcp::handlers::handle_command(command, &state)
                    {
                        let socket = reader.into_inner();
                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;
                        let _ = socket.shutdown().await;
                    }
                }
            });
        }
    });

    // Give servers time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    (http_addr, tcp_addr, state)
}

#[tokio::test]
async fn test_client_can_query_http_versions() {
    let (http_addr, _tcp_addr, _state) = start_test_server().await;

    // Configure client to use our test server (HTTP TACT endpoint)
    let config = ClientConfig {
        tact_http_url: format!("http://{http_addr}"),
        tact_https_url: String::new(), // Disable HTTPS
        ribbit_url: String::new(),     // Disable TCP fallback
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    let client =
        RibbitTactClient::new(config).expect("Failed to create RibbitTactClient with test config");

    // Query versions endpoint (client expects simplified path)
    let result = client.query("wow/versions").await;

    assert!(result.is_ok(), "Client should successfully query server");

    let response = result.expect("Query result should be Ok");

    // Verify response has expected structure
    assert!(
        !response.rows().is_empty(),
        "Response should have data rows"
    );

    // Verify we can parse version information
    let row = response
        .rows()
        .first()
        .expect("Response should have at least one row");
    let version_name = row.get_by_name("VersionsName", response.schema());
    assert!(
        version_name.is_some(),
        "Response should have VersionsName field"
    );

    let build_id = row.get_by_name("BuildId", response.schema());
    assert!(build_id.is_some(), "Response should have BuildId field");
}

#[tokio::test]
async fn test_client_can_query_tcp_versions() {
    let (_http_addr, tcp_addr, _state) = start_test_server().await;

    // Configure client to use only Ribbit TCP
    let config = ClientConfig {
        tact_http_url: String::new(),  // Disable HTTP
        tact_https_url: String::new(), // Disable HTTPS
        ribbit_url: format!("tcp://{tcp_addr}"),
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    let client =
        RibbitTactClient::new(config).expect("Failed to create RibbitTactClient with TCP config");

    // Query versions via TCP (use v2 protocol path)
    let result = client.query("v2/products/wow/versions").await;

    assert!(
        result.is_ok(),
        "Client should successfully query server via TCP"
    );

    let response = result.expect("TCP query result should be Ok");
    assert!(
        !response.rows().is_empty(),
        "Response should have data rows"
    );
}

#[tokio::test]
async fn test_client_can_query_cdns() {
    let (http_addr, _tcp_addr, _state) = start_test_server().await;

    let config = ClientConfig {
        tact_http_url: format!("http://{http_addr}"),
        tact_https_url: String::new(),
        ribbit_url: String::new(),
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    let client =
        RibbitTactClient::new(config).expect("Failed to create RibbitTactClient for CDN test");

    let result = client.query("wow/cdns").await;

    assert!(result.is_ok(), "Client should query CDN endpoint");

    let response = result.expect("CDN query result should be Ok");
    assert!(!response.rows().is_empty(), "CDN response should have data");

    // Verify CDN fields
    let row = response
        .rows()
        .first()
        .expect("CDN response should have at least one row");
    let hosts = row.get_by_name("Hosts", response.schema());
    assert!(hosts.is_some(), "CDN response should have Hosts field");

    let path = row.get_by_name("Path", response.schema());
    assert!(path.is_some(), "CDN response should have Path field");
}

#[tokio::test]
async fn test_client_handles_multiple_products() {
    let (http_addr, _tcp_addr, _state) = start_test_server().await;

    let config = ClientConfig {
        tact_http_url: format!("http://{http_addr}"),
        tact_https_url: String::new(),
        ribbit_url: String::new(),
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    let client = RibbitTactClient::new(config)
        .expect("Failed to create RibbitTactClient for multi-product test");

    // Query WoW Classic
    let wow_result = client.query("wow/versions").await;
    assert!(wow_result.is_ok(), "Should query wow product");

    // Query WoW PTR
    let wowt_result = client.query("wowt/versions").await;
    assert!(wowt_result.is_ok(), "Should query wowt product");

    // Verify they return different data
    let wow_response = wow_result.expect("WoW query result should be Ok");
    let wowt_response = wowt_result.expect("WoWT query result should be Ok");

    // Get version names to verify different products
    let wow_version = wow_response
        .rows()
        .first()
        .expect("WoW response should have at least one row")
        .get_by_name("VersionsName", wow_response.schema())
        .expect("WoW response should have VersionsName field");
    let wowt_version = wowt_response
        .rows()
        .first()
        .expect("WoWT response should have at least one row")
        .get_by_name("VersionsName", wowt_response.schema())
        .expect("WoWT response should have VersionsName field");

    assert_ne!(
        wow_version.as_string(),
        wowt_version.as_string(),
        "Different products should return different versions"
    );
}

#[tokio::test]
async fn test_client_handles_not_found() {
    let (http_addr, _tcp_addr, _state) = start_test_server().await;

    let config = ClientConfig {
        tact_http_url: format!("http://{http_addr}"),
        tact_https_url: String::new(),
        ribbit_url: String::new(),
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    let client = RibbitTactClient::new(config)
        .expect("Failed to create RibbitTactClient for not-found test");

    // Query non-existent product
    let result = client.query("nonexistent/versions").await;

    assert!(result.is_err(), "Should fail for non-existent product");
}

#[tokio::test]
async fn test_client_parses_multi_region_responses() {
    let (http_addr, _tcp_addr, _state) = start_test_server().await;

    let config = ClientConfig {
        tact_http_url: format!("http://{http_addr}"),
        tact_https_url: String::new(),
        ribbit_url: String::new(),
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    let client = RibbitTactClient::new(config)
        .expect("Failed to create RibbitTactClient for multi-region test");

    let result = client.query("wow/versions").await;
    assert!(result.is_ok());

    let response = result.expect("Multi-region query result should be Ok");

    // Verify we have 7 regions (us, eu, cn, kr, tw, sg, xx)
    assert_eq!(
        response.rows().len(),
        7,
        "Response should have 7 regional rows"
    );

    // Verify each region has data
    for row in response.rows() {
        let region = row.get_by_name("Region", response.schema());
        assert!(region.is_some(), "Each row should have Region field");
    }
}

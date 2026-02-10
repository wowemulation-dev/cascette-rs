//! Integration tests for TCP v1 (MIME-wrapped) protocol.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use cascette_ribbit::{AppState, ServerConfig};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

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

/// Start test TCP server on random port.
async fn start_test_server() -> (SocketAddr, Arc<AppState>) {
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
    let state_clone = state.clone();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind TCP listener");
    let addr = listener
        .local_addr()
        .expect("Failed to get listener address");

    tokio::spawn(async move {
        // Manually handle connections since start_server tries to bind again
        while let Ok((mut socket, _)) = listener.accept().await {
            let state = state_clone.clone();
            tokio::spawn(async move {
                // Inline connection handler
                let mut reader = BufReader::new(&mut socket);
                let mut command = String::new();
                if let Ok(Ok(_)) = tokio::time::timeout(
                    tokio::time::Duration::from_secs(10),
                    reader.read_line(&mut command),
                )
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

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (addr, state)
}

/// Send TCP v1 command and get response.
async fn send_tcp_v1_command(addr: SocketAddr, command: &str) -> String {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to test server");

    // Send command with newline
    stream
        .write_all(format!("{command}\n").as_bytes())
        .await
        .expect("Failed to write command to stream");
    stream
        .flush()
        .await
        .expect("Failed to flush stream after writing command");

    // Read response
    let mut reader = BufReader::new(&mut stream);
    let mut response = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => response.push_str(&line),
        }
    }

    response
}

/// Verify MIME format structure.
fn verify_mime_format(response: &str) {
    // Check MIME header
    assert!(
        response.contains("MIME-Version: 1.0"),
        "Missing MIME-Version header"
    );
    assert!(
        response.contains("Content-Type: multipart/alternative"),
        "Missing Content-Type header"
    );
    assert!(
        response.contains("boundary=\"RibbitBoundary\""),
        "Missing boundary definition"
    );

    // Check boundary markers
    assert!(
        response.contains("--RibbitBoundary\r\n"),
        "Missing opening boundary"
    );
    assert!(
        response.contains("--RibbitBoundary--"),
        "Missing closing boundary"
    );

    // Check content part headers
    assert!(
        response.contains("Content-Type: text/plain"),
        "Missing content part Content-Type"
    );
    assert!(
        response.contains("Content-Disposition: data"),
        "Missing Content-Disposition"
    );
}

/// Verify SHA-256 checksum.
fn verify_checksum(response: &str) {
    // Find checksum line
    let checksum_line = response
        .lines()
        .find(|l| l.starts_with("Checksum: "))
        .expect("Missing Checksum line");

    let expected_checksum = checksum_line
        .strip_prefix("Checksum: ")
        .expect("Checksum line should have 'Checksum: ' prefix")
        .trim();

    // Calculate checksum of everything before "Checksum:"
    let content_before_checksum = response
        .find("Checksum:")
        .map(|pos| &response[..pos])
        .expect("Could not find Checksum: in response");

    let mut hasher = Sha256::new();
    hasher.update(content_before_checksum.as_bytes());
    let calculated_checksum = format!("{:x}", hasher.finalize());

    assert_eq!(
        expected_checksum, calculated_checksum,
        "Checksum mismatch: expected {expected_checksum}, got {calculated_checksum}",
    );
}

#[tokio::test]
async fn test_tcp_v1_versions_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/wow/versions").await;

    assert!(!response.is_empty());

    // Verify MIME format
    verify_mime_format(&response);

    // Verify checksum
    verify_checksum(&response);

    // Verify BPSV content is present
    assert!(response.contains("Region!STRING:0"));
    assert!(response.contains("BuildConfig!HEX:16"));
    assert!(response.contains("us|"));
    assert!(response.contains("eu|"));
    assert!(response.contains("42597"));
}

#[tokio::test]
async fn test_tcp_v1_cdns_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/wow/cdns").await;

    assert!(!response.is_empty());

    // Verify MIME format
    verify_mime_format(&response);

    // Verify checksum
    verify_checksum(&response);

    // Verify CDN content
    assert!(response.contains("Name!STRING:0"));
    assert!(response.contains("cdn.test.com"));
    assert!(response.contains("test/path"));
}

#[tokio::test]
async fn test_tcp_v1_bgdl_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/wow/bgdl").await;

    assert!(!response.is_empty());

    // Verify MIME format
    verify_mime_format(&response);

    // Verify checksum
    verify_checksum(&response);

    // Verify BGDL content
    assert!(response.contains("Region!STRING:0"));
    assert!(response.contains("us|"));
}

#[tokio::test]
async fn test_tcp_v1_summary_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/summary").await;

    assert!(!response.is_empty());

    // Verify MIME format
    verify_mime_format(&response);

    // Verify checksum
    verify_checksum(&response);

    // Verify summary content
    assert!(response.contains("Product!STRING:0"));
    assert!(response.contains("Seqn!DEC:4"));
    assert!(response.contains("wow|"));
    assert!(response.contains("wowt|"));
}

#[tokio::test]
async fn test_tcp_v1_invalid_product() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/nonexistent/versions").await;

    // Should receive error or empty response
    assert!(response.is_empty() || response.contains("not found") || response.contains("error"));
}

#[tokio::test]
async fn test_tcp_v1_invalid_command_format() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "invalid_command").await;

    // Should receive error or empty response
    assert!(response.is_empty() || response.contains("Invalid") || response.contains("error"));
}

#[tokio::test]
async fn test_tcp_v1_mime_boundary_integrity() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/wow/versions").await;

    // Count boundary occurrences
    let opening_boundaries = response.matches("--RibbitBoundary\r\n").count();
    let closing_boundaries = response.matches("--RibbitBoundary--").count();

    // Should have exactly one opening and one closing boundary
    assert_eq!(
        opening_boundaries, 1,
        "Should have exactly one opening boundary"
    );
    assert_eq!(
        closing_boundaries, 1,
        "Should have exactly one closing boundary"
    );

    // Verify boundary order
    let opening_pos = response
        .find("--RibbitBoundary\r\n")
        .expect("Should have opening boundary");
    let closing_pos = response
        .find("--RibbitBoundary--")
        .expect("Should have closing boundary");
    assert!(
        opening_pos < closing_pos,
        "Opening boundary should come before closing boundary"
    );
}

#[tokio::test]
async fn test_tcp_v1_checksum_position() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/wow/versions").await;

    // Checksum should be after closing boundary
    let closing_boundary_pos = response
        .find("--RibbitBoundary--")
        .expect("Should have closing boundary");
    let checksum_pos = response
        .find("Checksum:")
        .expect("Should have Checksum line");

    assert!(
        checksum_pos > closing_boundary_pos,
        "Checksum should appear after closing boundary"
    );

    // Checksum should be the last meaningful content
    let lines: Vec<&str> = response.lines().collect();
    let checksum_line_index = lines
        .iter()
        .position(|l| l.starts_with("Checksum:"))
        .expect("Should find Checksum line");

    // After checksum, there should only be empty lines or nothing
    for line in &lines[checksum_line_index + 1..] {
        assert!(
            line.trim().is_empty(),
            "No content should appear after checksum line"
        );
    }
}

#[tokio::test]
async fn test_tcp_v1_crlf_line_endings() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v1_command(addr, "v1/products/wow/versions").await;

    // MIME format requires CRLF line endings
    assert!(
        response.contains("\r\n"),
        "Response should use CRLF line endings"
    );

    // Check specific MIME lines
    assert!(
        response.contains("MIME-Version: 1.0\r\n"),
        "MIME-Version should use CRLF"
    );
    assert!(
        response.contains("--RibbitBoundary\r\n"),
        "Boundary should use CRLF"
    );
}

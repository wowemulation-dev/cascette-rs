//! Integration tests for TCP v2 (raw BPSV) protocol.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use cascette_ribbit::{AppState, ServerConfig};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
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

/// Send TCP v2 command and get response.
async fn send_tcp_v2_command(addr: SocketAddr, command: &str) -> String {
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

#[tokio::test]
async fn test_tcp_v2_versions_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v2_command(addr, "v2/products/wow/versions").await;

    // Verify BPSV format
    assert!(!response.is_empty());

    let lines: Vec<&str> = response.lines().collect();

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

    // Verify regions
    assert!(response.contains("us|"));
    assert!(response.contains("eu|"));
    assert!(response.contains("cn|"));
    assert!(response.contains("kr|"));
    assert!(response.contains("tw|"));
    assert!(response.contains("sg|"));
    assert!(response.contains("xx|"));

    // Verify build data
    assert!(response.contains("0123456789abcdef0123456789abcdef"));
    assert!(response.contains("fedcba9876543210fedcba9876543210"));
    assert!(response.contains("42597"));
    assert!(response.contains("1.14.2.42597"));

    // Last line should be sequence number
    let last_line = lines
        .last()
        .expect("Response should have at least one line for sequence number");
    assert!(last_line.starts_with("## seqn = "));
}

#[tokio::test]
async fn test_tcp_v2_cdns_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v2_command(addr, "v2/products/wow/cdns").await;

    assert!(!response.is_empty());

    let lines: Vec<&str> = response.lines().collect();

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
    assert!(response.contains("cdn.test.com"));
    assert!(response.contains("test/path"));
}

#[tokio::test]
async fn test_tcp_v2_bgdl_command() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v2_command(addr, "v2/products/wow/bgdl").await;

    assert!(!response.is_empty());

    // BGDL should have same format as versions
    assert!(response.contains("Region!STRING:0"));
    assert!(response.contains("BuildConfig!HEX:16"));

    // Should have 7 regions
    assert!(response.contains("us|"));
    assert!(response.contains("eu|"));
    assert!(response.contains("cn|"));
    assert!(response.contains("kr|"));
    assert!(response.contains("tw|"));
    assert!(response.contains("sg|"));
    assert!(response.contains("xx|"));
}

#[tokio::test]
async fn test_tcp_v2_invalid_product() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v2_command(addr, "v2/products/nonexistent/versions").await;

    // Should receive error response or empty
    assert!(response.is_empty() || response.contains("not found") || response.contains("error"));
}

#[tokio::test]
async fn test_tcp_v2_invalid_endpoint() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v2_command(addr, "v2/products/wow/invalid").await;

    // Should receive error response or empty
    assert!(response.is_empty() || response.contains("Unknown") || response.contains("error"));
}

#[tokio::test]
async fn test_tcp_v2_invalid_command_format() {
    let (addr, _state) = start_test_server().await;

    let response = send_tcp_v2_command(addr, "invalid_command").await;

    // Should receive error response or empty
    assert!(response.is_empty() || response.contains("Invalid") || response.contains("error"));
}

#[tokio::test]
async fn test_tcp_v2_connection_closes_after_response() {
    let (addr, _state) = start_test_server().await;

    let mut stream = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to test server");

    // Send command
    stream
        .write_all(b"v2/products/wow/versions\n")
        .await
        .expect("Failed to write command");
    stream.flush().await.expect("Failed to flush stream");

    // Read response
    let mut reader = BufReader::new(&mut stream);
    let mut response = String::new();
    reader
        .read_to_string(&mut response)
        .await
        .expect("Failed to read response");

    assert!(!response.is_empty());

    // Connection should be closed now - try to send another command
    let result = stream.write_all(b"v2/products/wow/cdns\n").await;

    // Write might succeed, but read should fail because connection is closed
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    let read_result = reader.read_line(&mut line).await;

    // Either write failed or read returned 0 (EOF)
    assert!(
        result.is_err()
            || read_result.is_ok()
                && read_result.expect("Read should succeed or fail cleanly") == 0
    );
}

//! Ribbit TCP protocol implementation

use crate::error::{ProtocolError, Result};
use crate::mime_parser::{is_v1_mime_response, parse_v1_mime_to_bpsv};
use cascette_formats::CascFormat;
use cascette_formats::bpsv::BpsvDocument;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, trace};

/// Ribbit TCP client
#[derive(Debug)]
pub struct RibbitClient {
    url: String,
    host: String,
    port: u16,
    connect_timeout: Duration,
}

impl RibbitClient {
    /// Create a new Ribbit TCP client
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        // Parse the URL to extract host and port
        // Expected format: tcp://host:port or just host:port
        let url_without_protocol = url.strip_prefix("tcp://").unwrap_or(&url);

        // Parse host and port
        let (host, port) = if let Some(colon_pos) = url_without_protocol.rfind(':') {
            let host = url_without_protocol[..colon_pos].to_string();
            let port_str = &url_without_protocol[colon_pos + 1..];
            let port = port_str
                .parse::<u16>()
                .map_err(|_| ProtocolError::Parse(format!("Invalid port in URL: {url}")))?;
            (host, port)
        } else {
            // Default port for Ribbit TCP
            (url_without_protocol.to_string(), 1119)
        };

        Ok(Self {
            url: url.clone(),
            host,
            port,
            connect_timeout: Duration::from_secs(10),
        })
    }

    /// Query Ribbit endpoint
    pub async fn query(&self, endpoint: &str) -> Result<BpsvDocument> {
        // Ribbit TCP now returns V1 MIME format, use query_v1_mime
        self.query_v1_mime(endpoint).await
    }

    /// Query Ribbit endpoint with V1 MIME support
    pub async fn query_v1_mime(&self, endpoint: &str) -> Result<BpsvDocument> {
        let raw_response = self.query_raw(endpoint).await?;

        // Detect if this is a V1 MIME response
        if is_v1_mime_response(&raw_response) {
            debug!("Detected V1 MIME response, parsing with signature verification");
            // Use the new MIME parser for V1 responses
            parse_v1_mime_to_bpsv(&raw_response)
        } else {
            debug!("Detected V2 text response, parsing directly as BPSV");
            // Parse V2 response directly as BPSV
            BpsvDocument::parse(&raw_response)
                .map_err(|e| ProtocolError::Parse(format!("BPSV parse error: {e}")))
        }
    }

    /// Query Ribbit endpoint and return raw response bytes
    pub async fn query_raw(&self, endpoint: &str) -> Result<Vec<u8>> {
        // Ribbit TCP accepts the full v1/products/ path
        let command = format!("{endpoint}\r\n");

        // Connect to the single configured host
        let addr = format!("{}:{}", self.host, self.port);
        self.query_host_raw(&addr, &command).await.map_err(|e| {
            tracing::warn!("Failed to query {}: {}", self.url, e);
            e
        })
    }

    /// Query TCP-only endpoints (like /certs/{hash} and /ocsp/{hash})
    pub async fn query_tcp_only(&self, endpoint: &str) -> Result<String> {
        let raw_response = self.query_raw(endpoint).await?;

        // Convert to string - these endpoints typically return text responses
        String::from_utf8(raw_response)
            .map_err(|e| ProtocolError::Parse(format!("Invalid UTF-8 response: {e}")))
    }

    async fn query_host_raw(&self, host: &str, command: &str) -> Result<Vec<u8>> {
        trace!("Connecting to Ribbit host: {}", host);

        // Strip tcp:// prefix if present for TcpStream::connect
        let connect_addr = host.strip_prefix("tcp://").unwrap_or(host);

        // Connect with timeout
        let stream = tokio::time::timeout(self.connect_timeout, TcpStream::connect(connect_addr))
            .await
            .map_err(|_| ProtocolError::Timeout)??;

        let mut stream = stream;

        // Send command
        trace!("Sending command: {}", command.trim());
        stream.write_all(command.as_bytes()).await?;

        // Shutdown write side to signal we're done sending
        stream.shutdown().await?;

        // Read response with a timeout
        let read_timeout = Duration::from_secs(30);
        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 8192]; // Larger buffer for MIME responses

        let read_result = tokio::time::timeout(read_timeout, async {
            loop {
                match stream.read(&mut temp_buf).await {
                    Ok(0) => break, // Connection closed
                    Ok(n) => {
                        buffer.extend_from_slice(&temp_buf[..n]);

                        // For V2 responses, check for double newline terminator
                        // For V1 MIME responses, we need to read until connection closes
                        // or we detect the complete MIME structure
                        if buffer.ends_with(b"\n\n") {
                            // Check if this might be a V1 MIME response that's not complete
                            if !is_v1_mime_response(&buffer) {
                                break;
                            }
                        }

                        // Safety limit - V1 responses can be larger due to signatures
                        if buffer.len() > 50 * 1024 * 1024 {
                            // 50MB max for V1 MIME responses
                            return Err(ProtocolError::Parse("Response too large".to_string()));
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Ok(())
        })
        .await;

        match read_result {
            Ok(Ok(())) => {
                trace!("Received response: {} bytes", buffer.len());
                Ok(buffer)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ProtocolError::Timeout),
        }
    }

    #[allow(dead_code)]
    async fn query_host(&self, host: &str, command: &str) -> Result<BpsvDocument> {
        // Connect with timeout
        let stream = tokio::time::timeout(self.connect_timeout, TcpStream::connect(host))
            .await
            .map_err(|_| ProtocolError::Timeout)??;

        let mut stream = stream;

        // Send command
        stream.write_all(command.as_bytes()).await?;

        // Read response
        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 4096];

        loop {
            let n = stream.read(&mut temp_buf).await?;
            if n == 0 {
                break;
            }

            buffer.extend_from_slice(&temp_buf[..n]);

            // Check for end of response (double newline)
            if buffer.ends_with(b"\n\n") {
                break;
            }

            // Safety limit
            if buffer.len() > 10 * 1024 * 1024 {
                // 10MB max
                return Err(ProtocolError::Parse("Response too large".to_string()));
            }
        }

        // Parse BPSV response
        <BpsvDocument as CascFormat>::parse(&buffer)
            .map_err(|e| ProtocolError::Parse(format!("BPSV parse error: {e}")))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::error::ProtocolError;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    fn create_valid_bpsv() -> &'static str {
        "##seqn!DEC:4|region!STRING:0|buildconfig!HEX:16|cdnconfig!HEX:16|keyring!HEX:16|buildid!DEC:4|versionsname!STRING:0|productconfig!HEX:16\n1|us|abcd1234abcd1234|cdef5678cdef5678|def90123def90123|12345|1.0.0|fedcba09fedcba09\n\n"
    }

    // Mock TCP server that responds with BPSV data
    struct MockRibbitServer {
        listener: TcpListener,
        addr: SocketAddr,
        response_data: Arc<Mutex<Vec<u8>>>,
        should_fail: Arc<Mutex<bool>>,
    }

    #[allow(dead_code)]
    impl MockRibbitServer {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("Operation should succeed");
            let addr = listener.local_addr().expect("Operation should succeed");

            Self {
                listener,
                addr,
                response_data: Arc::new(Mutex::new(create_valid_bpsv().as_bytes().to_vec())),
                should_fail: Arc::new(Mutex::new(false)),
            }
        }

        async fn set_response(&self, data: &[u8]) {
            *self.response_data.lock().await = data.to_vec();
        }

        async fn set_should_fail(&self, fail: bool) {
            *self.should_fail.lock().await = fail;
        }

        fn addr(&self) -> SocketAddr {
            self.addr
        }

        fn host_string(&self) -> String {
            format!("127.0.0.1:{}", self.addr.port())
        }

        async fn run_once(&self) {
            if let Ok((mut stream, _)) = self.listener.accept().await {
                if *self.should_fail.lock().await {
                    // Close connection immediately
                    return;
                }

                // Read the command
                let mut buffer = [0; 1024];
                if (stream.read(&mut buffer).await).is_ok() {
                    // Send response
                    let response = self.response_data.lock().await;
                    let _ = stream.write_all(&response).await;
                    let _ = stream.shutdown().await;
                }
            }
        }

        async fn run(&self) {
            loop {
                self.run_once().await;
            }
        }
    }

    #[tokio::test]
    async fn test_ribbit_client_creation() {
        let url = "tcp://host1:1119".to_string();
        let client = RibbitClient::new(url);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_ribbit_client_invalid_url() {
        let url = "not-a-valid-url".to_string();
        let client = RibbitClient::new(url);
        // Should parse but with default port
        assert!(client.is_ok());
        let client = client.expect("Operation should succeed");
        assert_eq!(client.port, 1119); // Default port
    }

    #[tokio::test]
    async fn test_successful_query() {
        let server = MockRibbitServer::start().await;
        let host = server.host_string();

        // Start server in background
        let server_handle = tokio::spawn(async move {
            server.run_once().await;
        });

        let client = RibbitClient::new(format!("tcp://{host}")).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        server_handle.abort();

        assert!(result.is_ok());
        let doc = result.expect("Operation should succeed");
        assert!(!doc.rows().is_empty());
    }

    #[tokio::test]
    async fn test_connection_failure() {
        // Try to connect to a non-existent host
        let client =
            RibbitClient::new("tcp://127.0.0.1:0".to_string()).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_bpsv_response() {
        let server = MockRibbitServer::start().await;
        server.set_response(b"invalid bpsv data\n\n").await;
        let host = server.host_string();

        let server_handle = tokio::spawn(async move {
            server.run_once().await;
        });

        let client = RibbitClient::new(format!("tcp://{host}")).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        server_handle.abort();

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::Parse(_)
        ));
    }

    #[tokio::test]
    async fn test_single_host_connection() {
        let server = MockRibbitServer::start().await;
        let host = server.host_string();

        let server_handle = tokio::spawn(async move {
            server.run_once().await;
        });

        // Test with single working host
        let client = RibbitClient::new(format!("tcp://{host}")).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        server_handle.abort();

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_command_format() {
        let server = MockRibbitServer::start().await;
        let host = server.host_string();

        // We'll capture what command was sent by examining the server's behavior
        let server_handle = tokio::spawn(async move {
            if let Ok((mut stream, _)) = server.listener.accept().await {
                let mut buffer = [0; 1024];
                if let Ok(n) = stream.read(&mut buffer).await {
                    let command = String::from_utf8_lossy(&buffer[..n]);
                    // Should end with \r\n
                    assert!(command.ends_with("\r\n"));
                    assert!(command.starts_with("v1/products/wow/versions"));
                }
                let response = create_valid_bpsv().as_bytes();
                let _ = stream.write_all(response).await;
            }
        });

        let client = RibbitClient::new(format!("tcp://{host}")).expect("Operation should succeed");
        let _result = client.query("v1/products/wow/versions").await;

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_response_size_limit() {
        let server = MockRibbitServer::start().await;
        // Create a response that exceeds the 10MB limit
        let large_response = vec![b'x'; 11 * 1024 * 1024]; // 11MB
        server.set_response(&large_response).await;
        let host = server.host_string();

        let server_handle = tokio::spawn(async move {
            server.run_once().await;
        });

        let client = RibbitClient::new(format!("tcp://{host}")).expect("Operation should succeed");
        let result = client.query("v1/products/wow/versions").await;

        server_handle.abort();

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::Parse(_)
        ));
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let client =
            RibbitClient::new("192.0.2.1:1119".to_string()).expect("Operation should succeed"); // Non-routable address
        let result = client.query("v1/products/wow/versions").await;

        assert!(result.is_err());
        // Should be timeout error since we can't connect
        // The error could be either Timeout or Io depending on network config
    }

    #[test]
    fn test_connect_timeout_configuration() {
        let client = RibbitClient::new("host:1119".to_string()).expect("Operation should succeed");
        assert_eq!(client.connect_timeout, Duration::from_secs(10));
    }
}

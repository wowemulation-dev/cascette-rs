//! TCP command parsing and routing.

use crate::error::ProtocolError;
use crate::server::AppState;
use crate::tcp::{v1, v2};

/// Parse and handle a TCP command.
///
/// Routes to appropriate protocol handler based on command prefix:
/// - `v1/...` -> TCP Ribbit v1 (MIME-wrapped)
/// - `v2/...` -> TCP Ribbit v2 (raw BPSV)
///
/// # Errors
///
/// Returns `ProtocolError` if the command is invalid or processing fails.
pub fn handle_command(command: &str, state: &AppState) -> Result<String, ProtocolError> {
    if command.starts_with("v1/") {
        v1::handle_v1_command(command, state)
    } else if command.starts_with("v2/") {
        v2::handle_v2_command(command, state)
    } else {
        Err(ProtocolError::InvalidCommand(format!(
            "Unknown protocol version: {command}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    fn create_test_state() -> Arc<AppState> {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"[{\"id\":1,\"product\":\"test_product\",\"version\":\"1.0.0\",\"build\":\"1\",\"build_config\":\"0123456789abcdef0123456789abcdef\",\"cdn_config\":\"fedcba9876543210fedcba9876543210\",\"product_config\":null,\"build_time\":\"2024-01-01T00:00:00+00:00\",\"encoding_ekey\":\"aaaabbbbccccddddeeeeffffaaaaffff\",\"root_ekey\":\"bbbbccccddddeeeeffffaaaabbbbcccc\",\"install_ekey\":\"ccccddddeeeeffffaaaabbbbccccdddd\",\"download_ekey\":\"ddddeeeeffffaaaabbbbccccddddeeee\"}]").unwrap();

        let config = ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: file.path().to_path_buf(),
            cdn_hosts: "cdn.test.com".to_string(),
            cdn_path: "test/path".to_string(),
            tls_cert: None,
            tls_key: None,
        };

        Arc::new(AppState::new(&config).unwrap())
    }

    #[tokio::test]
    async fn test_invalid_command() {
        let state = create_test_state();
        let result = handle_command("invalid", &state);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_v2_versions_command() {
        let state = create_test_state();
        let result = handle_command("v2/products/test_product/versions", &state);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("Region!STRING"));
    }
}

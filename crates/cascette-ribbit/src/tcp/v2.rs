//! TCP Ribbit v2 protocol (raw BPSV responses).

use crate::config::CdnConfig;
use crate::error::ProtocolError;
use crate::responses::BpsvResponse;
use crate::server::AppState;

/// Handle TCP Ribbit v2 command.
///
/// v2 commands return raw BPSV responses (no MIME wrapping).
///
/// Supported commands:
/// - `v2/products/{product}/versions`
/// - `v2/products/{product}/cdns`
/// - `v2/products/{product}/bgdl`
///
/// # Errors
///
/// Returns `ProtocolError` if the command is invalid or processing fails.
pub fn handle_v2_command(command: &str, state: &AppState) -> Result<String, ProtocolError> {
    // Parse command format: v2/products/{product}/{endpoint}
    let parts: Vec<&str> = command.split('/').collect();

    if parts.len() != 4 || parts[0] != "v2" || parts[1] != "products" {
        return Err(ProtocolError::InvalidCommand(format!(
            "Invalid v2 command format: {command}"
        )));
    }

    let product = parts[2];
    let endpoint = parts[3];

    // Get build for product
    let build = state
        .database()
        .latest_build(product)
        .ok_or_else(|| ProtocolError::InvalidCommand(format!("Product not found: {product}")))?;

    let seqn = state.current_seqn();

    // Generate appropriate BPSV response
    let response = match endpoint {
        "versions" => BpsvResponse::versions(build, seqn),
        "cdns" => {
            let cdn_config = CdnConfig::resolve_for_build(build, state.cdn_config());
            BpsvResponse::cdns(&cdn_config, seqn)
        }
        "bgdl" => BpsvResponse::bgdl(build, seqn),
        _ => {
            return Err(ProtocolError::InvalidCommand(format!(
                "Unknown v2 endpoint: {endpoint}"
            )));
        }
    };

    Ok(response.to_string())
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
    async fn test_v2_versions() {
        let state = create_test_state();
        let result = handle_v2_command("v2/products/test_product/versions", &state);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("Region!STRING"));
        assert!(response.contains("BuildConfig!HEX"));
    }

    #[tokio::test]
    async fn test_v2_cdns() {
        let state = create_test_state();
        let result = handle_v2_command("v2/products/test_product/cdns", &state);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("Name!STRING"));
        assert!(response.contains("Hosts!STRING"));
    }

    #[tokio::test]
    async fn test_v2_bgdl() {
        let state = create_test_state();
        let result = handle_v2_command("v2/products/test_product/bgdl", &state);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_v2_invalid_format() {
        let state = create_test_state();
        let result = handle_v2_command("v2/invalid", &state);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_v2_product_not_found() {
        let state = create_test_state();
        let result = handle_v2_command("v2/products/nonexistent/versions", &state);
        assert!(result.is_err());
    }
}

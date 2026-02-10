//! TCP Ribbit v1 protocol (MIME-wrapped with SHA-256 checksums).

use crate::config::CdnConfig;
use crate::error::ProtocolError;
use crate::responses::BpsvResponse;
use crate::server::AppState;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Handle TCP Ribbit v1 command.
///
/// v1 commands return MIME-wrapped BPSV responses with SHA-256 checksums.
///
/// Supported commands:
/// - `v1/summary` (TCP v1 only - list all products)
/// - `v1/products/{product}/versions`
/// - `v1/products/{product}/cdns`
/// - `v1/products/{product}/bgdl`
///
/// # Errors
///
/// Returns `ProtocolError` if the command is invalid or processing fails.
pub fn handle_v1_command(command: &str, state: &AppState) -> Result<String, ProtocolError> {
    // Special case: v1/summary endpoint
    if command == "v1/summary" {
        return Ok(handle_summary(state));
    }

    // Parse command format: v1/products/{product}/{endpoint}
    let parts: Vec<&str> = command.split('/').collect();

    if parts.len() != 4 || parts[0] != "v1" || parts[1] != "products" {
        return Err(ProtocolError::InvalidCommand(format!(
            "Invalid v1 command format: {command}"
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
    let bpsv = match endpoint {
        "versions" => BpsvResponse::versions(build, seqn),
        "cdns" => {
            let cdn_config = CdnConfig::resolve_for_build(build, state.cdn_config());
            BpsvResponse::cdns(&cdn_config, seqn)
        }
        "bgdl" => BpsvResponse::bgdl(build, seqn),
        _ => {
            return Err(ProtocolError::InvalidCommand(format!(
                "Unknown v1 endpoint: {endpoint}"
            )));
        }
    };

    // Wrap in MIME with checksum
    Ok(wrap_in_mime(&bpsv.to_string()))
}

/// Handle v1/summary command (TCP v1 only).
///
/// Returns list of all available products.
fn handle_summary(state: &AppState) -> String {
    let products = state.database().products();
    let seqn = state.current_seqn();

    let bpsv = BpsvResponse::summary(&products, seqn);

    // Wrap in MIME with checksum
    wrap_in_mime(&bpsv.to_string())
}

/// Wrap BPSV content in MIME multipart/alternative with SHA-256 checksum.
///
/// Format:
/// ```text
/// MIME-Version: 1.0\r\n
/// Content-Type: multipart/alternative; boundary="RibbitBoundary"\r\n
/// \r\n
/// --RibbitBoundary\r\n
/// Content-Type: text/plain\r\n
/// Content-Disposition: data\r\n
/// \r\n
/// [BPSV content with LF line endings]
/// \r\n
/// --RibbitBoundary--\r\n
/// Checksum: [64-character SHA-256 hex]\r\n
/// ```
fn wrap_in_mime(bpsv_content: &str) -> String {
    let mime_parts = [
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/alternative; boundary=\"RibbitBoundary\"\r\n",
        "\r\n",
        "--RibbitBoundary\r\n",
        "Content-Type: text/plain\r\n",
        "Content-Disposition: data\r\n",
        "\r\n",
        bpsv_content,
        "\r\n",
        "--RibbitBoundary--\r\n",
    ];

    // Calculate SHA-256 checksum of everything before "Checksum:" line
    let content_before_checksum = mime_parts.join("");
    let mut hasher = Sha256::new();
    hasher.update(content_before_checksum.as_bytes());
    let checksum = format!("{:x}", hasher.finalize());

    // Add checksum epilogue
    let mut result = content_before_checksum;
    // write! to String is infallible but returns Result
    let _ = write!(&mut result, "Checksum: {checksum}\r\n");

    result
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
    async fn test_v1_versions() {
        let state = create_test_state();
        let result = handle_v1_command("v1/products/test_product/versions", &state);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("MIME-Version: 1.0"));
        assert!(response.contains("RibbitBoundary"));
        assert!(response.contains("Checksum:"));
        assert!(response.contains("Region!STRING"));
    }

    #[tokio::test]
    async fn test_v1_summary() {
        let state = create_test_state();
        let result = handle_v1_command("v1/summary", &state);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("MIME-Version: 1.0"));
        assert!(response.contains("Product!STRING"));
        assert!(response.contains("test_product"));
    }

    #[test]
    fn test_mime_wrapping() {
        let bpsv = "Header!STRING:0\ndata1|data2\n## seqn = 123";
        let mime = wrap_in_mime(bpsv);

        assert!(mime.contains("MIME-Version: 1.0\r\n"));
        assert!(mime.contains("multipart/alternative"));
        assert!(mime.contains("--RibbitBoundary\r\n"));
        assert!(mime.contains("--RibbitBoundary--\r\n"));
        assert!(mime.contains("Checksum: "));

        // Checksum should be 64 hex characters
        let checksum_line = mime.lines().last().unwrap();
        assert!(checksum_line.starts_with("Checksum: "));
        let checksum = checksum_line.strip_prefix("Checksum: ").unwrap().trim();
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_checksum_calculation() {
        let bpsv = "test content";
        let mime = wrap_in_mime(bpsv);

        // Extract checksum
        let checksum_line = mime.lines().last().unwrap();
        let checksum = checksum_line.strip_prefix("Checksum: ").unwrap().trim();

        // Verify checksum is correct by recalculating
        let content_before_checksum = mime.split("Checksum:").next().unwrap();
        let mut hasher = Sha256::new();
        hasher.update(content_before_checksum.as_bytes());
        let expected = format!("{:x}", hasher.finalize());

        assert_eq!(checksum, expected);
    }
}

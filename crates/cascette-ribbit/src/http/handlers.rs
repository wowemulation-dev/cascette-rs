//! HTTP request handlers for Ribbit protocol endpoints.

use crate::config::CdnConfig;
use crate::error::DatabaseError;
use crate::responses::BpsvResponse;
use crate::server::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Handle GET /:product/versions endpoint.
///
/// Returns BPSV-formatted version information for the specified product.
///
/// # Errors
///
/// Returns `AppError` if the product is not found or a database error occurs.
pub async fn handle_versions(
    Path(product): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    tracing::debug!("Handling versions request for product: {}", product);

    // Get latest build for product
    let build = state
        .database()
        .latest_build(&product)
        .ok_or_else(|| AppError::NotFound(format!("Product not found: {product}")))?;

    // Generate BPSV response
    let seqn = state.current_seqn();
    let response = BpsvResponse::versions(build, seqn);

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        response.to_string(),
    )
        .into_response())
}

/// Handle GET /:product/cdns endpoint.
///
/// Returns BPSV-formatted CDN configuration for the specified product.
///
/// # Errors
///
/// Returns `AppError` if the product is not found or a database error occurs.
pub async fn handle_cdns(
    Path(product): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    tracing::debug!("Handling cdns request for product: {}", product);

    // Verify product exists
    let build = state
        .database()
        .latest_build(&product)
        .ok_or_else(|| AppError::NotFound(format!("Product not found: {product}")))?;

    // Resolve CDN config for this product (uses product-specific path if available)
    let cdn_config = CdnConfig::resolve_for_build(build, state.cdn_config());

    // Generate BPSV response
    let seqn = state.current_seqn();
    let response = BpsvResponse::cdns(&cdn_config, seqn);

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        response.to_string(),
    )
        .into_response())
}

/// Handle GET /:product/bgdl endpoint.
///
/// Returns BPSV-formatted background download information (same format as versions).
///
/// # Errors
///
/// Returns `AppError` if the product is not found or a database error occurs.
pub async fn handle_bgdl(
    Path(product): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    tracing::debug!("Handling bgdl request for product: {}", product);

    // Get latest build for product
    let build = state
        .database()
        .latest_build(&product)
        .ok_or_else(|| AppError::NotFound(format!("Product not found: {product}")))?;

    // Generate BPSV response (bgdl uses same format as versions)
    let seqn = state.current_seqn();
    let response = BpsvResponse::bgdl(build, seqn);

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        response.to_string(),
    )
        .into_response())
}

/// Application-level error type for HTTP handlers.
#[derive(Debug)]
pub enum AppError {
    /// Resource not found (404)
    NotFound(String),
    /// Database error (500)
    Database(DatabaseError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::Database(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        (status, message).into_response()
    }
}

impl From<DatabaseError> for AppError {
    fn from(err: DatabaseError) -> Self {
        Self::Database(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use std::io::Write;
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
    async fn test_handle_versions() {
        let state = create_test_state();
        let result = handle_versions(Path("test_product".to_string()), State(state)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_versions_not_found() {
        let state = create_test_state();
        let result = handle_versions(Path("nonexistent".to_string()), State(state)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_cdns() {
        let state = create_test_state();
        let result = handle_cdns(Path("test_product".to_string()), State(state)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_bgdl() {
        let state = create_test_state();
        let result = handle_bgdl(Path("test_product".to_string()), State(state)).await;
        assert!(result.is_ok());
    }
}

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

/// Handle GET /v2/summary endpoint.
///
/// Returns BPSV-formatted summary of all products with their sequence numbers.
/// Matches the Blizzard TACT v2 summary format at `https://{region}.version.battle.net/v2/summary`.
pub async fn handle_summary(State(state): State<Arc<AppState>>) -> Result<Response, AppError> {
    tracing::debug!("Handling v2 summary request");

    let db = state.database().await;
    let products = db.products();

    // Use the maximum seqn across all products as the summary-level seqn
    let seqn = if products.is_empty() {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    } else {
        products
            .iter()
            .map(|p| state.current_seqn(p))
            .max()
            .unwrap_or(0)
    };

    let response = BpsvResponse::summary(&products, seqn);

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

    let db = state.database().await;
    let build = db
        .latest_build(&product)
        .ok_or_else(|| AppError::NotFound(format!("Product not found: {product}")))?;

    let seqn = state.current_seqn(&product);
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

    let db = state.database().await;
    let build = db
        .latest_build(&product)
        .ok_or_else(|| AppError::NotFound(format!("Product not found: {product}")))?;

    let cdn_config = CdnConfig::resolve_for_build(build, state.cdn_config());

    let seqn = state.current_seqn(&product);
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

    let db = state.database().await;
    let build = db
        .latest_build(&product)
        .ok_or_else(|| AppError::NotFound(format!("Product not found: {product}")))?;

    let seqn = state.current_seqn(&product);
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

/// Handle GET `/{product}/versions/{build}` and `/v2/products/{product}/versions/{build}`.
///
/// Returns BPSV-formatted version information for a specific build number.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the product or build number is not found.
pub async fn handle_versioned_versions(
    Path((product, build)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    tracing::debug!(
        "Handling versioned versions request for product: {}, build: {}",
        product,
        build
    );

    let db = state.database().await;
    let record = db.find_build(&product, &build).ok_or_else(|| {
        AppError::NotFound(format!("Build {build} not found for product: {product}"))
    })?;

    let seqn = state.current_seqn(&product);
    let response = BpsvResponse::versions(record, seqn);

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

/// Handle GET `/{product}/cdns/{build}` and `/v2/products/{product}/cdns/{build}`.
///
/// Returns BPSV-formatted CDN configuration for a specific build number.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the product or build number is not found.
pub async fn handle_versioned_cdns(
    Path((product, build)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    tracing::debug!(
        "Handling versioned cdns request for product: {}, build: {}",
        product,
        build
    );

    let db = state.database().await;
    let record = db.find_build(&product, &build).ok_or_else(|| {
        AppError::NotFound(format!("Build {build} not found for product: {product}"))
    })?;

    let cdn_config = CdnConfig::resolve_for_build(record, state.cdn_config());

    let seqn = state.current_seqn(&product);
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

/// Handle GET `/{product}/bgdl/{build}` and `/v2/products/{product}/bgdl/{build}`.
///
/// Returns BPSV-formatted background download information for a specific build number.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the product or build number is not found.
pub async fn handle_versioned_bgdl(
    Path((product, build)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    tracing::debug!(
        "Handling versioned bgdl request for product: {}, build: {}",
        product,
        build
    );

    let db = state.database().await;
    let record = db.find_build(&product, &build).ok_or_else(|| {
        AppError::NotFound(format!("Build {build} not found for product: {product}"))
    })?;

    let seqn = state.current_seqn(&product);
    let response = BpsvResponse::bgdl(record, seqn);

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
            http_bind: "127.0.0.1:8080".parse().unwrap(),
            tcp_bind: "127.0.0.1:1119".parse().unwrap(),
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

    #[tokio::test]
    async fn test_handle_summary() {
        let state = create_test_state();
        let result = handle_summary(State(state)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_versioned_versions() {
        let state = create_test_state();
        let result = handle_versioned_versions(
            Path(("test_product".to_string(), "1".to_string())),
            State(state),
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_versioned_versions_build_not_found() {
        let state = create_test_state();
        let result = handle_versioned_versions(
            Path(("test_product".to_string(), "999".to_string())),
            State(state),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_versioned_versions_product_not_found() {
        let state = create_test_state();
        let result = handle_versioned_versions(
            Path(("nonexistent".to_string(), "1".to_string())),
            State(state),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_versioned_cdns() {
        let state = create_test_state();
        let result = handle_versioned_cdns(
            Path(("test_product".to_string(), "1".to_string())),
            State(state),
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_versioned_bgdl() {
        let state = create_test_state();
        let result = handle_versioned_bgdl(
            Path(("test_product".to_string(), "1".to_string())),
            State(state),
        )
        .await;
        assert!(result.is_ok());
    }
}

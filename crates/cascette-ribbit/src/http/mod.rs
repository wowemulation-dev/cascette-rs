//! HTTP/HTTPS server implementation using axum.

use crate::error::ServerError;
use crate::server::AppState;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub mod handlers;

/// Create HTTP router with all endpoints.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/{product}/versions",
            axum::routing::get(handlers::handle_versions),
        )
        .route("/{product}/cdns", axum::routing::get(handlers::handle_cdns))
        .route("/{product}/bgdl", axum::routing::get(handlers::handle_bgdl))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Start HTTP server.
///
/// # Errors
///
/// Returns `ServerError` if the server fails to bind or encounters a runtime error.
pub async fn start_server(bind_addr: SocketAddr, state: Arc<AppState>) -> Result<(), ServerError> {
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|source| ServerError::HttpBindFailed {
            addr: bind_addr,
            source,
        })?;

    tracing::info!("HTTP server listening on {}", bind_addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| ServerError::Shutdown(format!("HTTP server error: {e}")))?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        use crate::config::ServerConfig;
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"[{\"id\":1,\"product\":\"test\",\"version\":\"1.0.0\",\"build\":\"1\",\"build_config\":\"0123456789abcdef0123456789abcdef\",\"cdn_config\":\"fedcba9876543210fedcba9876543210\",\"product_config\":null,\"build_time\":\"2024-01-01T00:00:00+00:00\",\"encoding_ekey\":\"aaaabbbbccccddddeeeeffffaaaaffff\",\"root_ekey\":\"bbbbccccddddeeeeffffaaaabbbbcccc\",\"install_ekey\":\"ccccddddeeeeffffaaaabbbbccccdddd\",\"download_ekey\":\"ddddeeeeffffaaaabbbbccccddddeeee\"}]").unwrap();

        let config = ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: file.path().to_path_buf(),
            cdn_hosts: "cdn.test.com".to_string(),
            cdn_path: "test/path".to_string(),
            tls_cert: None,
            tls_key: None,
        };

        let state = Arc::new(AppState::new(&config).unwrap());
        let _router = create_router(state);

        // Test passes if router creation succeeds without panic
    }
}

//! GET /content/{hash} -- Content delivery by encoding key.
//!
//! Serves raw file content from a registered installation, looked up by
//! encoding key hash. The hash is a 32-character hex string representing
//! the 16-byte encoding key used internally by CASC.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::server::router::AppState;

/// GET /content/{hash} -- serve content by encoding key hash.
pub async fn get_content(State(state): State<Arc<AppState>>, Path(hash): Path<String>) -> Response {
    let Ok(encoding_key) = cascette_crypto::EncodingKey::from_hex(&hash) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("invalid encoding key hex: {hash}"),
        )
            .into_response();
    };

    // Collect install paths from all registered products.
    let products = match state.registry.list().await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("registry error: {e}"),
            )
                .into_response();
        }
    };

    let install_paths: Vec<PathBuf> = products
        .iter()
        .filter_map(|p| p.install_path.as_ref().map(PathBuf::from))
        .filter(|p| p.exists())
        .collect();

    if install_paths.is_empty() {
        return (StatusCode::NOT_FOUND, "no installations registered").into_response();
    }

    // Search each installation for the encoding key.
    for path in &install_paths {
        let Ok(installation) = cascette_client_storage::Installation::open(path.join("Data"))
        else {
            continue;
        };

        if let Err(_e) = installation.initialize().await {
            continue;
        }

        if !installation.has_encoding_key(&encoding_key).await {
            continue;
        }

        if let Ok(data) = installation.read_file_by_encoding_key(&encoding_key).await {
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/octet-stream")
                .header("content-length", data.len())
                .body(Body::from(data))
                .unwrap_or_else(|_| {
                    (StatusCode::INTERNAL_SERVER_ERROR, "response build error").into_response()
                });
        }
    }

    (
        StatusCode::NOT_FOUND,
        format!("encoding key not found: {hash}"),
    )
        .into_response()
}

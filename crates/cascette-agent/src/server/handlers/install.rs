//! POST /install/{product} and POST /install -- Start product installation.
//!
//! The original Agent.exe requires the product to be registered via `/register`
//! first. It does not auto-create products. The bare `/install` endpoint
//! dispatches via `uid` in the JSON body.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::models::operation::{Operation, OperationType, Priority};
use crate::models::product::ProductStatus;
use crate::server::handlers::error_codes::{
    AGENT_ERROR_DIRECTORY_CONFLICT, AGENT_ERROR_DUPLICATE_UID, AGENT_ERROR_INVALID_REQUEST,
};
use crate::server::router::AppState;

/// Install request body.
///
/// Fields: install_path, game_dir (alias for install_path), decryption_buffer,
/// finalized, uid, priority, response_uri, result_uri.
#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    /// Product unique identifier. Required for bare `/install`, optional for
    /// `/install/{product}` (falls back to path parameter).
    pub uid: Option<String>,
    /// Download priority (default 700).
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Response callback URI.
    pub response_uri: Option<String>,
    /// Result callback URI.
    pub result_uri: Option<String>,
    /// Install directory.
    pub install_path: Option<String>,
    /// Alias for install_path.
    pub game_dir: Option<String>,
    /// Armadillo DRM decryption buffer. Passed through in the response.
    /// Empty string if not provided.
    #[serde(default)]
    pub decryption_buffer: String,
    /// Finalization flag.
    #[serde(default)]
    pub finalized: bool,
    /// Custom build config hash for installing a specific version (cascette extension).
    pub build_config: Option<String>,
    /// Custom CDN config hash for installing a specific version (cascette extension).
    pub cdn_config: Option<String>,
    /// Product config hash (cascette extension).
    pub product_config: Option<String>,
}

fn default_priority() -> u32 {
    700
}

/// Shared install logic used by both `/install/{product}` and bare `/install`.
async fn handle_install(
    state: &AppState,
    product_code: &str,
    body: &InstallRequest,
) -> (StatusCode, Json<serde_json::Value>) {
    // Product must exist via /register first. UID lookup is case-insensitive.
    let Ok(product) = state.registry.get(product_code).await else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    };

    // Check product is in a state that allows installation.
    if product.status != ProductStatus::Available {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    }

    // Check for directory conflict: another product at the same install path.
    if let Some(ref path) = product.install_path
        && let Ok(others) = state.registry.find_by_install_path(path).await
    {
        let conflict = others
            .iter()
            .any(|p| p.product_code != product_code && p.status.is_installed());
        if conflict {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": AGENT_ERROR_DIRECTORY_CONFLICT})),
            );
        }
    }

    // Resolve install_path: body.install_path takes precedence, then game_dir
    // alias, then fall back to the product's registered path.
    let resolved_install_path = body
        .install_path
        .as_deref()
        .or(body.game_dir.as_deref())
        .map(String::from)
        .or_else(|| product.install_path.clone());

    let priority = Priority::from_agent_priority(body.priority);
    let params = serde_json::json!({
        "install_path": resolved_install_path,
        "region": product.region,
        "locale": product.locale,
        "build_config": body.build_config,
        "cdn_config": body.cdn_config,
        "finalized": body.finalized,
    });

    let operation = Operation::new(
        product_code.to_string(),
        OperationType::Install,
        priority,
        Some(params),
    );

    match state.queue.insert(&operation).await {
        Ok(()) => {
            state.queue_notify.notify_one();

            // After a successful install, initialize download tracking for the
            // product (priority, download_limit, decryption_buffer).
            tracing::debug!(
                product = product_code,
                priority = body.priority,
                "download config chained from install"
            );

            let response_uri = format!("/install/{product_code}");
            let result_uri = format!("/install/{product_code}");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "response_uri": response_uri,
                    "uid": product_code,
                    "decryption_buffer": body.decryption_buffer,
                    "priority": body.priority,
                    "result_uri": result_uri,
                })),
            )
        }
        Err(e) => {
            let msg = e.to_string();
            // Duplicate UID in the operation queue.
            if msg.contains("active") {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": AGENT_ERROR_DUPLICATE_UID})),
                )
            } else {
                tracing::error!(error = %e, "CreateProductInstall Failed with error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
                )
            }
        }
    }
}

/// POST /install/{product}
pub async fn post_install(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
    Json(body): Json<InstallRequest>,
) -> impl IntoResponse {
    let product_code = body.uid.as_deref().unwrap_or(&product);
    handle_install(&state, product_code, &body).await
}

/// POST /install (bare endpoint, product resolved from body `uid`).
pub async fn post_install_bare(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InstallRequest>,
) -> impl IntoResponse {
    let product_code = match body.uid.as_deref() {
        Some(uid) if !uid.is_empty() => uid.to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
            );
        }
    };
    handle_install(&state, &product_code, &body).await
}

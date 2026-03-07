//! POST /uninstall/{product} and POST /uninstall -- Start product uninstallation.
//!
//! Registers both `/uninstall/{product}` and bare `/uninstall/`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::models::operation::{Operation, OperationType, Priority};
use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::AppState;

/// Uninstall request body.
#[derive(Debug, Deserialize)]
pub struct UninstallRequest {
    /// Product unique identifier.
    pub uid: Option<String>,
    /// Priority (default 700).
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    700
}

/// Shared uninstall logic used by both `/uninstall/{product}` and bare `/uninstall`.
async fn handle_uninstall(
    state: &AppState,
    product_code: &str,
    body: &UninstallRequest,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.registry.get(product_code).await {
        Ok(p) if !p.status.is_installed() => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
            );
        }
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
            );
        }
        Ok(_) => {}
    }

    let priority = Priority::from_agent_priority(body.priority);
    let operation = Operation::new(
        product_code.to_string(),
        OperationType::Uninstall,
        priority,
        None,
    );

    match state.queue.insert(&operation).await {
        Ok(()) => {
            state.queue_notify.notify_one();
            let response_uri = format!("/uninstall/{product_code}");
            let result_uri = format!("/uninstall/{product_code}");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "response_uri": response_uri,
                    "result_uri": result_uri,
                    "uid": product_code,
                    "priority": body.priority,
                })),
            )
        }
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        ),
    }
}

/// POST /uninstall/{product}
pub async fn post_uninstall(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
    Json(body): Json<UninstallRequest>,
) -> impl IntoResponse {
    let product_code = body.uid.as_deref().unwrap_or(&product);
    handle_uninstall(&state, product_code, &body).await
}

/// POST /uninstall (bare endpoint, product resolved from body `uid`).
pub async fn post_uninstall_bare(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UninstallRequest>,
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
    handle_uninstall(&state, &product_code, &body).await
}

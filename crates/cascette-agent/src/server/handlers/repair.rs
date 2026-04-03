//! POST /repair/{product} and POST /repair -- Start product repair.
//!
//! Registers both `/repair/{product}` and bare `/repair/`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::models::operation::{Operation, OperationType, Priority};
use crate::models::product::ProductStatus;
use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::AppState;

/// Repair request body.
#[derive(Debug, Deserialize)]
pub struct RepairRequest {
    /// Product unique identifier.
    pub uid: Option<String>,
    /// Priority (default 700).
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    700
}

/// Shared repair logic used by both `/repair/{product}` and bare `/repair`.
async fn handle_repair(
    state: &AppState,
    product_code: &str,
    body: &RepairRequest,
) -> (StatusCode, Json<serde_json::Value>) {
    // Agent.exe rejects "unsupported" products — those not in the registry or
    // not in an installed state. We mirror this by allowing repair from any
    // installed state (Installed, Corrupted, Updating, Verifying) and
    // rejecting products that are busy with an incompatible operation
    // (Installing, Uninstalling, Repairing) or not yet installed (Available).
    match state.registry.get(product_code).await {
        Ok(p) if !p.status.is_installed() || p.status == ProductStatus::Repairing => {
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
        OperationType::Repair,
        priority,
        None,
    );

    match state.queue.insert(&operation).await {
        Ok(()) => {
            state.queue_notify.notify_one();
            let response_uri = format!("/repair/{product_code}");
            let result_uri = format!("/repair/{product_code}");
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

/// POST /repair/{product}
pub async fn post_repair(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
    Json(body): Json<RepairRequest>,
) -> impl IntoResponse {
    let product_code = body.uid.as_deref().unwrap_or(&product);
    handle_repair(&state, product_code, &body).await
}

/// POST /repair (bare endpoint, product resolved from body `uid`).
pub async fn post_repair_bare(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RepairRequest>,
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
    handle_repair(&state, &product_code, &body).await
}

//! POST /backfill/{product} and POST /backfill -- Start background content fill.
//!
//! Registers both `/backfill/{product}` and bare `/backfill/`.

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

/// Backfill request body.
#[derive(Debug, Deserialize)]
pub struct BackfillRequest {
    /// Product unique identifier.
    pub uid: Option<String>,
    /// Priority (default 700, matching Agent.exe 0x2bc).
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    700
}

/// Shared backfill logic used by both `/backfill/{product}` and bare `/backfill`.
async fn handle_backfill(
    state: &AppState,
    product_code: &str,
    body: &BackfillRequest,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.registry.get(product_code).await {
        Ok(p) if p.status != ProductStatus::Installed => {
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
        OperationType::Backfill,
        priority,
        None,
    );

    match state.queue.insert(&operation).await {
        Ok(()) => {
            state.queue_notify.notify_one();
            let response_uri = format!("/backfill/{product_code}");
            let result_uri = format!("/backfill/{product_code}");
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

/// POST /backfill/{product}
pub async fn post_backfill(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
    Json(body): Json<BackfillRequest>,
) -> impl IntoResponse {
    let product_code = body.uid.as_deref().unwrap_or(&product);
    handle_backfill(&state, product_code, &body).await
}

/// POST /backfill (bare endpoint, product resolved from body `uid`).
pub async fn post_backfill_bare(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BackfillRequest>,
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
    handle_backfill(&state, &product_code, &body).await
}

//! GET /install/{product} (polling) -- Operation progress.
//!
//! When polled via GET, the install endpoint returns the current progress
//! of any active operation on the product. The response includes `response_uri`
//! and `result_uri` fields matching Agent.exe polling behavior.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::server::router::AppState;

/// GET /install/{product} -- poll for installation progress.
pub async fn get_progress(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
) -> impl IntoResponse {
    let response_uri = format!("/install/{product}");
    let result_uri = format!("/install/{product}");

    match state.queue.find_active_for_product(&product).await {
        Ok(Some(op)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "uid": product,
                "operation_id": op.operation_id.to_string(),
                "operation_type": op.operation_type.to_string(),
                "state": op.state.to_string(),
                "progress": op.progress,
                "error": op.error,
                "response_uri": response_uri,
                "result_uri": result_uri,
            })),
        ),
        Ok(None) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "uid": product,
                "state": "idle",
                "response_uri": response_uri,
                "result_uri": result_uri,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

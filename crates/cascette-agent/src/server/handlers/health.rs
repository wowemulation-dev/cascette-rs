//! GET /health -- Health check endpoint (cascette extension).

use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::server::router::AppState;

/// GET /health
pub async fn get_health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let uptime = state.started_at.elapsed().unwrap_or_default().as_secs();

    Json(serde_json::json!({
        "status": "ok",
        "version": state.agent_version,
        "uptime_seconds": uptime,
    }))
}

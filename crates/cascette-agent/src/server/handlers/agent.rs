//! GET/POST /agent -- Agent info and per-product download configuration.
//!
//! GET returns version info and configuration state.
//! POST configures per-product download state (background_download, priority,
//! download_limit, paused).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::{AppState, ProductDownloadConfig};

/// Agent info response (GET /agent).
#[derive(Debug, Serialize)]
pub struct AgentInfoResponse {
    /// Agent version string.
    pub agent_version: String,
    /// Authorization token (placeholder for compatibility).
    pub authorization: String,
    /// Whether the agent allows commands.
    pub allow_commands: bool,
    /// Port the agent is listening on.
    pub port: u16,
    /// Uptime in seconds.
    pub uptime_seconds: u64,
}

/// Per-product download configuration request (POST /agent).
#[derive(Debug, Deserialize)]
pub struct AgentConfigRequest {
    /// Product identifier. Required. Looked up in registry.
    pub uid: String,
    /// Enable background downloads for this product.
    #[serde(default)]
    pub background_download: bool,
    /// Download priority (default 700).
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Download speed limit in bytes per second (0 = unlimited).
    #[serde(default)]
    pub download_limit: u64,
    /// Pause downloads for this product.
    #[serde(default)]
    pub paused: bool,
}

fn default_priority() -> u32 {
    700
}

/// GET /agent
pub async fn get_agent_info(State(state): State<Arc<AppState>>) -> Json<AgentInfoResponse> {
    let uptime = state.started_at.elapsed().unwrap_or_default().as_secs();

    Json(AgentInfoResponse {
        agent_version: state.agent_version.clone(),
        authorization: String::new(),
        allow_commands: state.config.allowcommands,
        port: state.bound_port.load(Ordering::Relaxed),
        uptime_seconds: uptime,
    })
}

/// POST /agent -- configure per-product download state.
pub async fn post_agent_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AgentConfigRequest>,
) -> impl IntoResponse {
    // Validate UID is present and non-empty.
    if body.uid.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    }

    // Product must exist in registry.
    if state.registry.get(&body.uid).await.is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    }

    // Store per-product download config.
    let config = ProductDownloadConfig {
        background_download: body.background_download,
        priority: body.priority,
        download_limit: body.download_limit,
        paused: body.paused,
    };

    // Persist to database.
    if let Err(e) = state.registry.set_download_config(&body.uid, &config).await {
        tracing::warn!(uid = %body.uid, error = %e, "failed to persist download config");
    }

    // Update in-memory cache.
    {
        let mut configs = state.product_download_config.write().await;
        configs.insert(body.uid.clone(), config);
    }

    // Build response.
    let response_uri = format!("/agent/{}", body.uid);
    let result_uri = format!("/agent/{}", body.uid);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "response_uri": response_uri,
            "result_uri": result_uri,
        })),
    )
}

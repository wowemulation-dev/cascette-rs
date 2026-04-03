//! GET/POST /download -- Download speed configuration.
//!
//! GET returns current global download state.
//! POST configures per-product download parameters.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::{AppState, ProductDownloadConfig};

/// Download configuration request (POST /download).
#[derive(Debug, Deserialize)]
pub struct DownloadConfigRequest {
    /// Product identifier. Required.
    pub uid: String,
    /// Speed limit in bytes per second (0 = unlimited).
    #[serde(default)]
    pub download_limit: u64,
    /// Armadillo DRM decryption buffer.
    #[serde(default)]
    pub decryption_buffer: String,
    /// Download priority (default 700).
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    700
}

/// GET /download -- current download state.
pub async fn get_download(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let ds = state.download_state.read().await;
    Json(serde_json::json!({
        "max_speed_bps": ds.max_speed_bps,
        "paused": ds.paused,
        "current_speed_bps": ds.current_speed_bps,
    }))
}

/// POST /download -- configure per-product download parameters.
pub async fn post_download(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DownloadConfigRequest>,
) -> impl IntoResponse {
    // Validate UID.
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
    {
        let config_snapshot = {
            let mut configs = state.product_download_config.write().await;
            let config = configs
                .entry(body.uid.clone())
                .or_insert_with(ProductDownloadConfig::default);
            config.download_limit = body.download_limit;
            config.priority = body.priority;
            let snapshot = config.clone();
            drop(configs);
            snapshot
        };

        // Persist to database (guard already dropped above).
        if let Err(e) = state
            .registry
            .set_download_config(&body.uid, &config_snapshot)
            .await
        {
            tracing::warn!(uid = %body.uid, error = %e, "failed to persist download config");
        }
    }

    // Build response.
    let response_uri = format!("/download/{}", body.uid);
    let result_uri = format!("/download/{}", body.uid);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "response_uri": response_uri,
            "result_uri": result_uri,
        })),
    )
}

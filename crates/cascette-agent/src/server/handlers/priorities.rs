//! GET/POST /priorities -- Product priority list and per-product priority setting.
//!
//! GET returns the priority ordering of all registered products.
//! POST sets the download priority for a specific product.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::{AppState, ProductDownloadConfig};

/// Per-product priority request (POST /priorities).
#[derive(Debug, Deserialize)]
pub struct PriorityRequest {
    /// Product identifier. Required.
    pub uid: String,
    /// Download priority (default 700).
    #[serde(default = "default_priority")]
    pub priority: u32,
}

/// Default download priority.
const DEFAULT_PRIORITY: u32 = 700;

fn default_priority() -> u32 {
    DEFAULT_PRIORITY
}

/// GET /priorities -- list registered products with their download priorities.
///
/// Reads stored per-product priorities from the in-memory cache,
/// falling back to the default (700) for products without explicit config.
pub async fn get_priorities(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let configs = state.product_download_config.read().await;

    let priorities = match state.registry.list().await {
        Ok(products) => products
            .iter()
            .map(|p| {
                let priority = configs
                    .get(&p.product_code)
                    .map_or(DEFAULT_PRIORITY, |c| c.priority);
                serde_json::json!({
                    "uid": p.product_code,
                    "priority": priority,
                })
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };

    Json(serde_json::json!({
        "priorities": priorities,
    }))
}

/// POST /priorities -- set download priority for a product.
pub async fn post_priorities(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PriorityRequest>,
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

    // Store priority per-product.
    {
        let config_snapshot = {
            let mut configs = state.product_download_config.write().await;
            let config = configs
                .entry(body.uid.clone())
                .or_insert_with(ProductDownloadConfig::default);
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
            tracing::warn!(uid = %body.uid, error = %e, "failed to persist priority");
        }
    }

    // Build response.
    let response_uri = format!("/priorities/{}", body.uid);
    let result_uri = format!("/priorities/{}", body.uid);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "response_uri": response_uri,
            "result_uri": result_uri,
        })),
    )
}

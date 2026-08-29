//! GET /agent/download -- Per-product download state.
//!
//! Returns download configuration scoped to a product. The real agent tracks
//! per-product download priority and background download status. This endpoint
//! is separate from POST /download which sets global speed limits.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::server::router::{AppState, ProductDownloadConfig};

/// Query parameters for GET /agent/download.
#[derive(Debug, Deserialize)]
pub struct AgentDownloadQuery {
    /// Product identifier for per-product download state.
    pub uid: Option<String>,
}

/// GET /agent/download -- read download state for a product.
///
/// Returns per-product download configuration from the in-memory cache.
/// Falls back to defaults if no configuration has been set for the product.
pub async fn get_agent_download(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AgentDownloadQuery>,
) -> Json<serde_json::Value> {
    let uid = query.uid.as_deref().unwrap_or("");

    let configs = state.product_download_config.read().await;
    let default_config = ProductDownloadConfig::default();
    let config = configs.get(uid).unwrap_or(&default_config).clone();
    drop(configs);

    Json(serde_json::json!({
        "uid": uid,
        "background_download": config.background_download,
        "priority": config.priority,
        "download_limit": config.download_limit,
        "paused": config.paused,
    }))
}

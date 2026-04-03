//! GET/POST /spawned -- child process spawn tracking.
//! GET/POST /spawned/{product} -- per-product spawn status.
//!
//! Registers `/spawned/{product}` as a dynamic sub-route. Allocates a
//! spawner context, associates the request with a running child process, and
//! returns a `response_uri` pointing to the per-product spawn sub-endpoint.
//!
//! In cascette-agent, game launch lifecycle is tracked via `POST /gamesession`.
//! These endpoints exist for wire compatibility and return valid responses
//! without side effects.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::router::AppState;

/// POST /spawned or POST /spawned/{product} request body.
///
/// Agent.exe records binary path and launch arguments. Fields are accepted
/// but not stored.
#[derive(Debug, Deserialize)]
pub struct SpawnedRequest {
    /// Product UID. Optional — may also be provided via path parameter.
    #[serde(default)]
    pub uid: String,
}

/// GET /spawned -- global spawn status list.
pub async fn get_spawned(_state: State<Arc<AppState>>) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"spawned": []})))
}

/// GET /spawned/{product} -- per-product spawn status.
pub async fn get_spawned_product(
    _state: State<Arc<AppState>>,
    Path(product): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({"uid": product, "spawned": false})),
    )
}

/// POST /spawned -- record a global spawn event.
///
/// Returns `response_uri`.
pub async fn post_spawned(
    _state: State<Arc<AppState>>,
    Json(body): Json<SpawnedRequest>,
) -> impl IntoResponse {
    let response_uri = if body.uid.is_empty() {
        "/spawned".to_string()
    } else {
        format!("/spawned/{}", body.uid)
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({"response_uri": response_uri})),
    )
}

/// POST /spawned/{product} -- record a per-product spawn event.
///
/// Returns `response_uri` for the spawned product sub-endpoint.
pub async fn post_spawned_product(
    _state: State<Arc<AppState>>,
    Path(product): Path<String>,
) -> impl IntoResponse {
    let response_uri = format!("/spawned/{product}");

    (
        StatusCode::OK,
        Json(serde_json::json!({"response_uri": response_uri})),
    )
}

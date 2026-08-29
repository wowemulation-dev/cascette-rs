//! GET/POST /gamesession, /gamesession/{product} -- Game session tracking.
//!
//! Tracks running game processes. The agent validates the caller's OS PID
//! against the `pid` field in the JSON body by checking the supplied PID
//! against live game process PIDs via process_detection — rejecting PIDs that
//! are not known game processes for the given product.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::process_detection;
use crate::server::router::AppState;

/// Session start request.
#[derive(Debug, Deserialize)]
pub struct SessionRequest {
    /// Product unique identifier.
    pub uid: Option<String>,
    /// Process ID of the game.
    pub pid: Option<u32>,
}

/// GET /gamesession -- list all active sessions.
pub async fn get_sessions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sessions = state.session_tracker.list().await;
    let session_data: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "uid": s.product_code,
                "active": true,
                "pid": s.pid,
                "started_at": s.started_at.to_rfc3339(),
            })
        })
        .collect();

    Json(serde_json::json!({
        "sessions": session_data,
    }))
}

/// GET /gamesession/{product} -- get session for specific product.
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
) -> impl IntoResponse {
    // Check tracker first, fall back to process detection
    if let Some(session) = state.session_tracker.get(&product).await {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "uid": product,
                "active": true,
                "pid": session.pid,
                "started_at": session.started_at.to_rfc3339(),
            })),
        );
    }

    // Fall back to process detection
    let active = process_detection::is_game_running(&product);
    let pids = if active {
        process_detection::game_pids(&product)
    } else {
        vec![]
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "uid": product,
            "active": active,
            "pid": pids.first().copied(),
        })),
    )
}

/// POST /gamesession/{product} -- start or update a game session.
///
/// If a `pid` is supplied in the request body, it is validated against live
/// game process PIDs for the product (via `process_detection::game_pids`).
/// A PID that does not belong to a known game process for this product is
/// rejected with HTTP 400.
pub async fn post_session(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
    Json(body): Json<SessionRequest>,
) -> impl IntoResponse {
    let uid = body.uid.as_deref().unwrap_or(&product);

    // Validate pid against live game processes if supplied.
    if let Some(pid) = body.pid {
        let live_pids = process_detection::game_pids(uid);
        // If the product has known executables but the PID is not among them,
        // reject the request. If the product has no known executables (empty list),
        // allow through — unrecognised products should not block session tracking.
        let names = process_detection::executable_names(uid);
        if !names.is_empty() && !live_pids.contains(&pid) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": 2312,
                    "message": format!("Unable to validate connecting process ({pid})"),
                })),
            );
        }
    }

    state.session_tracker.start_session(uid, body.pid).await;

    // Update metrics
    let count = state.session_tracker.count().await;
    #[allow(clippy::cast_possible_wrap)]
    state.metrics.active_game_sessions.set(count as i64);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "uid": uid,
            "active": true,
            "pid": body.pid,
        })),
    )
}

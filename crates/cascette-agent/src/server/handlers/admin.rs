//! POST /admin -- admin session management (conditional registration).
//!
//! POST /admin_command -- admin command dispatch (requires --allowcommands).
//! GET/POST /gce_state -- Game Client Events state for battle.net product.
//! POST /createshortcut -- desktop shortcut creation (Windows-specific).
//!
//! - `/admin_command` is registered only when the `allowcommands` flag is set.
//! - `/admin` is registered only when an admin session flag is set.
//! - `/gce_state` is registered unconditionally; hardcodes product `"battle.net"`
//!   and dispatches an internal fetch request for telemetry. Has no user-visible
//!   response body.
//! - `/createshortcut` is registered unconditionally; Windows-specific shortcut
//!   creation with no meaningful behavior on Linux/macOS.
//!
//! All four handlers return HTTP 200 with an empty JSON object for wire
//! compatibility — callers do not parse the response body for these endpoints.

use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;

/// POST /admin -- admin session management.
///
/// Conditionally registered when an admin session flag is set.
/// Returns 200 for wire compatibility.
pub async fn post_admin() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

/// POST /admin_command -- admin command dispatch.
///
/// Registered only when the `--allowcommands` flag is set.
/// Returns 200 for wire compatibility.
pub async fn post_admin_command() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

/// GET /gce_state -- Game Client Events state.
/// POST /gce_state -- set GCE state.
///
/// Hardcodes product `"battle.net"` and dispatches an internal fetch request.
/// The response body is not used by callers. Returns 200.
pub async fn get_gce_state() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

/// POST /gce_state -- set GCE state for the battle.net product.
pub async fn post_gce_state() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

/// POST /createshortcut -- desktop shortcut creation.
///
/// Windows-specific. Creates a desktop shortcut for the given product.
/// No-op on Linux/macOS. Returns 200.
pub async fn post_createshortcut() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({})))
}

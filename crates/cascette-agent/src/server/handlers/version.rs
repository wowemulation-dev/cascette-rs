//! GET /version -- Version information.
//!
//! Agent.exe accepts an optional `uid` query parameter and returns a single
//! `version` string: `"TACT x.y.z (CASC a.b.c)"`. The uid is ignored; the
//! response is the same regardless of its value.
//!
//! cascette-agent returns the wire-compatible `version` field plus additional
//! informational fields (`agent_version`, `product_version`, `build_date`).
//! These extensions are not present in Agent.exe but do not break callers that
//! only read `version`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::server::router::AppState;

/// Query parameters for GET /version.
#[derive(Debug, Deserialize)]
pub struct VersionQuery {
    /// Product identifier (optional, accepted for wire compatibility, ignored).
    pub uid: Option<String>,
}

/// GET /version
pub async fn get_version(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<VersionQuery>,
) -> Json<serde_json::Value> {
    let pkg_version = env!("CARGO_PKG_VERSION");

    Json(serde_json::json!({
        "version": format!("cascette-agent {pkg_version}"),
        "agent_version": state.agent_version,
        "product_version": pkg_version,
        "build_date": env!("CASCETTE_BUILD_DATE"),
    }))
}

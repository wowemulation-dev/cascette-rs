//! GET /game, GET /game/{product}, POST /game -- Product listing, details, and launch config.
//!
//! GET returns product lists and details.
//! POST configures game launch parameters (binary_type, run64bit, launch_arguments).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::AppState;

/// Product entry in the game list response.
///
/// Field names match the Blizzard Agent wire format: result_uri, uid, region,
/// product_code, install_dir, subpath, and optionally conflict_install_dir.
#[derive(Debug, Serialize)]
pub struct GameEntry {
    /// Dynamic result URL (e.g. "/game/{uid}").
    pub result_uri: String,
    /// Product unique identifier.
    pub uid: String,
    /// Product region code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// TACT product code (e.g. "wow_classic").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_code: Option<String>,
    /// Installation directory path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    /// Sub-directory within install_dir.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
    /// Conflicting installation directory (present only when a conflict exists).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_install_dir: Option<String>,
}

/// Binary type enum for the game launch configuration.
///
/// Compared case-insensitively. Unrecognized values default to `Game3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BinaryType {
    /// Main game binary.
    Game = 0,
    /// Game variant 0.
    Game0 = 1,
    /// Game variant 1.
    Game1 = 2,
    /// Game variant 2.
    Game2 = 3,
    /// Game variant 3 / default.
    Game3 = 4,
    /// Editor binary.
    Editor = 5,
}

impl BinaryType {
    /// Parse from string using case-insensitive comparison.
    fn from_str_ci(s: &str) -> Self {
        if s.eq_ignore_ascii_case("editor") {
            Self::Editor
        } else if s.eq_ignore_ascii_case("game") {
            Self::Game
        } else if s.eq_ignore_ascii_case("game_0") {
            Self::Game0
        } else if s.eq_ignore_ascii_case("game_1") {
            Self::Game1
        } else if s.eq_ignore_ascii_case("game_2") {
            Self::Game2
        } else if s.eq_ignore_ascii_case("game_3") {
            Self::Game3
        } else {
            // Default when no match: Game3
            Self::Game3
        }
    }
}

/// Game launch configuration request (POST /game).
#[derive(Debug, Deserialize)]
pub struct GameConfigRequest {
    /// Product identifier. Required.
    pub uid: String,
    /// Binary type selector (case-insensitive).
    #[serde(default)]
    pub binary_type: Option<String>,
    /// Use 64-bit binary.
    #[serde(default)]
    pub run64bit: bool,
    /// CLI arguments for game launch.
    #[serde(default)]
    pub launch_arguments: Vec<String>,
}

/// GET /game -- list all products.
pub async fn list_games(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<GameEntry>>, (StatusCode, Json<serde_json::Value>)> {
    let products = state.registry.list().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let entries: Vec<GameEntry> = products
        .into_iter()
        .map(|p| {
            let uid = p.product_code.clone();
            GameEntry {
                result_uri: format!("/game/{uid}"),
                uid,
                region: p.region,
                product_code: Some(p.product_code),
                install_dir: p.install_path,
                subpath: None,
                conflict_install_dir: None,
            }
        })
        .collect();

    Ok(Json(entries))
}

/// GET /game/{product} -- single product details.
pub async fn get_game(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
) -> impl IntoResponse {
    match state.registry.get(&product).await {
        Ok(p) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "result_uri": format!("/game/{}", p.product_code),
                "uid": p.product_code,
                "region": p.region,
                "product_code": p.product_code,
                "install_dir": p.install_path,
            })),
        ),
        // Unknown product: return error 2312 with HTTP 400.
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        ),
    }
}

/// POST /game -- configure game launch parameters.
///
/// Validates the UID, resolves the binary path, and writes `Launcher.db`.
/// Returns `{"response_uri": "...", "launch_path": "..."}` on success or
/// `{"error": 2312}` on failure.
pub async fn post_game_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GameConfigRequest>,
) -> impl IntoResponse {
    // Validate UID.
    if body.uid.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    }

    // Product must exist in registry.
    let Ok(product) = state.registry.get(&body.uid).await else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    };

    // Parse binary_type using case-insensitive comparison.
    let _binary_type = body
        .binary_type
        .as_deref()
        .map_or(BinaryType::Game, BinaryType::from_str_ci);

    // Resolve launch_path from install directory.
    // The path depends on binary_type and run64bit selections.
    // We return the install directory as base; actual executable resolution
    // requires per-product binary mapping not yet implemented.
    let launch_path = product.install_path.clone().unwrap_or_default();

    // If the install path is set but does not exist on disk, return an error
    // matching the UID-not-found error code.
    if !launch_path.is_empty() && !std::path::Path::new(&launch_path).exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    }

    // Write Launcher.db in the install directory with minimal game config
    // to persist launch parameters across sessions.
    if let Some(ref path) = product.install_path {
        write_launcher_db(path, &body).await;
    }

    let response_uri = format!("/game/{}", body.uid);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "response_uri": response_uri,
            "launch_path": launch_path,
        })),
    )
}

/// Write Launcher.db to the install directory with game launch configuration.
///
/// Agent.exe writes this file during `HandleGameConfig` to persist launch
/// parameters. We write a JSON-encoded config matching the known fields.
/// Errors are non-fatal — the response is returned regardless.
async fn write_launcher_db(install_path: &str, body: &GameConfigRequest) {
    use tokio::io::AsyncWriteExt;

    let db_path = std::path::Path::new(install_path).join("Launcher.db");
    let content = serde_json::json!({
        "uid": body.uid,
        "binary_type": body.binary_type,
        "run64bit": body.run64bit,
        "launch_arguments": body.launch_arguments,
    });
    let bytes = content.to_string().into_bytes();

    match tokio::fs::File::create(&db_path).await {
        Ok(mut f) => {
            if let Err(e) = f.write_all(&bytes).await {
                tracing::warn!(
                    path = %db_path.display(),
                    error = %e,
                    "failed to write Launcher.db"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                path = %db_path.display(),
                error = %e,
                "failed to create Launcher.db"
            );
        }
    }
}

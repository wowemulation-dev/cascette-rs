//! POST /register -- Product registration.
//!
//! The launcher calls this endpoint to register a product with the agent,
//! providing its install directory, patch URL, and protocol. The agent creates
//! a product entry, detects install-path conflicts with existing products,
//! and chains to the install logic.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use tracing::debug;

use crate::models::operation::{Operation, OperationType, Priority};
use crate::models::product::Product;
use crate::server::handlers::error_codes::{
    AGENT_ERROR_DUPLICATE_UID, AGENT_ERROR_INVALID_CONFIG, AGENT_ERROR_INVALID_PROTOCOL,
    AGENT_ERROR_INVALID_REQUEST,
};
use crate::server::router::AppState;

/// Build a per-game JSON entry.
///
/// Fields: result_uri, uid, region, product_code, install_dir,
/// subpath, and optionally conflict_install_dir.
fn build_game_entry(p: &Product, conflict_install_dir: Option<&str>) -> serde_json::Value {
    let mut entry = serde_json::json!({
        "result_uri": format!("/install/{}", p.product_code),
        "uid": p.product_code,
        "region": p.region,
        "product_code": p.name,
        "install_dir": p.install_path,
    });
    if let Some(ref sub) = p.subfolder {
        entry["subpath"] = serde_json::Value::String(sub.clone());
    }
    if let Some(dir) = conflict_install_dir {
        entry["conflict_install_dir"] = serde_json::Value::String(dir.to_string());
    }
    entry
}

/// Registration request body.
///
/// Fields match Agent.exe wire format. The `build_config`, `cdn_config`, and
/// `product_config` fields are cascette extensions not present in the original
/// agent.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Product identifier (e.g., "wow_classic").
    pub uid: String,
    /// Product code.
    pub product: String,
    /// Installation directory.
    pub install_dir: String,
    /// Region code (e.g., "us").
    pub region: String,
    /// Patch server URL.
    pub instructions_patch_url: String,
    /// Protocol identifier. Agent.exe requires exact `"NGDP"` match.
    pub instructions_product: String,
    /// Subdirectory within install_dir.
    pub subfolder: Option<String>,
    /// Preferred locale hint.
    pub primary_locale_hint: Option<String>,
    /// Patch region hint from the launcher.
    pub patch_region_hint: Option<String>,
    /// Build config hash (cascette extension).
    pub build_config: Option<String>,
    /// CDN config hash (cascette extension).
    pub cdn_config: Option<String>,
    /// Product config hash (cascette extension).
    pub product_config: Option<String>,
}

/// POST /register -- register a product with the agent.
pub async fn post_register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Validate required fields are non-empty.
    if body.uid.is_empty() || body.product.is_empty() || body.install_dir.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
        );
    }

    // Protocol validation: case-insensitive comparison against "NGDP".
    if !body.instructions_product.eq_ignore_ascii_case("NGDP") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_PROTOCOL})),
        );
    }

    // Validate patch URL is non-empty.
    if body.instructions_patch_url.is_empty() || body.region.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_PROTOCOL})),
        );
    }

    // Validate cascette extension fields if present (hex hash format).
    if let Some(ref bc) = body.build_config
        && !bc.is_empty()
        && !bc.chars().all(|c| c.is_ascii_hexdigit())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_CONFIG})),
        );
    }
    if let Some(ref cc) = body.cdn_config
        && !cc.is_empty()
        && !cc.chars().all(|c| c.is_ascii_hexdigit())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_INVALID_CONFIG})),
        );
    }

    let install_path = match &body.subfolder {
        Some(sub) if !sub.is_empty() => format!("{}/{sub}", body.install_dir),
        _ => body.install_dir.clone(),
    };

    // UID duplication check: if the same UID exists at a different path, reject.
    if let Ok(existing) = state.registry.get(&body.uid).await
        && let Some(ref existing_path) = existing.install_path
        && *existing_path != install_path
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": AGENT_ERROR_DUPLICATE_UID})),
        );
    }

    let mut product = Product::new(body.uid.clone(), body.product.clone());
    product.install_path = Some(install_path.clone());
    product.region = Some(body.region.clone());
    product.locale.clone_from(&body.primary_locale_hint);
    product.patch_url = Some(body.instructions_patch_url.clone());
    product.protocol = Some("NGDP".to_string());
    product.subfolder.clone_from(&body.subfolder);
    product
        .patch_region_hint
        .clone_from(&body.patch_region_hint);
    product.build_config.clone_from(&body.build_config);
    product.cdn_config.clone_from(&body.cdn_config);

    // Detect conflicts: other products at the same install path.
    let conflicting_products = match state.registry.find_by_install_path(&install_path).await {
        Ok(products) => products
            .into_iter()
            .filter(|p| p.product_code != body.uid)
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    let has_conflicts = !conflicting_products.is_empty();
    // Build a set of conflicting product codes for per-entry conflict_install_dir.
    let conflict_uids: std::collections::HashSet<String> = conflicting_products
        .iter()
        .map(|p| p.product_code.clone())
        .collect();
    let conflicted: Vec<serde_json::Value> = conflicting_products
        .iter()
        .map(|p| build_game_entry(p, None))
        .collect();

    if has_conflicts {
        debug!(
            uid = %body.uid,
            conflicts = conflict_uids.len(),
            "install path conflicts detected"
        );
    }

    let is_new = match state.registry.register(&product).await {
        Ok(created) => created,
        Err(e) => {
            tracing::error!(error = %e, "failed to register product");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
            );
        }
    };

    // Chain to install: create an install operation for newly registered products.
    if is_new {
        let priority = Priority::from_agent_priority(700);
        let params = serde_json::json!({
            "install_path": install_path,
            "region": body.region,
            "locale": body.primary_locale_hint,
            "build_config": body.build_config,
            "cdn_config": body.cdn_config,
        });

        let operation = Operation::new(
            body.uid.clone(),
            OperationType::Install,
            priority,
            Some(params),
        );

        if let Err(e) = state.queue.insert(&operation).await {
            tracing::warn!(error = %e, uid = %body.uid, "failed to chain install after register");
        } else {
            state.queue_notify.notify_one();
        }
    }

    // Build games list from registry, filtering out "bna" products.
    let games = match state.registry.list().await {
        Ok(products) => products
            .iter()
            .filter(|p| !p.product_code.contains("bna") && !p.name.contains("bna"))
            .map(|p| {
                let conflict_dir = if conflict_uids.contains(&p.product_code) {
                    Some(install_path.clone())
                } else {
                    None
                };
                build_game_entry(p, conflict_dir.as_deref())
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "response_uri": format!("/install/{}", body.uid),
            "result_uri": format!("/install/{}", body.uid),
            "status": product.status.to_string(),
            "games": games,
            "conflicted_games": conflicted,
        })),
    )
}

//! GET/POST /agent/override -- CDN, version service, and config server overrides.
//! GET/POST /agent/{product} -- per-product override state.
//!
//! The override state is stored in-memory and reset when the agent process exits.
//! Battle.net client uses these overrides to redirect CDN or version service traffic
//! for debugging or staging purposes.
//!
//! ## GET /agent/override
//!
//! Returns the current override configuration with four fields:
//! - `patch_url_override`: array of per-product CDN URL overrides
//! - `version_service_overrides`: global version service override string
//! - `version_server_url_override`: direct version server URL override
//! - `config_overrides`: object of arbitrary key/value config pairs
//!
//! ## POST /agent/override
//!
//! Applies new overrides. Accepts the same four fields plus
//! `version_server_override` (a named server override). Returns a
//! `failed_overrides` array for any entries that could not be applied.
//!
//! ## GET /agent/{product}
//!
//! Returns per-product session state combining install status, session context,
//! and any active per-product overrides. Fields:
//! - `pid`: active game session PID, or 0
//! - `user_id`, `user_name`, `session`: session context strings (empty when no
//!   session is active)
//! - `state`: integer install state (1003 = installing, 1004 = installed, 1007 = not installed)
//! - `version`: installed version string, or empty
//! - `region`: configured region, or empty
//! - `type`: product type string ("agent")
//! - `opt_in_feedback`: always true (matches Agent.exe default)
//! - `patch_url_override`: per-product CDN URL overrides (array)
//! - `version_service_overrides`: version service overrides (array)
//! - `version_server_url_override`: direct version server URL override
//!
//! ## POST /agent/{product}
//!
//! Applies per-product overrides. Accepted fields:
//! - `patch_url_override`: array of key/value CDN URL override entries
//! - `version_server_url_override`: direct version server URL
//! - `version_server_override`: named version server
//! - `account_country`, `geo_ip_country`: country overrides
//! - `region`: region redirect (triggers CDN re-routing in Agent.exe)

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::router::AppState;

/// Per-product CDN URL override entry.
///
/// Maps a product identifier to an alternate CDN patch URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchUrlOverride {
    /// Product identifier (e.g. `"wow_classic_era"`).
    pub product: String,
    /// Alternate CDN patch URL for this product.
    pub url: String,
}

/// Override configuration stored in-memory and returned by GET.
///
/// All fields are optional — the agent starts with no overrides active.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OverrideConfig {
    /// Per-product CDN URL overrides.
    pub patch_url_override: Vec<PatchUrlOverride>,
    /// Global version service override (e.g. `"us.version.battle.net:1119"`).
    pub version_service_overrides: String,
    /// Direct version server URL override.
    pub version_server_url_override: String,
    /// Named version server override (stored, not returned by GET).
    pub version_server_override: String,
    /// Arbitrary config key/value pairs applied over the build config.
    pub config_overrides: HashMap<String, String>,
}

/// POST /agent/override request body.
///
/// All fields are optional. Omitted fields leave the current value unchanged.
#[derive(Debug, Deserialize)]
pub struct SetOverrideRequest {
    /// Per-product CDN URL overrides to set.
    pub patch_url_override: Option<Vec<PatchUrlOverride>>,
    /// Global version service override.
    pub version_service_overrides: Option<String>,
    /// Direct version server URL override.
    pub version_server_url_override: Option<String>,
    /// Named version server override.
    pub version_server_override: Option<String>,
    /// Arbitrary config key/value pairs.
    pub config_overrides: Option<HashMap<String, String>>,
}

/// GET /agent/override -- return current override configuration.
///
/// Returns the four user-visible override fields. `version_server_override`
/// is not returned (internal field, not surfaced by GET).
pub async fn get_override_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.override_config.read().await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "patch_url_override": cfg.patch_url_override,
            "version_service_overrides": cfg.version_service_overrides,
            "version_server_url_override": cfg.version_server_url_override,
            "config_overrides": cfg.config_overrides,
        })),
    )
}

/// POST /agent/override -- apply new override configuration.
///
/// Merges the supplied fields into the current override state.
/// Returns `failed_overrides` — an empty array when all overrides are accepted.
pub async fn post_override_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetOverrideRequest>,
) -> impl IntoResponse {
    let mut cfg = state.override_config.write().await;

    if let Some(patch_urls) = body.patch_url_override {
        cfg.patch_url_override = patch_urls;
    }
    if let Some(vs) = body.version_service_overrides {
        cfg.version_service_overrides = vs;
    }
    if let Some(vsu) = body.version_server_url_override {
        cfg.version_server_url_override = vsu;
    }
    if let Some(vso) = body.version_server_override {
        cfg.version_server_override = vso;
    }
    if let Some(co) = body.config_overrides {
        cfg.config_overrides = co;
    }
    drop(cfg);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "failed_overrides": [],
        })),
    )
}

/// POST /agent/{product} request body.
///
/// All fields are optional.
#[derive(Debug, Deserialize)]
pub struct SetProductOverrideRequest {
    /// Per-product CDN URL override entries. Each entry is a key/url pair.
    pub patch_url_override: Option<Vec<PatchUrlOverride>>,
    /// Direct version server URL override.
    pub version_server_url_override: Option<String>,
    /// Named version server override.
    pub version_server_override: Option<String>,
    /// Account country override.
    pub account_country: Option<String>,
    /// Geo-IP country override.
    pub geo_ip_country: Option<String>,
    /// Region redirect (triggers CDN re-routing).
    pub region: Option<String>,
}

/// Integer state codes used in GET /agent/{product} `state` field.
///
/// - 1003: product is in an installing/transitioning state
/// - 1004: product is installed and ready
/// - 1007: product is not installed (available)
const STATE_INSTALLING: u32 = 1003;
const STATE_INSTALLED: u32 = 1004;
const STATE_NOT_INSTALLED: u32 = 1007;

/// GET /agent/{product} -- per-product session and override state.
///
/// Returns a snapshot of the product's install state, active game session,
/// and any per-product overrides stored in the global override config.
pub async fn get_product_override_state(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
) -> impl IntoResponse {
    let product_lower = product.to_ascii_lowercase();

    // Resolve install state and version from the registry.
    let (install_state, version, region) = if let Ok(p) = state.registry.get(&product_lower).await {
        use crate::models::product::ProductStatus;
        let s = match p.status {
            ProductStatus::Installed | ProductStatus::Corrupted => STATE_INSTALLED,
            ProductStatus::Installing
            | ProductStatus::Updating
            | ProductStatus::Repairing
            | ProductStatus::Verifying
            | ProductStatus::Uninstalling => STATE_INSTALLING,
            ProductStatus::Available => STATE_NOT_INSTALLED,
        };
        (
            s,
            p.version.unwrap_or_default(),
            p.region.unwrap_or_default(),
        )
    } else {
        (STATE_NOT_INSTALLED, String::new(), String::new())
    };

    // Resolve active session context.
    let (pid, session_token) =
        if let Some(session) = state.session_tracker.get(&product_lower).await {
            (session.pid.unwrap_or(0), session.started_at.to_rfc3339())
        } else {
            (0u32, String::new())
        };

    // Read per-product overrides from the global override config.
    let (patch_url_override, version_server_url_override) = {
        let cfg = state.override_config.read().await;
        let patch = cfg
            .patch_url_override
            .iter()
            .filter(|e| e.product.eq_ignore_ascii_case(&product_lower))
            .cloned()
            .collect::<Vec<_>>();
        let vsu = cfg.version_server_url_override.clone();
        drop(cfg);
        (patch, vsu)
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "pid": pid,
            "user_id": "",
            "user_name": "",
            "state": install_state,
            "version": version,
            "region": region,
            "type": "agent",
            "opt_in_feedback": true,
            "session": session_token,
            "patch_url_override": patch_url_override,
            "version_service_overrides": [],
            "version_server_url_override": version_server_url_override,
        })),
    )
}

/// POST /agent/{product} -- apply per-product override state.
///
/// Merges the supplied per-product overrides into the global override config.
/// Returns HTTP 200 on success.
pub async fn post_product_override_state(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
    Json(body): Json<SetProductOverrideRequest>,
) -> impl IntoResponse {
    let product_lower = product.to_ascii_lowercase();
    let mut cfg = state.override_config.write().await;

    // Apply patch_url_override entries for this product.
    if let Some(entries) = body.patch_url_override {
        // Remove existing entries for this product, then append new ones.
        cfg.patch_url_override
            .retain(|e| !e.product.eq_ignore_ascii_case(&product_lower));
        cfg.patch_url_override.extend(entries);
    }

    if let Some(vsu) = body.version_server_url_override {
        cfg.version_server_url_override = vsu;
    }
    if let Some(vso) = body.version_server_override {
        cfg.version_server_override = vso;
    }
    // account_country, geo_ip_country, region are accepted but not stored
    // separately — Agent.exe uses them for internal CDN routing which is
    // handled by the download pipeline, not the override manager directly.
    drop(cfg);

    (StatusCode::OK, Json(serde_json::json!({})))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Default OverrideConfig has no overrides set.
    #[test]
    fn test_override_config_default() {
        let cfg = OverrideConfig::default();
        assert!(cfg.patch_url_override.is_empty());
        assert!(cfg.version_service_overrides.is_empty());
        assert!(cfg.version_server_url_override.is_empty());
        assert!(cfg.version_server_override.is_empty());
        assert!(cfg.config_overrides.is_empty());
    }

    /// OverrideConfig serialises to the expected JSON shape (GET response fields).
    #[test]
    fn test_override_config_serialization() {
        let cfg = OverrideConfig {
            patch_url_override: vec![PatchUrlOverride {
                product: "wow_classic_era".to_string(),
                url: "http://cdn.example.com".to_string(),
            }],
            version_service_overrides: "us.version.battle.net:1119".to_string(),
            version_server_url_override: String::new(),
            version_server_override: String::new(),
            config_overrides: {
                let mut m = HashMap::new();
                m.insert("some_key".to_string(), "some_value".to_string());
                m
            },
        };

        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(
            json["version_service_overrides"],
            "us.version.battle.net:1119"
        );
        assert_eq!(json["patch_url_override"][0]["product"], "wow_classic_era");
        assert_eq!(
            json["patch_url_override"][0]["url"],
            "http://cdn.example.com"
        );
        assert_eq!(json["config_overrides"]["some_key"], "some_value");
    }

    /// SetOverrideRequest with all None fields deserialises cleanly.
    #[test]
    fn test_set_override_request_empty() {
        let json = serde_json::json!({});
        let req: SetOverrideRequest = serde_json::from_value(json).unwrap();
        assert!(req.patch_url_override.is_none());
        assert!(req.version_service_overrides.is_none());
        assert!(req.config_overrides.is_none());
    }

    /// Applying a SetOverrideRequest merges fields into OverrideConfig.
    #[test]
    fn test_apply_override_request() {
        let mut cfg = OverrideConfig::default();

        // Simulate what the POST handler does (without Axum state machinery).
        let patch_urls = vec![PatchUrlOverride {
            product: "wow".to_string(),
            url: "http://example.com".to_string(),
        }];
        cfg.patch_url_override = patch_urls;
        cfg.version_service_overrides = "override.example.com:1119".to_string();

        assert_eq!(cfg.patch_url_override.len(), 1);
        assert_eq!(cfg.patch_url_override[0].product, "wow");
        assert_eq!(cfg.version_service_overrides, "override.example.com:1119");
        // Unset fields remain at defaults.
        assert!(cfg.config_overrides.is_empty());
    }
}

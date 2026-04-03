//! POST /size_estimate + GET /size_estimate/{uid} -- async size estimation.
//!
//! Matches the Agent.exe `HandleSizeEstimate` wire format: POST starts a
//! background estimation task, GET polls for the result.

use std::io::Cursor;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use binrw::BinRead;
use serde::Deserialize;
use tracing::{debug, warn};

use cascette_crypto::{TactKey, TactKeyStore};
use cascette_formats::blte::BlteFile;
use cascette_formats::config::BuildConfig;
use cascette_formats::size::SizeManifest;
use cascette_protocol::ContentType;

use super::error_codes::AGENT_ERROR_INVALID_CONFIG;
use crate::executor::helpers::resolve_product_metadata;
use crate::server::router::AppState;
use crate::state::size_cache::SizeEstimateStatus;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// POST request body for size estimation.
#[derive(Debug, Deserialize)]
pub struct SizeEstimateRequest {
    /// Product unique identifier.
    pub uid: Option<String>,
    /// Optional TACT encryption key for decrypting CDN content.
    ///
    /// Parsed as `{key_id_hex}:{key_value_hex}` (e.g.
    /// `FA505078126ACB3E:BDC51862ABED79B2DE48C8E7E66C6200`).
    /// Added to the hardcoded key store before the estimation pipeline runs.
    /// If the format does not match or parsing fails, the field is ignored.
    #[serde(default)]
    pub encryption_key: Option<String>,
}

/// POST /size_estimate -- start a background size estimation.
pub async fn post_size_estimate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SizeEstimateRequest>,
) -> impl IntoResponse {
    let uid = match body.uid {
        Some(ref u) if !u.is_empty() => u.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": AGENT_ERROR_INVALID_CONFIG })),
            );
        }
    };

    // If entry already exists (pending or done), return the poll URI without
    // spawning a duplicate task.
    if !state.size_cache.insert_pending(&uid).await {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "response_uri": format!("/size_estimate/{uid}") })),
        );
    }

    let bg_state = Arc::clone(&state);
    let bg_uid = uid.clone();
    let bg_key = body.encryption_key.clone();
    tokio::spawn(async move {
        estimate_size_background(&bg_state, &bg_uid, bg_key.as_deref()).await;
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({ "response_uri": format!("/size_estimate/{uid}") })),
    )
}

/// GET /size_estimate/{uid} -- poll for estimation result.
pub async fn get_size_estimate_result(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let response_uri = format!("/size_estimate/{uid}");

    let Some(entry) = state.size_cache.get(&uid).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "response_uri": response_uri })),
        );
    };

    match entry.status {
        SizeEstimateStatus::Pending => (
            StatusCode::OK,
            Json(serde_json::json!({
                "uid": uid,
                "estimated_bytes": 0,
                "response_uri": response_uri,
            })),
        ),
        SizeEstimateStatus::Ready(bytes) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "uid": uid,
                "estimated_bytes": bytes,
                "response_uri": response_uri,
            })),
        ),
        SizeEstimateStatus::Failed => (
            StatusCode::OK,
            Json(serde_json::json!({
                "uid": uid,
                "estimated_bytes": 0,
                "error": AGENT_ERROR_INVALID_CONFIG,
            })),
        ),
    }
}

/// Background task: resolve metadata, fetch build config, and compute size.
async fn estimate_size_background(state: &AppState, uid: &str, encryption_key: Option<&str>) {
    match do_estimate(state, uid, encryption_key).await {
        Ok(bytes) => {
            debug!(uid, estimated_bytes = bytes, "size estimation completed");
            state.size_cache.set_ready(uid, bytes).await;
        }
        Err(e) => {
            warn!(uid, error = %e, "size estimation failed");
            state.size_cache.set_failed(uid).await;
        }
    }
}

/// Perform the actual size estimation.
///
/// Tries the size manifest first (accurate for builds that have one), then
/// falls back to CDN config archive-size counting for older builds.
///
/// If `encryption_key` is `Some("{id_hex}:{key_hex}")`, the parsed key is added
/// to the TACT key store used for BLTE decryption during estimation.
async fn do_estimate(
    state: &AppState,
    product: &str,
    encryption_key: Option<&str>,
) -> Result<u64, BoxError> {
    let key_store = build_estimation_key_store(encryption_key);
    let cdn_overrides = state.config.cdn_endpoint_overrides();

    let meta = resolve_product_metadata(
        &state.ribbit_client,
        &state.cdn_client,
        product,
        "us",
        cdn_overrides.as_deref(),
    )
    .await?;

    // Download and parse build config.
    let build_config_key = hex::decode(&meta.build_config)?;
    let build_config_data = state
        .cdn_client
        .download_from_endpoints(&meta.endpoints, ContentType::Config, &build_config_key)
        .await?;
    let build_config = BuildConfig::parse(Cursor::new(&build_config_data))
        .map_err(|e| -> BoxError { e.to_string().into() })?;

    // Try the size manifest path first (available on newer builds).
    if let Some(size_info) = build_config.size()
        && let Some(ref ekey) = size_info.encoding_key
    {
        let ekey_bytes = hex::decode(ekey)?;
        match state
            .cdn_client
            .download_from_endpoints(&meta.endpoints, ContentType::Data, &ekey_bytes)
            .await
        {
            Ok(blte_data) => {
                let mut cursor = Cursor::new(&blte_data);
                let blte = BlteFile::read_options(&mut cursor, binrw::Endian::Big, ())
                    .map_err(|e| -> BoxError { e.to_string().into() })?;
                let decompressed = blte.decompress_with_keys(key_store.as_ref())?;
                let manifest = SizeManifest::parse(&decompressed)?;
                let total = manifest.header.total_size;
                if total > 0 {
                    return Ok(total);
                }
            }
            Err(e) => {
                debug!(product, error = %e, "size manifest download failed, falling back to CDN config");
            }
        }
    }

    // Fallback: estimate from CDN config archive sizes.
    estimate_from_cdn_config(state, &meta).await
}

/// Build the TACT key store for size estimation.
///
/// Starts with hardcoded WoW keys and optionally merges a caller-supplied key.
/// `encryption_key` must be `{id_hex}:{key_hex}` (colon-separated). If the
/// format does not match or hex decoding fails, the extra key is silently
/// skipped.
fn build_estimation_key_store(
    encryption_key: Option<&str>,
) -> Arc<dyn cascette_crypto::TactKeyProvider + Send + Sync> {
    let mut store = TactKeyStore::new();

    if let Some(raw) = encryption_key
        && let Some((id_hex, key_hex)) = raw.split_once(':')
        && let Ok(id) = u64::from_str_radix(id_hex.trim(), 16)
        && let Ok(key_bytes) = hex::decode(key_hex.trim())
        && let Ok(key_arr) = <[u8; 16]>::try_from(key_bytes)
    {
        store.add(TactKey::new(id, key_arr));
    }

    Arc::new(store)
}

/// Fallback size estimation from CDN config archive counts.
///
/// Downloads the CDN config text, sums `archives-sizes` values, or
/// estimates ~100 MB per archive if sizes are not listed.
async fn estimate_from_cdn_config(
    state: &AppState,
    meta: &crate::executor::helpers::ProductMetadata,
) -> Result<u64, BoxError> {
    let cdn_config_key = hex::decode(&meta.cdn_config)?;
    let cdn_config_data = state
        .cdn_client
        .download_from_endpoints(&meta.endpoints, ContentType::Config, &cdn_config_key)
        .await?;

    let cdn_config_text = String::from_utf8_lossy(&cdn_config_data);
    let mut archive_count: u64 = 0;
    let mut total_size_estimate: u64 = 0;

    for line in cdn_config_text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once(" = ") {
            if key == "archives" {
                archive_count = value.split_whitespace().count() as u64;
            } else if key == "archives-sizes" {
                total_size_estimate = value
                    .split_whitespace()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .sum();
            }
        }
    }

    if total_size_estimate == 0 && archive_count > 0 {
        total_size_estimate = archive_count * 100 * 1024 * 1024;
    }

    debug!(
        archive_count,
        estimated_bytes = total_size_estimate,
        "CDN config fallback size estimation"
    );

    Ok(total_size_estimate)
}

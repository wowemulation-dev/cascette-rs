//! GET/POST /option -- Product user options (language, region preferences).
//!
//! Options are stored in-memory per-product. The agent reads the default
//! locale from its configuration.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::handlers::error_codes::AGENT_ERROR_INVALID_REQUEST;
use crate::server::router::AppState;

/// Option request body (POST).
#[derive(Debug, Deserialize)]
pub struct OptionRequest {
    /// Product unique identifier.
    pub uid: Option<String>,
    /// Language setting.
    pub language: Option<String>,
    /// Region setting.
    pub region: Option<String>,
}

/// Query parameters for GET /option.
#[derive(Debug, Deserialize)]
pub struct OptionQuery {
    /// Product unique identifier for per-product option lookup.
    pub uid: Option<String>,
}

/// GET /option -- get current options.
///
/// When `uid` is provided as a query parameter, returns the product-specific
/// locale and region. Otherwise returns the global default locale.
pub async fn get_option(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OptionQuery>,
) -> Json<serde_json::Value> {
    if let Some(ref uid) = query.uid
        && let Ok(product) = state.registry.get(uid).await
    {
        return Json(serde_json::json!({
            "uid": uid,
            "language": product.locale.as_deref().unwrap_or(&state.config.locale),
            "region": product.region,
        }));
    }

    Json(serde_json::json!({
        "options": {
            "default_locale": state.config.locale,
        },
    }))
}

/// POST /option -- update options for a product.
///
/// When a product uid is provided, updates that product's locale/region
/// in the registry. Returns error 2312 if uid is given but product is not found
/// (matches Agent.exe behavior for unknown UIDs).
pub async fn post_option(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OptionRequest>,
) -> impl IntoResponse {
    if let Some(ref uid) = body.uid {
        match state.registry.get(uid).await {
            Ok(mut product) => {
                if let Some(ref language) = body.language {
                    product.locale = Some(language.clone());
                }
                if let Some(ref region) = body.region {
                    product.region = Some(region.clone());
                }
                let _ = state.registry.update(&product).await;
            }
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": AGENT_ERROR_INVALID_REQUEST})),
                );
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "uid": body.uid,
            "language": body.language,
            "region": body.region,
        })),
    )
}

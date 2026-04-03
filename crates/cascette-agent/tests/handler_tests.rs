//! Handler-level integration tests (Tier 1).
//!
//! Uses `tower::ServiceExt::oneshot()` on the router. No TCP listener, no
//! network. Each test builds a `Request`, sends it through the router, and
//! asserts on the `Response`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[allow(dead_code)]
mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use cascette_agent::models::product::ProductStatus;

/// Read an axum response body as a JSON value.
async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Read an axum response body as a string.
async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

// ── Read-only endpoint tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_get_agent_info() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/agent")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["agent_version"].is_string());
    assert!(json.get("authorization").is_some());
    assert!(json["port"].is_number());
    assert!(json["uptime_seconds"].is_number());
}

#[tokio::test]
async fn test_get_health() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "ok");
    assert!(json["version"].is_string());
    assert!(json["uptime_seconds"].is_number());
}

#[tokio::test]
async fn test_get_version() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/version")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["version"].is_string());
    assert!(json["agent_version"].is_string());
    assert!(json["product_version"].is_string());
}

#[tokio::test]
async fn test_get_hardware() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/hardware")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["cpu_num_cores"].is_number());
    assert!(json["memory"].is_number());
    assert!(json["cpu_arch"].is_number());
    assert!(json["gpu_1"].is_object());
}

#[tokio::test]
async fn test_get_metrics() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text = body_string(resp).await;
    assert!(
        text.contains("cascette_") || text.contains("# HELP"),
        "metrics body should contain prometheus output"
    );
}

// ── Product CRUD endpoint tests ────────────────────────────────────────────

#[tokio::test]
async fn test_list_games_empty() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder().uri("/game").body(Body::empty()).unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test]
async fn test_list_games_with_product() {
    let state = common::test_app_state().await;
    common::seed_product(&state, "wow_classic", ProductStatus::Available).await;
    let router = common::test_router(state);

    let req = Request::builder().uri("/game").body(Body::empty()).unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["uid"], "wow_classic");
    assert_eq!(arr[0]["product_code"], "wow_classic");
    assert!(arr[0]["result_uri"].is_string());
}

#[tokio::test]
async fn test_get_game_found() {
    let state = common::test_app_state().await;
    common::seed_product(&state, "wow_classic", ProductStatus::Installed).await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/game/wow_classic")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["uid"], "wow_classic");
    assert_eq!(json["product_code"], "wow_classic");
    assert!(json["result_uri"].is_string());
}

#[tokio::test]
async fn test_get_game_not_found() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/game/nonexistent")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], 2312);
}

// ── Operation endpoint tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_get_progress_idle() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/install/wow_classic")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["state"], "idle");
}

#[tokio::test]
async fn test_post_install_creates_op() {
    let state = common::test_app_state().await;
    // Product must be registered first (install no longer auto-creates).
    common::seed_product(&state, "wow_classic", ProductStatus::Available).await;
    let router = common::test_router(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/install/wow_classic")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"priority": 700}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["uid"], "wow_classic");
    assert_eq!(json["priority"], 700);
    assert!(json["response_uri"].is_string());
}

#[tokio::test]
async fn test_post_install_rejects_installed() {
    let state = common::test_app_state().await;
    common::seed_product(&state, "wow_classic", ProductStatus::Installed).await;
    let router = common::test_router(state);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/install/wow_classic")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"priority": 700}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    // Error is now a numeric code (2312 = AGENT_ERROR_INVALID_REQUEST).
    assert_eq!(json["error"], 2312);
}

#[tokio::test]
async fn test_post_install_then_poll() {
    let state = common::test_app_state().await;
    // Product must be registered first.
    common::seed_product(&state, "wow_classic", ProductStatus::Available).await;
    let router = common::test_router(state.clone());

    // POST install
    let req = Request::builder()
        .method(Method::POST)
        .uri("/install/wow_classic")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"priority": 700}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Poll progress -- must not be idle since we just queued an operation
    let router2 = common::test_router(state);
    let req = Request::builder()
        .uri("/install/wow_classic")
        .body(Body::empty())
        .unwrap();
    let resp = router2.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_ne!(json["state"], "idle");
}

// ── Download / option / session endpoint tests ─────────────────────────────

#[tokio::test]
async fn test_get_download_defaults() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/download")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["max_speed_bps"], 0);
    assert_eq!(json["paused"], false);
}

#[tokio::test]
async fn test_post_download_update() {
    let state = common::test_app_state().await;

    // Seed a product so UID validation passes
    common::seed_product(
        &state,
        "wow_classic",
        cascette_agent::models::product::ProductStatus::Installed,
    )
    .await;

    // POST to update download config (Agent.exe wire format)
    let router = common::test_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/download")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"uid": "wow_classic", "download_limit": 1048576}"#,
        ))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert!(json["response_uri"].as_str().is_some());
    assert!(json["result_uri"].as_str().is_some());

    // GET /agent/download to verify per-product download_limit was stored
    let router2 = common::test_router(state);
    let req = Request::builder()
        .uri("/agent/download?uid=wow_classic")
        .body(Body::empty())
        .unwrap();
    let resp = router2.oneshot(req).await.unwrap();

    let json = body_json(resp).await;
    assert_eq!(json["download_limit"], 1_048_576);
    assert_eq!(json["priority"], 700);
}

#[tokio::test]
async fn test_post_download_unknown_uid() {
    let state = common::test_app_state().await;

    let router = common::test_router(state);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/download")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"uid": "nonexistent"}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], 2312);
}

#[tokio::test]
async fn test_get_option_defaults() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/option")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["options"]["default_locale"], "enUS");
}

#[tokio::test]
async fn test_post_option_updates_product() {
    let state = common::test_app_state().await;
    common::seed_product(&state, "wow_classic", ProductStatus::Available).await;

    // POST option with language update
    let router = common::test_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/option")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"uid": "wow_classic", "language": "deDE", "region": "eu"}"#,
        ))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify the product was updated in the registry
    let product = state.registry.get("wow_classic").await.unwrap();
    assert_eq!(product.locale.as_deref(), Some("deDE"));
    assert_eq!(product.region.as_deref(), Some("eu"));
}

#[tokio::test]
async fn test_get_sessions_empty() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/gamesession")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    let sessions = json["sessions"].as_array().unwrap();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn test_post_and_get_session() {
    let state = common::test_app_state().await;

    // POST to create a session without a pid (no process validation required)
    let router = common::test_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/gamesession/wow")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["active"], true);

    // GET to verify session was stored
    let router2 = common::test_router(state);
    let req = Request::builder()
        .uri("/gamesession/wow")
        .body(Body::empty())
        .unwrap();
    let resp = router2.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["active"], true);
}

#[tokio::test]
async fn test_size_estimate_no_uid() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/size_estimate")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], 2311);
}

// ── CORS tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cors_localhost_allowed() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/health")
        .header("origin", "http://localhost")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers().contains_key("access-control-allow-origin"),
        "localhost origin should get CORS header"
    );
}

#[tokio::test]
async fn test_cors_external_rejected() {
    let state = common::test_app_state().await;
    let router = common::test_router(state);

    let req = Request::builder()
        .uri("/health")
        .header("origin", "http://evil.com")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        !resp.headers().contains_key("access-control-allow-origin"),
        "external origin should NOT get CORS header"
    );
}

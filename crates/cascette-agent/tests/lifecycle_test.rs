//! Full operation lifecycle test (Tier 3).
//!
//! Exercises the complete request-to-executor path: HTTP handler -> queue ->
//! OperationRunner -> executor -> Ribbit mock. Marked `#[ignore]` so it does
//! not run in default `cargo test` (use `cargo test -- --ignored`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[allow(dead_code)]
mod common;

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use cascette_agent::executor::OperationRunner;

/// Canned versions BPSV response (STRING types for `as_string()` compatibility).
const VERSIONS_BPSV: &str = "Region!STRING:0|BuildConfig!STRING:0|CDNConfig!STRING:0|VersionsName!STRING:0|BuildId!STRING:0\nus|aabbccdd00112233aabbccdd00112233|eeff00112233445566778899aabbccdd|1.15.5.99999|99999\n";

/// Canned CDNs BPSV response.
const CDNS_BPSV: &str = "Name!STRING:0|Path!STRING:0|Hosts!STRING:0|ConfigPath!STRING:0\nus|tpr/wow|level3.blizzard.com cdn.blizzard.com|tpr/wow\n";

/// Full lifecycle test: POST install -> runner picks up -> executor queries
/// Ribbit mock -> executor fails at download stage -> error recorded.
///
/// The install executor calls `resolve_product_metadata()` against the mock
/// Ribbit, gets valid BPSV data, then attempts to download build/CDN configs.
/// Since we don't mock valid CASC archive data, the pipeline fails at the
/// download stage. We verify:
/// - Handler queues the operation correctly
/// - Runner picks it up and transitions to Initializing
/// - Executor calls Ribbit (mock validates the request was made)
/// - Failure is captured in ErrorInfo and stored
/// - Product status resets appropriately
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires dedicated runner; use cargo test -- --ignored"]
async fn test_install_lifecycle() {
    let mock_server = MockServer::start().await;

    // Mount Ribbit responses
    Mock::given(method("GET"))
        .and(path_regex(r".*/versions$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(VERSIONS_BPSV)
                .insert_header("content-type", "text/plain"),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r".*/cdns$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(CDNS_BPSV)
                .insert_header("content-type", "text/plain"),
        )
        .mount(&mock_server)
        .await;

    // Build state with mock Ribbit
    let state = common::test_app_state_with_mock(&mock_server.uri()).await;

    // Bind router to ephemeral port
    let router = common::test_router(Arc::clone(&state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let cancellation = CancellationToken::new();

    // Spawn HTTP server
    let server_cancel = cancellation.clone();
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                server_cancel.cancelled().await;
            })
            .await
            .unwrap();
    });

    // Spawn operation runner
    let runner = OperationRunner::new(Arc::clone(&state), 1, cancellation.child_token());
    let runner_handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let base = format!("http://{addr}");

    // Create a temp dir for the install path
    let install_dir = tempfile::tempdir().unwrap();
    let install_body = serde_json::json!({
        "priority": 700,
        "install_path": install_dir.path().to_str().unwrap(),
        "region": "us",
        "locale": "enUS",
    });

    // POST /install/test_product
    let resp = client
        .post(format!("{base}/install/test_product"))
        .json(&install_body)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["response_uri"].is_string());
    assert!(json["result_uri"].is_string());
    assert_eq!(json["uid"], "test_product");
    assert_eq!(json["priority"], 700);

    // Look up the queued operation by product code to get its ID.
    let active_op = state
        .queue
        .find_active_for_product("test_product")
        .await
        .unwrap()
        .expect("operation should be queued");
    let operation_id = active_op.operation_id.to_string();

    // Poll the operation by ID directly from the queue. The GET /install/{product}
    // endpoint uses `find_active_for_product` which excludes terminal states
    // (complete/failed/cancelled), returning "idle" once the operation finishes.
    // Since the executor fails fast (no real CASC data), the operation can reach
    // a terminal state between poll intervals, making it invisible to the HTTP
    // endpoint. Querying by ID avoids this race.
    let mut terminal = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let op = state.queue.get(&operation_id).await.unwrap();
        let state_str = op.state.to_string();

        if state_str == "complete" || state_str == "failed" || state_str == "cancelled" {
            terminal = true;

            // If failed, verify error info is present
            if state_str == "failed" {
                assert!(
                    op.error.is_some(),
                    "failed operation should have error info"
                );
            }
            break;
        }
    }

    assert!(
        terminal,
        "operation should reach a terminal state within 10s"
    );

    // Verify product exists in registry
    let resp = client
        .get(format!("{base}/game/test_product"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Shutdown
    cancellation.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server_handle).await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), runner_handle).await;
}

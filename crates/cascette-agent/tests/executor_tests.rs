//! Executor helper tests (Tier 2) -- wiremock for Ribbit responses.
//!
//! Tests the `helpers` module functions that query Ribbit. Uses
//! `wiremock::MockServer` with canned BPSV responses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use cascette_agent::executor::helpers;
use cascette_protocol::CdnEndpoint;

/// Canned versions BPSV response for a test product.
///
/// Uses STRING:0 for BuildConfig/CDNConfig because `helpers::resolve_product_metadata`
/// reads these fields via `as_string()`, which only matches `BpsvValue::String`.
const VERSIONS_BPSV: &str = "Region!STRING:0|BuildConfig!STRING:0|CDNConfig!STRING:0|VersionsName!STRING:0|BuildId!STRING:0\nus|aabbccdd00112233aabbccdd00112233|eeff00112233445566778899aabbccdd|1.15.5.99999|99999\n";

/// Canned CDNs BPSV response.
const CDNS_BPSV: &str = "Name!STRING:0|Path!STRING:0|Hosts!STRING:0|ConfigPath!STRING:0\nus|tpr/wow|level3.blizzard.com cdn.blizzard.com|tpr/wow\n";

/// Mount canned Ribbit responses on the mock server for versions and CDNs.
async fn mount_ribbit_mocks(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path_regex(r".*/versions$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(VERSIONS_BPSV)
                .insert_header("content-type", "text/plain"),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r".*/cdns$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(CDNS_BPSV)
                .insert_header("content-type", "text/plain"),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_resolve_product_metadata() {
    let server = MockServer::start().await;
    mount_ribbit_mocks(&server).await;

    let state = common::test_app_state_with_mock(&server.uri()).await;

    let metadata = helpers::resolve_product_metadata(
        &state.ribbit_client,
        &state.cdn_client,
        "test_product",
        "us",
        None,
    )
    .await
    .unwrap();

    assert_eq!(metadata.build_config, "aabbccdd00112233aabbccdd00112233");
    assert_eq!(metadata.cdn_config, "eeff00112233445566778899aabbccdd");
    assert_eq!(metadata.cdn_path, "tpr/wow");
    assert_eq!(metadata.version_name, "1.15.5.99999");
    assert_eq!(metadata.build_id, "99999");
    assert!(!metadata.endpoints.is_empty());
}

#[tokio::test]
async fn test_resolve_metadata_with_overrides() {
    let server = MockServer::start().await;
    mount_ribbit_mocks(&server).await;

    let state = common::test_app_state_with_mock(&server.uri()).await;

    let overrides = vec![CdnEndpoint {
        host: "custom.cdn.example.com".to_string(),
        path: "custom/path".to_string(),
        product_path: None,
        scheme: None,
        is_fallback: false,
        strict: false,
        max_hosts: None,
    }];

    let metadata = helpers::resolve_product_metadata(
        &state.ribbit_client,
        &state.cdn_client,
        "test_product",
        "us",
        Some(&overrides),
    )
    .await
    .unwrap();

    // Overrides are prepended; Ribbit-advertised endpoints follow as fallback.
    assert!(!metadata.endpoints.is_empty());
    assert_eq!(metadata.endpoints[0].host, "custom.cdn.example.com");
    assert_eq!(metadata.cdn_path, "custom/path");
}

#[tokio::test]
async fn test_resolve_metadata_not_found() {
    let server = MockServer::start().await;

    // Mount a 404 for versions
    Mock::given(method("GET"))
        .and(path_regex(r".*/versions$"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let state = common::test_app_state_with_mock(&server.uri()).await;

    let result = helpers::resolve_product_metadata(
        &state.ribbit_client,
        &state.cdn_client,
        "nonexistent_product",
        "us",
        None,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_progress_bridge_event_mapping() {
    use cascette_installation::ProgressEvent;

    let state = common::test_app_state().await;

    // Seed a product so the foreign key constraint is satisfied
    common::seed_product(
        &state,
        "test",
        cascette_agent::models::product::ProductStatus::Available,
    )
    .await;

    // Insert a dummy operation so the bridge can persist
    let operation = cascette_agent::models::operation::Operation::new(
        "test".to_string(),
        cascette_agent::models::operation::OperationType::Install,
        cascette_agent::models::operation::Priority::Normal,
        None,
    );
    let op_id = operation.operation_id;
    state.queue.insert(&operation).await.unwrap();

    let (bridge, flush_handle) = helpers::ProgressBridge::new(op_id, &state);
    let callback = bridge.callback();

    // Send MetadataResolved event
    callback(ProgressEvent::MetadataResolved {
        artifacts: 100,
        total_bytes: 1_000_000,
    });

    // Send FileDownloading event
    callback(ProgressEvent::FileDownloading {
        path: "data/00/00/00.data".to_string(),
        size: 5000,
    });

    // Send FileComplete event
    callback(ProgressEvent::FileComplete {
        path: "data/00/00/00.data".to_string(),
    });

    // Give the flush task a moment to run, then abort it
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    flush_handle.abort();
}

#[tokio::test]
async fn test_build_install_config() {
    let endpoints = vec![CdnEndpoint {
        host: "cdn.example.com".to_string(),
        path: "tpr/wow".to_string(),
        product_path: None,
        scheme: None,
        is_fallback: false,
        strict: false,
        max_hosts: None,
    }];

    let config = helpers::build_install_config(
        "wow",
        "/opt/games/wow",
        "tpr/wow",
        endpoints,
        "us",
        "enUS",
        Some("aabb".to_string()),
        Some("ccdd".to_string()),
        None,
    );

    assert_eq!(config.product, "wow");
    assert_eq!(config.install_path.to_str().unwrap(), "/opt/games/wow");
    assert_eq!(config.cdn_path, "tpr/wow");
    assert_eq!(config.region, "us");
    assert_eq!(config.locale, "enUS");
    assert_eq!(config.build_config, Some("aabb".to_string()));
    assert_eq!(config.cdn_config, Some("ccdd".to_string()));
    assert_eq!(config.max_connections_per_host, 3);
    assert_eq!(config.max_connections_global, 12);
    assert!(config.resume);
}

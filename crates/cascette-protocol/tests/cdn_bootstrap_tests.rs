#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for the CDN bootstrap workflow.
//!
//! Every TACT-aware tool (TACTSharp, wow.export, BuildBackup, CASCHost)
//! performs the same bootstrap sequence:
//!   1. Fetch versions BPSV → extract BuildConfig + CDNConfig hashes
//!   2. Fetch cdns BPSV → extract CDN hosts + path
//!   3. Fetch BuildConfig from CDN config path
//!   4. Fetch CDNConfig from CDN config path
//!
//! These tests use wiremock to simulate the Blizzard TACT HTTP endpoint
//! and verify the protocol layer correctly parses BPSV responses and
//! propagates errors (404, 503, rate-limit).

use cascette_formats::bpsv::parse as parse_bpsv;
use cascette_protocol::client::TactClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Fixture helpers ───────────────────────────────────────────────────────

/// A minimal versions BPSV for wow_classic_era (3 regions).
fn versions_bpsv() -> &'static str {
    "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16\n\
     ## seqn = 6226474\n\
     us|2b4e7dc7462f9ca5d2e6b5b0ce9cf1d3|aa3a0d72d7dfc97d4d3e2ecba86a9b31||6226474|1.15.5.56260|\n\
     eu|2b4e7dc7462f9ca5d2e6b5b0ce9cf1d3|aa3a0d72d7dfc97d4d3e2ecba86a9b31||6226474|1.15.5.56260|\n\
     kr|2b4e7dc7462f9ca5d2e6b5b0ce9cf1d3|aa3a0d72d7dfc97d4d3e2ecba86a9b31||6226474|1.15.5.56260|\n"
}

/// A minimal cdns BPSV for wow_classic_era.
fn cdns_bpsv() -> &'static str {
    "Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0\n\
     ## seqn = 6226474\n\
     us|tpr/wow_classic|blzddist1-a.akamaihd.net blzddist2-a.akamaihd.net|http://us.patch.battle.net:1119/wow_classic|tpr/configs/data\n\
     eu|tpr/wow_classic|eu.cdn.blizzard.com level3.blizzard.com|http://eu.patch.battle.net:1119/wow_classic|tpr/configs/data\n"
}

// ─── TactClient + wiremock tests ──────────────────────────────────────────

#[tokio::test]
async fn tact_query_versions_parses_bpsv() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow_classic_era/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(versions_bpsv()))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    let doc = client
        .query("/wow_classic_era/versions")
        .await
        .expect("versions query should succeed");

    assert_eq!(doc.row_count(), 3, "Should parse 3 region rows");
    assert_eq!(doc.sequence_number(), Some(6_226_474));
    assert!(doc.has_field("BuildConfig"));
    assert!(doc.has_field("CDNConfig"));
}

#[tokio::test]
async fn tact_query_cdns_parses_bpsv() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow_classic_era/cdns"))
        .respond_with(ResponseTemplate::new(200).set_body_string(cdns_bpsv()))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    let doc = client
        .query("/wow_classic_era/cdns")
        .await
        .expect("cdns query should succeed");

    assert_eq!(doc.row_count(), 2, "Should parse 2 CDN region rows");
    assert!(doc.has_field("Hosts"));
    assert!(doc.has_field("Path"));
}

#[tokio::test]
async fn tact_query_404_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/nonexistent/versions"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    let result = client.query("/nonexistent/versions").await;

    assert!(result.is_err(), "404 response must return an error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("404") || err.contains("not found") || err.contains("Not Found"),
        "Error must mention 404: {err}"
    );
}

#[tokio::test]
async fn tact_query_503_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow/versions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    let result = client.query("/wow/versions").await;

    assert!(result.is_err(), "503 response must return an error");
}

#[tokio::test]
async fn tact_query_invalid_bpsv_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow/versions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("not a valid bpsv response\nno field types"),
        )
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    let result = client.query("/wow/versions").await;

    assert!(result.is_err(), "Invalid BPSV must return parse error");
}

#[tokio::test]
async fn tact_query_versions_extract_build_config() {
    // Mirrors TACTSharp ExtractionTests: pinned build 9.0.1.35078
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16\n\
             ## seqn = 35078\n\
             us|43a001a23efd4193a96266be43fe67d8|c67fdeccf96e2a0ddf205e0a7e8f1927||35078|9.0.1.35078|\n\
             eu|43a001a23efd4193a96266be43fe67d8|c67fdeccf96e2a0ddf205e0a7e8f1927||35078|9.0.1.35078|\n",
        ))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    let doc = client
        .query("/wow/versions")
        .await
        .expect("query should succeed");

    // Extract the us row BuildConfig
    let us_row = doc.get_row(0).expect("Should have us row");
    assert_eq!(us_row.get_raw(0), Some("us"));
    let build_config = us_row.get_raw(1).expect("BuildConfig must be present");
    assert_eq!(
        build_config, "43a001a23efd4193a96266be43fe67d8",
        "BuildConfig must match the pinned TACTSharp test build"
    );
}

#[tokio::test]
async fn tact_query_v1_products_endpoint_format() {
    // The TCP Ribbit format "v1/products/{product}/versions" must also work
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow_classic_era/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(versions_bpsv()))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");
    // Send in TCP Ribbit format — TactClient strips the "v1/products" prefix
    let doc = client
        .query("v1/products/wow_classic_era/versions")
        .await
        .expect("v1/products format should be rewritten to TACT format");

    assert_eq!(doc.row_count(), 3);
}

// ─── BPSV integration: CDN bootstrap workflow ─────────────────────────────

#[tokio::test]
async fn bootstrap_versions_then_cdns() {
    // Full two-step bootstrap: versions → extract configs, cdns → extract hosts
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wow_classic_era/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(versions_bpsv()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/wow_classic_era/cdns"))
        .respond_with(ResponseTemplate::new(200).set_body_string(cdns_bpsv()))
        .mount(&server)
        .await;

    let client = TactClient::new(server.uri(), false).expect("Client creation should succeed");

    // Step 1: fetch versions
    let versions = client
        .query("/wow_classic_era/versions")
        .await
        .expect("versions fetch should succeed");

    let us_versions = versions.get_row(0).expect("Should have us row");
    let build_config = us_versions.get_raw(1).expect("BuildConfig must be present");
    let cdn_config = us_versions.get_raw(2).expect("CDNConfig must be present");

    assert_eq!(build_config.len(), 32, "BuildConfig must be 32-char hex");
    assert_eq!(cdn_config.len(), 32, "CDNConfig must be 32-char hex");

    // Step 2: fetch CDNs
    let cdns = client
        .query("/wow_classic_era/cdns")
        .await
        .expect("cdns fetch should succeed");

    let us_cdns = cdns.get_row(0).expect("Should have us CDN row");
    let cdn_path = us_cdns.get_raw(1).expect("CDN path must be present");
    let hosts_field = us_cdns.get_raw(2).expect("Hosts must be present");

    assert!(
        cdn_path.starts_with("tpr/"),
        "CDN path must start with tpr/"
    );
    assert!(
        hosts_field.split_whitespace().count() >= 2,
        "Must have at least 2 CDN hosts for failover"
    );
}

// ─── BPSV parser standalone tests (with real CDN-like content) ────────────

#[test]
fn bpsv_parse_realistic_versions_content() {
    // Directly exercise the BPSV parser with content matching CDN wire format.
    // This validates the same code path used by TactClient::query.
    let content = versions_bpsv();
    let doc = parse_bpsv(content).expect("Realistic versions BPSV should parse");

    assert_eq!(doc.row_count(), 3);
    assert_eq!(doc.sequence_number(), Some(6_226_474));

    // Verify the BuildConfig is consistently the same across all regions
    let configs: Vec<&str> = doc.iter().filter_map(|row| row.get_raw(1)).collect();
    assert_eq!(configs.len(), 3);
    let first_config = configs[0];
    for cfg in &configs {
        assert_eq!(
            *cfg, first_config,
            "All regions should share the same build"
        );
    }
}

#[test]
fn bpsv_parse_realistic_cdns_content() {
    let content = cdns_bpsv();
    let doc = parse_bpsv(content).expect("Realistic cdns BPSV should parse");

    assert_eq!(doc.row_count(), 2);

    for (i, row) in doc.iter().enumerate() {
        // CDN path
        let path = row
            .get_raw(1)
            .unwrap_or_else(|| panic!("Row {i} missing Path"));
        assert!(
            path.starts_with("tpr/"),
            "Row {i} CDN path must start with tpr/"
        );

        // CDN hosts (space-separated, minimum 2 for failover)
        let hosts = row
            .get_raw(2)
            .unwrap_or_else(|| panic!("Row {i} missing Hosts"));
        assert!(
            hosts.split_whitespace().count() >= 2,
            "Row {i} must have at least 2 CDN hosts"
        );

        // Config path
        let config_path = row
            .get_raw(4)
            .unwrap_or_else(|| panic!("Row {i} missing ConfigPath"));
        assert!(
            !config_path.is_empty(),
            "Row {i} ConfigPath must not be empty"
        );
    }
}

#[test]
fn bpsv_seqn_matches_between_versions_and_cdns() {
    // In a real bootstrap, versions and cdns should share the same seqn
    let versions = parse_bpsv(versions_bpsv()).expect("versions parse");
    let cdns = parse_bpsv(cdns_bpsv()).expect("cdns parse");

    assert_eq!(
        versions.sequence_number(),
        cdns.sequence_number(),
        "versions and cdns seqn must match for a consistent snapshot"
    );
}

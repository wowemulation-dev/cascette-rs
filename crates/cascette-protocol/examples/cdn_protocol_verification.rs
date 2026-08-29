#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! CDN protocol verification against live community CDN mirrors.
//!
//! Exercises `cascette-protocol` TACT/Ribbit queries and CDN downloads using
//! pinned hashes from WoW Classic 1.13.2.31650.
//!
//! Environment variables:
//!   CASCETTE_CDN_HOSTS  Comma-separated CDN hostnames
//!                       (default: casc.wago.tools,cdn.arctium.tools,archive.wow.tools)
//!   CASCETTE_CDN_PATH   CDN product path (default: tpr/wow)
//!
//! Usage:
//!   cargo run -p cascette-protocol --example cdn_protocol_verification

use cascette_protocol::{
    CacheConfig, CdnClient, CdnConfig, CdnEndpoint, ClientConfig, ContentType, RibbitTactClient,
};

// ---------------------------------------------------------------------------
// Pinned hashes from WoW Classic 1.13.2.31650
// ---------------------------------------------------------------------------

const BUILD_CONFIG: &str = "2c915a9a226a3f35af6c65fcc7b6ca4a";
const CDN_CONFIG: &str = "c54b41b3195b9482ce0d3c6bf0b86cdb";
const ENCODING_EKEY: &str = "59cad02d7dc0187413ae485a766f851b";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cdn_endpoints() -> Vec<CdnEndpoint> {
    let hosts = std::env::var("CASCETTE_CDN_HOSTS")
        .unwrap_or_else(|_| "casc.wago.tools,cdn.arctium.tools,archive.wow.tools".into());
    let path = std::env::var("CASCETTE_CDN_PATH").unwrap_or_else(|_| "tpr/wow".into());
    hosts
        .split(',')
        .map(|h| CdnEndpoint {
            host: h.trim().into(),
            path: path.clone(),
            product_path: None,
            scheme: Some("https".to_string()),
            is_fallback: false,
            strict: false,
            max_hosts: None,
        })
        .collect()
}

fn community_client_config() -> ClientConfig {
    ClientConfig {
        tact_https_url: "https://us.version.battle.net".to_string(),
        tact_http_url: String::new(),
        ribbit_url: "tcp://127.0.0.1:1".to_string(),
        cache_config: CacheConfig::memory_optimized(),
        ..Default::default()
    }
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex).unwrap_or_else(|e| panic!("invalid hex '{hex}': {e}"))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let config = community_client_config();
    let client = RibbitTactClient::new(config).expect("client creation");
    let cdn = CdnClient::new(client.cache().clone(), CdnConfig::default()).expect("cdn client");
    let endpoints = cdn_endpoints();

    // A1: Query versions
    println!("=== A1: Query wow_classic versions ===");
    let versions = client
        .query("v1/products/wow_classic/versions")
        .await
        .expect("versions query should succeed");

    assert!(
        !versions.rows().is_empty(),
        "should have at least one version row"
    );

    let schema = versions.schema();
    let has_version = versions.rows().iter().any(|row| {
        row.get_raw_by_name("VersionsName", schema)
            .is_some_and(|v| v.contains("1.13.2"))
    });
    println!(
        "  {} rows, has 1.13.2: {}",
        versions.rows().len(),
        has_version
    );

    // A2: Query CDN configuration
    println!("\n=== A2: Query CDN configuration ===");
    let cdns = client
        .query("v1/products/wow_classic/cdns")
        .await
        .expect("cdns query should succeed");

    assert!(!cdns.rows().is_empty(), "should have at least one CDN row");

    let schema = cdns.schema();
    let has_path = cdns.rows().iter().any(|row| {
        row.get_raw_by_name("Path", schema)
            .is_some_and(|v| v == "tpr/wow")
    });
    assert!(has_path, "CDN response should contain tpr/wow path");
    println!(
        "  {} rows, has tpr/wow path: {}",
        cdns.rows().len(),
        has_path
    );

    // A3: Download pinned build config
    println!("\n=== A3: Download build config ===");
    let key = hex_bytes(BUILD_CONFIG);
    let data = cdn
        .download_from_endpoints(&endpoints, ContentType::Config, &key)
        .await
        .expect("build config download should succeed");

    let text = String::from_utf8_lossy(&data);
    assert!(
        text.contains("root"),
        "build config should contain 'root' key"
    );
    assert!(
        text.contains("encoding"),
        "build config should contain 'encoding' key"
    );
    println!("  {} bytes, contains root and encoding keys", data.len());

    // A4: Download pinned CDN config
    println!("\n=== A4: Download CDN config ===");
    let key = hex_bytes(CDN_CONFIG);
    let data = cdn
        .download_from_endpoints(&endpoints, ContentType::Config, &key)
        .await
        .expect("CDN config download should succeed");

    let text = String::from_utf8_lossy(&data);
    assert!(
        text.contains("archives"),
        "CDN config should contain 'archives' key"
    );
    println!("  {} bytes, contains archives key", data.len());

    // A5: Download encoding table, verify BLTE magic
    println!("\n=== A5: Download encoding table ===");
    let key = hex_bytes(ENCODING_EKEY);
    let data = cdn
        .download_from_endpoints(&endpoints, ContentType::Data, &key)
        .await
        .expect("encoding table download should succeed");

    assert!(data.len() > 4, "encoding data should be non-trivial");
    assert_eq!(&data[..4], b"BLTE", "encoding data should be BLTE-encoded");
    println!("  {} bytes (BLTE-encoded)", data.len());

    // A6: Request nonexistent hash, verify error handling
    println!("\n=== A6: Nonexistent hash error handling ===");
    let fake_key = hex_bytes("00000000000000000000000000000000");
    let result = cdn
        .download_from_endpoints(&endpoints, ContentType::Config, &fake_key)
        .await;

    assert!(
        result.is_err(),
        "downloading a nonexistent hash should return an error"
    );
    println!("  error: {}", result.unwrap_err());

    println!("\nAll CDN protocol checks passed.");
}

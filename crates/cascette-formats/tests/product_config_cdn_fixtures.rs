#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for ProductConfig parsing using real CDN data.
//!
//! Tests parse a retail WoW product config fetched from Blizzard CDN.

use cascette_formats::config::ProductConfig;
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/config")
        .leak()
}

fn load_fixture(filename: &str) -> Vec<u8> {
    let path = fixtures_dir().join(filename);
    std::fs::read(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

#[test]
fn product_config_wow_retail_parse() {
    let data = load_fixture("wow_product_config.json");
    let text = String::from_utf8(data).expect("Should be valid UTF-8");
    let config: ProductConfig =
        serde_json::from_str(&text).expect("Should parse product config JSON");

    assert_eq!(config.all.config.product.as_deref(), Some("WoW"));
    assert_eq!(config.all.config.data_dir.as_deref(), Some("Data/"));

    let locales = config.all.config.supported_locales.as_ref();
    assert!(locales.is_some());
    let locales = locales.unwrap();
    assert!(locales.contains(&"enUS".to_string()));
    assert!(locales.contains(&"deDE".to_string()));
    assert!(locales.len() >= 10);
}

#[test]
fn product_config_wow_retail_has_platform_info() {
    let data = load_fixture("wow_product_config.json");
    let text = String::from_utf8(data).expect("Should be valid UTF-8");
    let config: ProductConfig =
        serde_json::from_str(&text).expect("Should parse product config JSON");

    assert!(config.platform.is_some());
}

#[test]
fn product_config_wow_retail_round_trip() {
    let data = load_fixture("wow_product_config.json");
    let text = String::from_utf8(data).expect("Should be valid UTF-8");
    let config: ProductConfig =
        serde_json::from_str(&text).expect("Should parse product config JSON");

    let rebuilt_json = serde_json::to_string(&config).expect("Should serialize");
    let reparsed: ProductConfig = serde_json::from_str(&rebuilt_json).expect("Should re-parse");

    assert_eq!(reparsed.all.config.product, config.all.config.product);
    assert_eq!(
        reparsed.all.config.supported_locales,
        config.all.config.supported_locales
    );
}

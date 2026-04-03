#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Wago API verification for community build data.
//!
//! Exercises `cascette-import` against the live Wago API to fetch and search
//! WoW Classic build information.
//!
//! Usage:
//!   cargo run -p cascette-import --example wago_verification

use cascette_import::{BuildSearchCriteria, ImportManager, WagoProvider};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    let wago = WagoProvider::new(cache_dir.path().to_path_buf()).expect("wago provider");

    let mut manager = ImportManager::new();
    manager
        .add_provider("wago", Box::new(wago))
        .await
        .expect("add provider");

    // D1: Fetch all wow_classic builds
    println!("=== D1: Fetch wow_classic builds ===");
    let builds = manager
        .get_builds("wow_classic")
        .await
        .expect("get_builds should succeed");

    assert!(!builds.is_empty(), "wago should have wow_classic builds");

    for build in &builds[..std::cmp::min(3, builds.len())] {
        assert_eq!(build.product, "wow_classic");
        assert!(!build.version.is_empty(), "version should be non-empty");
        assert!(build.build > 0, "build number should be positive");
    }

    println!("  {} builds found", builds.len());

    // D2: Search for 1.13.* builds
    println!("\n=== D2: Search for 1.13.* builds ===");
    let criteria = BuildSearchCriteria {
        product: Some("wow_classic".to_string()),
        version_pattern: Some("1.13.*".to_string()),
        min_build: None,
        max_build: None,
        version_type: None,
        region: None,
        metadata_filters: HashMap::new(),
    };

    let results = manager
        .search_builds(&criteria)
        .await
        .expect("search_builds should succeed");

    assert!(
        !results.is_empty(),
        "search for 1.13.* should return results"
    );

    let has_pinned = results.iter().any(|b| b.version.contains("1.13.2.31650"));
    println!(
        "  {} results, has 1.13.2.31650: {}",
        results.len(),
        has_pinned,
    );

    println!("\nAll Wago API checks passed.");
}

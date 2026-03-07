#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! BlizzTrack API verification for TACT product build data.
//!
//! Exercises `cascette-import` against the live BlizzTrack API to fetch
//! per-region build information for `agent` and `wow_classic_era`.
//!
//! Usage:
//!   cargo run -p cascette-import --example blizztrack_verification --features blizztrack

use cascette_import::{BlizzTrackProvider, ImportManager};

#[tokio::main]
async fn main() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    let provider =
        BlizzTrackProvider::new(cache_dir.path().to_path_buf()).expect("blizztrack provider");

    let mut manager = ImportManager::new();
    manager
        .add_provider("blizztrack", Box::new(provider))
        .await
        .expect("add provider");

    // D1: Fetch agent builds (no community CDN mirror — BlizzTrack-only)
    println!("=== D1: Fetch agent builds ===");
    let builds = manager
        .get_builds("agent")
        .await
        .expect("get_builds should succeed");

    assert!(!builds.is_empty(), "BlizzTrack should have agent builds");

    for build in &builds[..std::cmp::min(3, builds.len())] {
        assert_eq!(build.product, "agent");
        assert!(!build.version.is_empty(), "version should be non-empty");
        assert!(build.build > 0, "build number should be positive");
        assert!(build.region.is_some(), "region should be set");
    }

    println!("  {} builds found (per-region)", builds.len());
    for build in &builds[..std::cmp::min(5, builds.len())] {
        println!(
            "  region={} version={} build={}",
            build.region.as_deref().unwrap_or("?"),
            build.version,
            build.build,
        );
    }

    // D2: Fetch wow_classic_era builds as a WoW cross-check
    println!("\n=== D2: Fetch wow_classic_era builds ===");
    let wow_builds = manager
        .get_builds("wow_classic_era")
        .await
        .expect("get_builds should succeed");

    assert!(
        !wow_builds.is_empty(),
        "BlizzTrack should have wow_classic_era builds"
    );

    println!("  {} builds found (per-region)", wow_builds.len());
    for build in &wow_builds[..std::cmp::min(5, wow_builds.len())] {
        println!(
            "  region={} version={} build={}",
            build.region.as_deref().unwrap_or("?"),
            build.version,
            build.build,
        );
    }

    println!("\nAll BlizzTrack API checks passed.");
}
